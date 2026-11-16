use super::asset_id::TextureId;
use super::asset_manager::AssetManager;
use super::bindgroup::{MaterialBindGroup, MaterialBindGroupLayout};
use super::buffer::Buffer;
use super::context::RenderContext;
use super::uniform::MaterialUniform;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

#[derive(Debug, Clone)]
pub struct Material {
    pub base_color_texture: TextureId,
    pub normal_texture: TextureId,
    pub properties: MaterialUniform,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    pub bind_group: MaterialBindGroup,
    pub uniform_buffer: Buffer<MaterialUniform>,
}
impl Material {
    pub const DEFAULT_SETTINGS: MaterialUniform = MaterialUniform {
        base_color: [1.; 4],
        metallic: 0.,
        roughness: 1.,
        normal_scale: 1.,
        alpha_cutoff: 0.,
    };
    pub fn new(
        ctx: &RenderContext,
        assets: &AssetManager,
        layout: &MaterialBindGroupLayout,
        properties: MaterialUniform,
        alpha_mode: AlphaMode,
        double_sided: bool,
        base_color_texture: TextureId,
        normal_texture: TextureId,
    ) -> Self {
        let uniform_buffer = Buffer::new(&ctx.device, &properties);
        let bind_group = MaterialBindGroup::new(
            &ctx.device,
            layout,
            &uniform_buffer,
            assets.get_texture(base_color_texture),
            assets.get_texture(normal_texture),
        );
        Self {
            base_color_texture,
            normal_texture,
            alpha_mode,
            properties,
            uniform_buffer,
            bind_group,
            double_sided,
        }
    }
}
