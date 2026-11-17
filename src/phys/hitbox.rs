use glam::Vec2;

/// círculo no plano xz, em coordenadas do mundo
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub center: Vec2,
    pub radius: f32,
}

/// retângulo alinhado aos eixos xz, em coordenadas do mundo.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub center: Vec2,
    pub half_extents: Vec2,
}

/// segmento de parede no plano xz, em coordenadas do mundo.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub a: Vec2,
    pub b: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub enum Hitbox {
    Circle(Circle),
    Rect(Rect),
    Segment(Segment),
}
