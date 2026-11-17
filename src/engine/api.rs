//! Public API for games built on the engine, modeled after Unity's MonoBehaviour.
//!
//! A game is a set of `GameObject`s. Each one has a `Transform`, an optional
//! model and any number of `Behaviour`s. The engine calls the behaviours'
//! lifecycle methods every frame:
//!
//!   start        once, right before the behaviour's first update
//!   update       every frame
//!   late_update  every frame, after every object ran `update`
//!   on_destroy   once, when its object is destroyed (at the end of the frame)
//!
//! ```ignore
//! use slender::engine::prelude::*;
//!
//! struct Spin { speed: f32 }
//! impl Behaviour for Spin {
//!     fn update(&mut self, ctx: &mut Context) {
//!         let dt = ctx.time().delta;
//!         ctx.transform_mut().rotation *= Quat::from_rotation_y(self.speed * dt);
//!     }
//! }
//!
//! fn setup(ctx: &mut Context) -> anyhow::Result<()> {
//!     ctx.spawn("crate")
//!         .with_position(Vec3::new(0.0, 0.0, 5.0))
//!         .with_model("models/crate.glb", Mobility::Dynamic)?
//!         .with_behaviour(Spin { speed: 1.0 });
//!     Ok(())
//! }
//!
//! fn main() { slender::app::App::run(setup).unwrap(); }
//! ```

use super::input::Input;
use super::level::{self, ColliderDef, ColliderSource, LevelDef, PrefabDef, ShapeDef};
use crate::math::Transform;
use crate::phys::collider::{Collider, Shape};
use crate::phys::collision::circle_push;
use crate::phys::hitbox::{Circle, Hitbox};
use crate::render::asset_id::ModelId;
use crate::render::asset_manager::AssetManager;
use crate::render::camera::Camera;
use crate::render::context::RenderContext;
use crate::render::light::Light;
use crate::render::model::NodeKind;
use crate::render::scene::{Instance, InstanceKey, Mobility, Scene};
use glam::{Quat, Vec2, Vec3};
use slotmap::{SlotMap, new_key_type};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Game code entry point: spawns the initial objects.
pub type Setup = fn(&mut Context) -> anyhow::Result<()>;

new_key_type! { pub struct GameObjectId; }

/// Script attached to a `GameObject`. Every method is optional.
pub trait Behaviour: Any {
    fn start(&mut self, _ctx: &mut Context) {}
    fn update(&mut self, _ctx: &mut Context) {}
    fn late_update(&mut self, _ctx: &mut Context) {}
    fn on_destroy(&mut self, _ctx: &mut Context) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Start,
    Update,
    LateUpdate,
    Destroy,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Time {
    /// Seconds since the last frame.
    pub delta: f32,
    /// Seconds since the game started.
    pub elapsed: f32,
    pub frame: u64,
}
impl Time {
    pub(super) fn advance(&mut self, delta: f32) {
        self.delta = delta;
        self.elapsed += delta;
        self.frame += 1;
    }
}

struct BehaviourSlot {
    /// None while the behaviour is running (it is taken out so it can get `&mut Context`).
    behaviour: Option<Box<dyn Behaviour>>,
    started: bool,
}

pub struct GameObject {
    pub name: String,
    /// Authoritative transform; copied to the rendered instance every frame.
    pub transform: Transform,
    /// Collision shapes in local space (2D, on the xz plane).
    pub colliders: Vec<Collider>,
    instance: Option<InstanceKey>,
    behaviours: Vec<BehaviourSlot>,
    destroyed: bool,
}
impl GameObject {
    pub fn has_model(&self) -> bool {
        self.instance.is_some()
    }
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }
}

