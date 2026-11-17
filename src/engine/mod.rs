mod api;
mod input;
mod level;

use crate::render::camera::Camera;
use crate::render::light::{DirectionalLight, Light};
use crate::render::manager::RenderManager;
use std::time::Instant;

use crate::render::asset_manager::AssetManager;
use crate::render::context::RenderContext;
pub use api::{Behaviour, Context, GameObject, GameObjectId, ObjectBuilder, Setup, Time};
use api::{Phase, World};
use glam::{Vec2, Vec3};
pub use input::Input;
use winit::event::{DeviceEvent, DeviceId, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::PhysicalKey;

/// Everything a game needs: `use slender::engine::prelude::*;`
pub mod prelude {
    pub use super::{Behaviour, Context, GameObject, GameObjectId, Input, Time};
    pub use crate::math::Transform;
    pub use crate::phys::collider::{Collider, Shape};
    pub use crate::phys::hitbox::{Circle, Hitbox, Rect};
    pub use crate::render::camera::Camera;
    pub use crate::render::light::{DirectionalLight, Light, PointLight, SpotLight};
    pub use crate::render::scene::Mobility;
    pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3};
    pub use winit::event::MouseButton;
    pub use winit::keyboard::KeyCode;
}

pub struct Engine {
    context: RenderContext,
    input: Input,
    render: RenderManager,
    assets: AssetManager,
    world: World,
    time: Time,
    quit: bool,
    last_frame: Instant,
    mouse_delta: Vec2,
}
impl Engine {
    pub fn new(window: winit::window::Window, setup: Setup) -> anyhow::Result<Self> {
        window.set_cursor_grab(winit::window::CursorGrabMode::Locked)?;
        let context = RenderContext::new(window)?;
        let render = RenderManager::new(&context)?;
        let assets = AssetManager::new(&context, "assets")?;
        let camera = Camera {
            eye: glam::Vec3::new(0.0, 1.0, 2.0),
            target: glam::Vec3::ZERO,
            aspect: context.config.width as f32 / context.config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        let mut world = World::new(camera, "prefabs");
        world
            .scene
            .lights
            .push(Light::Directional(DirectionalLight {
                direction: Vec3::NEG_Y,
                color: Vec3::new(1., 1., 1.),
                intensity: 1.,
            }));
        let mut engine = Self {
            context,
            render,
            input: Input::new(),
            assets,
            world,
            time: Time::default(),
            quit: false,
            last_frame: Instant::now(),
            mouse_delta: Vec2::ZERO,
        };
        setup(&mut Context {
            world: &mut engine.world,
            assets: &mut engine.assets,
            gpu: &engine.context,
            input: &engine.input,
            time: engine.time,
            this: None,
            quit: &mut engine.quit,
        })?;
        engine.world.sync_transforms();
        // Loading took a while; don't count it as the first frame's delta.
        engine.last_frame = Instant::now();
        Ok(engine)
    }
    pub fn device_event(
        &mut self,
        device_id: DeviceId,
        event: DeviceEvent,
        event_loop: &ActiveEventLoop,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.mouse_delta.x += dx as f32;
            self.mouse_delta.y += dy as f32;
        }
    }
    pub fn window_event(&mut self, event: WindowEvent, event_loop: &ActiveEventLoop) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.render
                    .resize(&mut self.context, size.width, size.height);
                if size.height > 0 {
                    self.world.camera.aspect = size.width as f32 / size.height as f32;
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta = now - self.last_frame;
                self.last_frame = now;
                self.input.set_mouse_delta(self.mouse_delta);
                self.mouse_delta = Vec2::ZERO;
                self.update(delta.as_secs_f32());
                if self.quit {
                    event_loop.exit();
                    return;
                }

                match self.render.render(
                    &self.context,
                    &self.assets,
                    &self.world.scene,
                    self.world.camera.eye,
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("{e}");
                        event_loop.exit();
                    }
                }
                self.context.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                if state.is_pressed() {
                    self.input.press_key(code);
                } else {
                    self.input.release_key(code);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() {
                    self.input.press_mouse(button);
                } else {
                    self.input.release_mouse(button);
                }
            }
            _ => {}
        }
    }
    fn update(&mut self, delta: f32) {
        self.time.advance(delta);

        let ids = self.world.object_ids();
        for phase in [Phase::Start, Phase::Update, Phase::LateUpdate] {
            for &id in &ids {
                self.run_behaviours(id, phase);
            }
        }

        // Destroying can destroy more objects (from on_destroy), so loop.
        loop {
            let pending = self.world.take_pending_destroy();
            if pending.is_empty() {
                break;
            }
            for id in pending {
                self.run_behaviours(id, Phase::Destroy);
                self.world.remove(id);
            }
        }

        self.world.sync_transforms();
        if self.world.static_dirty {
            self.render
                .scene_render
                .load_level(&self.context, &self.assets, &self.world.scene);
            self.world.static_dirty = false;
        }
        self.render.update_camera(&self.context, &self.world.camera);
        self.render
            .update_lights(&self.context, &self.world.scene.lights);
        self.input.end_frame();
    }

    /// Runs one lifecycle method on every behaviour of an object. Each
    /// behaviour is taken out of the object while it runs so it can receive
    /// a `&mut Context` that also reaches the object.
    fn run_behaviours(&mut self, id: GameObjectId, phase: Phase) {
        // The count is re-read so behaviours added during the loop are seen.
        let mut index = 0;
        while index < self.world.behaviour_count(id) {
            if let Some(mut behaviour) = self.world.take_behaviour(id, index, phase) {
                let mut ctx = Context {
                    world: &mut self.world,
                    assets: &mut self.assets,
                    gpu: &self.context,
                    input: &self.input,
                    time: self.time,
                    this: Some(id),
                    quit: &mut self.quit,
                };
                match phase {
                    Phase::Start => behaviour.start(&mut ctx),
                    Phase::Update => behaviour.update(&mut ctx),
                    Phase::LateUpdate => behaviour.late_update(&mut ctx),
                    Phase::Destroy => behaviour.on_destroy(&mut ctx),
                }
                self.world.put_behaviour(id, index, behaviour);
            }
            index += 1;
        }
    }
}
