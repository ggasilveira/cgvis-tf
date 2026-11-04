use crate::low::resources::ModelHandle;
use glam::prelude::*;

pub struct EngineApi {}
impl EngineApi {
    pub fn load_model(&self, filename: &str) -> ModelHandle {
        todo!()
    }
}
pub struct RenderApi<'a> {
    engine: &'a super::Engine,
}
impl<'a> RenderApi<'a> {
    pub fn draw_model(&self, model: ModelHandle, position: Vec3, rotation: Quat) {}
}

pub trait Game {
    fn setup(&mut self, api: &EngineApi);
    fn update(&mut self, api: &EngineApi);
    fn render(&self, api: &RenderApi);
}
