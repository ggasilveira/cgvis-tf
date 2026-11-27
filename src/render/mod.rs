pub mod asset_id;
pub mod asset_manager;
pub mod bindgroup;
pub mod buffer;
pub mod camera;
pub mod context;
pub mod grass;
pub mod instance_buffer;
pub mod light;
pub mod manager;
pub mod material;
pub mod model;
pub mod pipeline;
pub mod scene;
pub mod scene_renderer;
pub mod skybox;
pub mod texture;
pub mod uniform;
pub mod vertex;

/// WGSL has no includes: prepends the shared definitions (camera, sky, fog).
pub fn shader_source(source: &str) -> wgpu::ShaderSource<'static> {
    wgpu::ShaderSource::Wgsl(format!("{}\n{}", include_str!("common.wgsl"), source).into())
}
