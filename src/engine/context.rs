use crate::engine::input::Input;

pub struct EngineContext {
    input: Input,
}
impl EngineContext {
    pub fn input(&self) -> &Input {
        &self.input
    }
}
