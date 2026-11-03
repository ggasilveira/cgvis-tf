use crate::phys;
use crate::resources::ModelHandle;
use glam::prelude::*;

// #[derive(Debug)]
// pub struct Object {
//     pub position: Vec3,
//     pub rotation: Quat,
//     pub model: ModelHandle,
//     pub hitbox: phys::hitbox::Hitbox,
// }

pub trait Object {
    fn update(&mut self, context: ());
}
