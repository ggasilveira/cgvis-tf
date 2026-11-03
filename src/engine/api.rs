use glam::prelude::*;

pub struct EngineApi {}
pub struct ModelHandle;
impl EngineApi {
    pub fn load_model(&self, filename: &str) -> ModelHandle {
        todo!()
    }
    pub fn draw_model(&self, model: ModelHandle, position: Vec3, rotation: Quat, scale: Vec3) {}
}
