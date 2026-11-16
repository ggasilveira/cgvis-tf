use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct InstanceKey; }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mobility {
    Static,  // trees, rocks, houses: never moves after load
    Dynamic, // doors, physics props, anything that moves or is spawned
}

pub struct Instance {
    pub model: ModelHandle,
    pub transform: Mat4,
    pub mobility: Mobility,
    pub bodies: Vec<RigidBodyHandle>, // physics colliders for this instance
    pub node_overrides: Option<Vec<Mat4>>, // per-instance node transforms (opening door)
}

pub struct Scene {
    pub instances: SlotMap<InstanceKey, Instance>,
    pub lights: Vec<Light>,
    pub spawn_points: Vec<SpawnPoint>,
    dynamic_dirty: bool, // set when a dynamic instance changes
}