/// Everything the game owns: objects, the rendered scene and the camera.
pub struct World {
    objects: SlotMap<GameObjectId, GameObject>,
    pending_destroy: Vec<GameObjectId>,
    pub(super) scene: Scene,
    pub(super) camera: Camera,
    /// Set when a static instance is added, moved or removed, so the static
    /// instance buffer gets rebuilt.
    pub(super) static_dirty: bool,
    models: HashMap<String, ModelId>,
    /// Walls from each model's `COL_` nodes.
    model_walls: HashMap<ModelId, Arc<[[Vec3; 2]]>>,
    warned_no_walls: HashSet<ModelId>,
    prefabs: HashMap<String, PrefabDef>,
    prefab_dir: PathBuf,
}
impl World {
    pub(super) fn new(camera: Camera, prefab_dir: impl Into<PathBuf>) -> Self {
        Self {
            objects: SlotMap::with_key(),
            pending_destroy: Vec::new(),
            scene: Scene::new(),
            camera,
            static_dirty: true,
            models: HashMap::new(),
            model_walls: HashMap::new(),
            warned_no_walls: HashSet::new(),
            prefabs: HashMap::new(),
            prefab_dir: prefab_dir.into(),
        }
    }

    pub(super) fn object_ids(&self) -> Vec<GameObjectId> {
        self.objects.keys().collect()
    }

    pub(super) fn behaviour_count(&self, id: GameObjectId) -> usize {
        self.objects.get(id).map_or(0, |o| o.behaviours.len())
    }

    /// Takes the behaviour out of its slot if it should run in `phase`.
    pub(super) fn take_behaviour(
        &mut self,
        id: GameObjectId,
        index: usize,
        phase: Phase,
    ) -> Option<Box<dyn Behaviour>> {
        let slot = self.objects.get_mut(id)?.behaviours.get_mut(index)?;
        match phase {
            Phase::Start if slot.started => return None,
            Phase::Start => slot.started = true,
            // Behaviours added this frame wait for their start next frame.
            Phase::Update | Phase::LateUpdate if !slot.started => return None,
            _ => {}
        }
        slot.behaviour.take()
    }

    pub(super) fn put_behaviour(
        &mut self,
        id: GameObjectId,
        index: usize,
        behaviour: Box<dyn Behaviour>,
    ) {
        if let Some(slot) = self
            .objects
            .get_mut(id)
            .and_then(|o| o.behaviours.get_mut(index))
        {
            slot.behaviour = Some(behaviour);
        }
    }

    pub(super) fn take_pending_destroy(&mut self) -> Vec<GameObjectId> {
        std::mem::take(&mut self.pending_destroy)
    }

    /// Removes the object and its rendered instance.
    pub(super) fn remove(&mut self, id: GameObjectId) {
        let Some(object) = self.objects.remove(id) else {
            return;
        };
        if let Some(instance) = object.instance.and_then(|k| self.scene.instances.remove(k)) {
            if instance.mobility == Mobility::Static {
                self.static_dirty = true;
            }
        }
    }

    /// Copies object transforms to their rendered instances.
    pub(super) fn sync_transforms(&mut self) {
        for object in self.objects.values() {
            let Some(instance) = object.instance.and_then(|k| self.scene.instances.get_mut(k))
            else {
                continue;
            };
            let transform = object.transform.mat4();
            if instance.transform != transform {
                instance.transform = transform;
                if instance.mobility == Mobility::Static {
                    self.static_dirty = true;
                }
            }
        }
    }
}

/// Handle a behaviour uses to talk to the engine. Methods without an `id`
/// argument act on the object the running behaviour is attached to.
pub struct Context<'a> {
    pub(super) world: &'a mut World,
    pub(super) assets: &'a mut AssetManager,
    pub(super) gpu: &'a RenderContext,
    pub(super) input: &'a Input,
    pub(super) time: Time,
    pub(super) this: Option<GameObjectId>,
    pub(super) quit: &'a mut bool,
}

impl<'a> Context<'a> {
    // -----------------------------------------------------------------------
    // Engine state
    // -----------------------------------------------------------------------

