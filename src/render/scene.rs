use crate::render::asset_id::*;
use crate::render::light::Light;
use glam::prelude::*;
use slotmap::{SlotMap, new_key_type};

new_key_type! { pub struct InstanceKey; }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mobility {
    Static,  // trees, rocks, houses: never moves after load
    Dynamic, // doors, physics props, anything that moves or is spawned
}

pub struct Instance {
    pub model: ModelId,
    pub transform: Mat4,
    pub mobility: Mobility,
    pub bodies: Vec<RigidBodyHandle>, // physics colliders for this instance
    pub node_overrides: Option<Vec<Mat4>>, // per-instance node transforms (opening door)
}

pub struct Scene {
    pub instances: SlotMap<InstanceKey, Instance>,
    pub lights: Vec<Light>,
    pub spawn_point: Vec3,
    dynamic_dirty: bool, // set when a dynamic instance changes
}
impl Scene {
    pub fn new() -> Self {
        Self {
            instances: SlotMap::with_key(),
            lights: Vec::new(),
            spawn_point: Vec3::ZERO,
            dynamic_dirty: true,
        }
    }
}

// dummy struct
pub struct RigidBodyHandle;
