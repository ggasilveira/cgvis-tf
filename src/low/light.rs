#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightUniform {
    pub position: [f32; 3],
    // padding needed because uniforms are 16 byte aligned
    pub _padding: u32,
    pub color: [f32; 3],
    // padding needed because uniforms are 16 byte aligned
    pub _padding2: u32,
}
