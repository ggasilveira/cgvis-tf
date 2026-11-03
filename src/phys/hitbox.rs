#[derive(Debug, Clone, Copy)]
pub enum Hitbox {
    Sphere { radius: f32 },
    Box { width: f32, height: f32 },
}
