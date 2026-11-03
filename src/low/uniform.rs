use crate::low::camera::Camera;
use crate::low::light::{DirectionalLight, Light, PointLight, SpotLight};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_position: [f32; 4],
    view_proj: [[f32; 4]; 4],
}
impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_position: [0.; 4],
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
        }
    }
    pub fn update(&mut self, cam: &Camera) {
        // change this to `rows` if the uniform is row major
        self.view_position = cam.eye.extend(1.).to_array();
        self.view_proj = cam.build_matrix().to_cols_array_2d();
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct LightUniform {
    // 16 bytes
    pub position: [f32; 3],
    pub typ: u32,

    // 16 bytes
    pub direction: [f32; 3],
    padding0: u32,

    // 16 bytes
    pub color: [f32; 3],
    pub intensity: f32,

    // 16 bytes
    pub inner_angle: f32,
    pub outer_angle: f32,
    padding1: u32,
    padding2: u32,
}
impl LightUniform {
    pub fn new(light: &Light) -> Self {
        match light {
            Light::Point(pt) => LightUniform {
                position: pt.position.to_array(),
                typ: 1,
                direction: [0.0; 3],
                padding0: 0,
                color: pt.color.to_array(),
                intensity: pt.intensity,
                inner_angle: 0.0,
                outer_angle: 0.0,
                padding1: 0,
                padding2: 0,
            },
            Light::Directional(dir) => LightUniform {
                position: [0.; 3],
                typ: 0,
                direction: dir.direction.to_array(),
                padding0: 0,
                color: dir.color.to_array(),
                intensity: dir.intensity,
                inner_angle: 0.0,
                outer_angle: 0.0,
                padding1: 0,
                padding2: 0,
            },
            Light::Spot(spot) => LightUniform {
                position: spot.position.to_array(),
                typ: 2,
                direction: spot.direction.to_array(),
                padding0: 0,
                color: spot.color.to_array(),
                intensity: spot.intensity,
                inner_angle: spot.inner_angle,
                outer_angle: spot.outer_angle,
                padding1: 0,
                padding2: 0,
            },
        }
    }
}

pub const LIGHTS_SIZE: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightsUniform {
    lights: [LightUniform; LIGHTS_SIZE],
    size: u32,
}
impl LightsUniform {
    pub fn new() -> Self {
        Self {
            lights: [LightUniform::default(); LIGHTS_SIZE],
            size: LIGHTS_SIZE as u32,
        }
    }
    pub fn update(&mut self, lights: &[Light]) {
        assert!(lights.len() <= LIGHTS_SIZE, "maximum light limit exceeded");
        self.size = lights.len() as u32;
        for (i, light) in lights.iter().enumerate() {
            self.lights[i] = LightUniform::new(light);
        }
    }
}