    pub fn input(&self) -> &Input {
        self.input
    }
    pub fn time(&self) -> Time {
        self.time
    }
    pub fn camera(&self) -> &Camera {
        &self.world.camera
    }
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.world.camera
    }
    pub fn lights(&self) -> &[Light] {
        &self.world.scene.lights
    }
    pub fn lights_mut(&mut self) -> &mut Vec<Light> {
        &mut self.world.scene.lights
    }
    /// Closes the game at the end of the frame.
    pub fn quit(&mut self) {
        *self.quit = true;
    }

    // -----------------------------------------------------------------------
    // The object running this behaviour
    // -----------------------------------------------------------------------

    /// Id of the object this behaviour is attached to.
    ///
    /// Panics when called from the setup function, which has no object.
    pub fn this(&self) -> GameObjectId {
        self.this
            .expect("Context::this() called outside of a behaviour")
    }
    pub fn transform(&self) -> &Transform {
        &self.world.objects[self.this()].transform
    }
    pub fn transform_mut(&mut self) -> &mut Transform {
        let id = self.this();
        &mut self.world.objects[id].transform
    }

    // -----------------------------------------------------------------------
    // Other objects
    // -----------------------------------------------------------------------

    pub fn object(&self, id: GameObjectId) -> Option<&GameObject> {
        self.world.objects.get(id)
    }
    pub fn object_mut(&mut self, id: GameObjectId) -> Option<&mut GameObject> {
        self.world.objects.get_mut(id)
    }
    /// First object with this name.
    pub fn find(&self, name: &str) -> Option<GameObjectId> {
        self.world
            .objects
            .iter()
            .find(|(_, o)| o.name == name && !o.destroyed)
            .map(|(id, _)| id)
    }
    pub fn find_all(&self, name: &str) -> Vec<GameObjectId> {
        self.world
            .objects
            .iter()
            .filter(|(_, o)| o.name == name && !o.destroyed)
            .map(|(id, _)| id)
            .collect()
    }

    /// First behaviour of type `T` on the object. Returns None for the
    /// behaviour that is currently running, since it is borrowed as `self`.
    pub fn get_behaviour<T: Behaviour>(&self, id: GameObjectId) -> Option<&T> {
        self.world.objects.get(id)?.behaviours.iter().find_map(|slot| {
            let b: &dyn Any = slot.behaviour.as_deref()?;
            b.downcast_ref::<T>()
        })
    }
    pub fn get_behaviour_mut<T: Behaviour>(&mut self, id: GameObjectId) -> Option<&mut T> {
        self.world
            .objects
            .get_mut(id)?
            .behaviours
            .iter_mut()
            .find_map(|slot| {
                let b: &mut dyn Any = slot.behaviour.as_deref_mut()?;
                b.downcast_mut::<T>()
            })
    }
    pub fn add_behaviour(&mut self, id: GameObjectId, behaviour: impl Behaviour) {
        if let Some(object) = self.world.objects.get_mut(id) {
            object.behaviours.push(BehaviourSlot {
                behaviour: Some(Box::new(behaviour)),
                started: false,
            });
        }
    }

    // -----------------------------------------------------------------------
    // Collision
    // -----------------------------------------------------------------------

    /// Objects whose colliders overlap `circle`, except the running behaviour's own.
    pub fn overlapping(&self, circle: Circle) -> Vec<GameObjectId> {
        let mut found = Vec::new();
        self.for_each_hitbox(|id, hitbox| {
            if found.last() != Some(&id) && circle_push(&circle, hitbox).is_some() {
                found.push(id);
            }
        });
        found
    }

    /// Moves `circle` out of every collider it overlaps, except the running
    /// behaviour's own, and returns its corrected center.
    pub fn resolve_circle(&self, mut circle: Circle) -> Vec2 {
        // Pushing out of one collider can push into another: repeat a few times.
        for _ in 0..4 {
            let mut moved = false;
            self.for_each_hitbox(|_, hitbox| {
                if let Some(push) = circle_push(&circle, hitbox) {
                    circle.center += push;
                    moved = true;
                }
            });
            if !moved {
                break;
            }
        }
        circle.center
    }

    fn for_each_hitbox(&self, mut f: impl FnMut(GameObjectId, &Hitbox)) {
        let mut hitboxes = Vec::new();
        for (id, object) in &self.world.objects {
            if Some(id) == self.this || object.destroyed || object.colliders.is_empty() {
                continue;
            }
            hitboxes.clear();
            let world = object.transform.mat4();
            for collider in &object.colliders {
                collider.hitboxes(world, &mut hitboxes);
            }
            for hitbox in &hitboxes {
                f(id, hitbox);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Spawning and destroying
    // -----------------------------------------------------------------------

    /// Creates an empty object. Configure it with the returned builder.
    pub fn spawn(&mut self, name: impl Into<String>) -> ObjectBuilder<'_, 'a> {
        let id = self.world.objects.insert(GameObject {
            name: name.into(),
            transform: Transform::IDENTITY,
            colliders: Vec::new(),
            instance: None,
            behaviours: Vec::new(),
            destroyed: false,
        });
        ObjectBuilder { ctx: self, id }
    }

    /// Spawns `prefabs/<name>.toml` with the given transform.
    pub fn spawn_prefab(
        &mut self,
        name: &str,
        transform: Transform,
    ) -> anyhow::Result<GameObjectId> {
        let mobility = self.prefab(name)?.mobility;
        self.spawn_prefab_with(name, transform, convert_mobility(mobility))
    }

    /// Spawns every instance in a level file. Returns the spawned objects.
    pub fn load_level(&mut self, path: impl AsRef<Path>) -> anyhow::Result<Vec<GameObjectId>> {
        let level = LevelDef::load(path.as_ref())?;
        let mut ids = Vec::with_capacity(level.instances.len());
        for def in &level.instances {
            let mobility = def.mobility(self.prefab(&def.prefab)?);
            let transform = Transform::from(def.transform());
            ids.push(self.spawn_prefab_with(&def.prefab, transform, convert_mobility(mobility))?);
        }
        Ok(ids)
    }

    /// Marks the object for removal at the end of the frame, after its
    /// behaviours' `on_destroy`.
    pub fn destroy(&mut self, id: GameObjectId) {
        if let Some(object) = self.world.objects.get_mut(id) {
            if !object.destroyed {
                object.destroyed = true;
                self.world.pending_destroy.push(id);
            }
        }
    }

    fn spawn_prefab_with(
        &mut self,
        name: &str,
        transform: Transform,
        mobility: Mobility,
    ) -> anyhow::Result<GameObjectId> {
        let prefab = self.prefab(name)?;
        let model = prefab.model.clone();
        let from_model = prefab.colliders == Some(ColliderSource::Model);
        let colliders: Vec<_> = prefab.collider_defs.iter().map(convert_collider).collect();

        let mut object = self
            .spawn(name)
            .with_transform(transform)
            .with_model(&model, mobility)?;
        if from_model {
            object = object.with_model_colliders();
        }
        for collider in colliders {
            object = object.with_collider(collider);
        }
        Ok(object.id())
    }

    fn prefab(&mut self, name: &str) -> anyhow::Result<&PrefabDef> {
        if !self.world.prefabs.contains_key(name) {
            let path = self.world.prefab_dir.join(format!("{name}.toml"));
            let def = PrefabDef::load(&path)
                .map_err(|e| e.context(format!("loading prefab {}", path.display())))?;
            self.world.prefabs.insert(name.to_owned(), def);
        }
        Ok(&self.world.prefabs[name])
    }

    fn load_model(&mut self, path: &str) -> anyhow::Result<ModelId> {
        if let Some(&model) = self.world.models.get(path) {
            return Ok(model);
        }
        let model = self.assets.load_glb_model(self.gpu, path)?;
        self.world.models.insert(path.to_owned(), model);

        let walls: Arc<[[Vec3; 2]]> = self
            .assets
            .get_model(model)
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::Collider(walls) => Some(walls.iter().copied()),
                _ => None,
            })
            .flatten()
            .collect();
        self.world.model_walls.insert(model, walls);
        Ok(model)
    }
}

