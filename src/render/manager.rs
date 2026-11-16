use glam::Vec3;

use super::pipeline::MaterialPipelines;
use super::vertex::*;
use crate::render::asset_manager::AssetManager;
use crate::render::bindgroup::{
    GlobalBindGroup, GlobalBindGroupLayout, InstanceBufferBindGroupLayout, MaterialBindGroupLayout,
};
use crate::render::buffer::Buffer;
use crate::render::camera::Camera;
use crate::render::context::RenderContext;
use crate::render::light::Light;
use crate::render::model;
use crate::render::pipeline::MaterialPipelineLayout;
use crate::render::scene::Scene;
use crate::render::scene_renderer::SceneRenderer;
use crate::render::texture;
use crate::render::uniform::{CameraUniform, LightsUniform};
use crate::render::{instance_buffer::*, scene_renderer};

pub struct RenderManager {
    is_surface_configured: bool,
    pub scene_render: SceneRenderer,
    camera_buffer: Buffer<CameraUniform>,
    depth_texture: texture::Texture,
    lights_uniform: LightsUniform,
    lights_buffer: Buffer<LightsUniform>,
    global_bg: GlobalBindGroup,
    instance_buffer_bind_group_layout: InstanceBufferBindGroupLayout,
}

impl RenderManager {
    pub fn new(context: &RenderContext) -> anyhow::Result<Self> {
        let camera_buffer = Buffer::new(&context.device, &CameraUniform::new());
        let lights_uniform = LightsUniform::new();
        let lights_buffer = Buffer::new(&context.device, &lights_uniform);

        let material_bind_group_layout = MaterialBindGroupLayout::new(&context.device);
        let global_bind_group_layout = GlobalBindGroupLayout::new(&context.device);
        let instance_buffer_bind_group_layout = InstanceBufferBindGroupLayout::new(&context.device);
        let global_bg = GlobalBindGroup::new(
            &context.device,
            &global_bind_group_layout,
            &camera_buffer,
            &lights_buffer,
        );

        let material_pipeline_layout = MaterialPipelineLayout::new(
            &context.device,
            &global_bind_group_layout,
            &material_bind_group_layout,
            &instance_buffer_bind_group_layout,
        );

        let depth_texture = texture::Texture::create_depth_texture(context, "depth_texture");

        let pipelines = {
            let shader = context
                .device
                .create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
            MaterialPipelines::new(
                &context.device,
                &shader,
                &material_pipeline_layout,
                &[Some(ModelVertex::desc())],
                context.config.format,
            )
        };
        let scene_render = SceneRenderer::new(context, pipelines);

        Ok(Self {
            is_surface_configured: false,
            global_bg,
            camera_buffer,
            lights_uniform,
            lights_buffer,
            depth_texture,
            scene_render,
            instance_buffer_bind_group_layout,
        })
    }
    pub fn resize(&mut self, context: &mut RenderContext, width: u32, height: u32) {
        if width > 0 && height > 0 {
            context.config.width = width;
            context.config.height = height;
            context.surface.configure(&context.device, &context.config);
            self.is_surface_configured = true;
            self.depth_texture = texture::Texture::create_depth_texture(context, "depth_texture");
        }
    }
    pub fn update_camera(&mut self, context: &RenderContext, camera: &Camera) {
        self.camera_buffer
            .update(&context.queue, &CameraUniform::from_camera(camera));
    }
    pub fn update_lights(&mut self, context: &RenderContext, lights: &[Light]) {
        self.lights_uniform.update(lights);
        self.lights_buffer
            .update(&context.queue, &self.lights_uniform);
    }

    pub fn render(
        &mut self,
        context: &RenderContext,
        assets: &AssetManager,
        scene: &Scene,
        camera_pos: Vec3,
    ) -> anyhow::Result<()> {
        context.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        }

        let output = match context.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_txt) => surface_txt,
            wgpu::CurrentSurfaceTexture::Suboptimal(surface_txt) => surface_txt,
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                context.surface.configure(&context.device, &context.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                anyhow::bail!("Lost device");
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        self.scene_render.update(context, assets, scene, camera_pos);
        render_pass.set_bind_group(GlobalBindGroup::GROUP, self.global_bg.bind_group(), &[]);
        self.scene_render.draw_solid(&mut render_pass, assets);
        self.scene_render.draw_transparent(&mut render_pass, assets);

        drop(render_pass);

        context.queue.submit(std::iter::once(encoder.finish()));
        context.queue.present(output);
        Ok(())
    }
}
