use super::context::EngineContext;
use glam::prelude::*;

pub trait Object {
    fn position(&self) -> Vec3;
    fn rotation(&self) -> Quat;
    fn update(&mut self, context: &EngineContext);
}
