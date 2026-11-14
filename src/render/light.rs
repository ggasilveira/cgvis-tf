use glam::Vec3;

#[derive(Debug, Clone)]
pub enum Light {
    Point(PointLight),
    Spot(SpotLight),
    Directional(DirectionalLight),
}

#[derive(Debug, Clone)]
pub struct PointLight {
    pub position: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}

#[derive(Debug, Clone)]
pub struct SpotLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
}
impl SpotLight {}
#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub color: Vec3,
    pub intensity: f32,
}
impl DirectionalLight {}