/// Returned by `Context::spawn` to configure the new object.
pub struct ObjectBuilder<'c, 'a> {
    ctx: &'c mut Context<'a>,
    id: GameObjectId,
}

impl ObjectBuilder<'_, '_> {
    pub fn id(&self) -> GameObjectId {
        self.id
    }
    pub fn with_transform(self, transform: Transform) -> Self {
        self.ctx.world.objects[self.id].transform = transform;
        self
    }
    pub fn with_position(self, position: Vec3) -> Self {
        self.ctx.world.objects[self.id].transform.position = position;
        self
    }
    pub fn with_rotation(self, rotation: Quat) -> Self {
        self.ctx.world.objects[self.id].transform.rotation = rotation;
        self
    }
    pub fn with_scale(self, scale: Vec3) -> Self {
        self.ctx.world.objects[self.id].transform.scale = scale;
        self
    }
    /// Renders a `.glb` model (path relative to the assets directory) at the
    /// object's transform. Static models are cheaper but shouldn't move.
    pub fn with_model(self, path: &str, mobility: Mobility) -> anyhow::Result<Self> {
        let model = self.ctx.load_model(path)?;
        let world = &mut *self.ctx.world;
        let object = &mut world.objects[self.id];
        if let Some(old) = object.instance.and_then(|k| world.scene.instances.remove(k)) {
            world.static_dirty |= old.mobility == Mobility::Static;
        }
        object.instance = Some(world.scene.instances.insert(Instance {
            model,
            transform: object.transform.mat4(),
            mobility,
            bodies: Vec::new(),
            node_overrides: None,
        }));
        world.static_dirty |= mobility == Mobility::Static;
        Ok(self)
    }
    pub fn with_collider(self, collider: Collider) -> Self {
        self.ctx.world.objects[self.id].colliders.push(collider);
        self
    }
    /// Uses the walls from the `COL_` nodes of the object's model (set it
    /// first with `with_model`). Logs a warning if the model has none.
    pub fn with_model_colliders(self) -> Self {
        let world = &mut *self.ctx.world;
        let object = &mut world.objects[self.id];
        let model = object
            .instance
            .and_then(|k| world.scene.instances.get(k))
            .map(|instance| instance.model);
        match model.and_then(|m| world.model_walls.get(&m)) {
            Some(walls) if !walls.is_empty() => {
                object.colliders.push(Collider::Walls(walls.clone()))
            }
            _ => {
                if model.is_none_or(|m| world.warned_no_walls.insert(m)) {
                    log::warn!("{}: model has no COL_ collider nodes", object.name);
                }
            }
        }
        self
    }
    pub fn with_behaviour(self, behaviour: impl Behaviour) -> Self {
        self.ctx.add_behaviour(self.id, behaviour);
        self
    }
}

fn convert_collider(def: &ColliderDef) -> Collider {
    let shape = match def.shape {
        ShapeDef::Box { half_extents } => Shape::Box {
            half_extents: Vec3::from_array(half_extents),
        },
        ShapeDef::Sphere { radius } => Shape::Sphere { radius },
        ShapeDef::Capsule { radius, height } => Shape::Capsule { radius, height },
        ShapeDef::Cylinder { radius, height } => Shape::Cylinder { radius, height },
    };
    Collider::Shape {
        shape,
        offset: Vec3::from_array(def.offset),
        rotation: Quat::from_array(def.rotation).normalize(),
    }
}

fn convert_mobility(mobility: level::Mobility) -> Mobility {
    match mobility {
        level::Mobility::Static => Mobility::Static,
        level::Mobility::Dynamic => Mobility::Dynamic,
    }
}
