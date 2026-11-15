use crate::render::bindgroup::{
    GlobalBindGroup, GlobalBindGroupLayout, InstanceBufferBindGroupLayout, MaterialBindGroupLayout,
};
use crate::render::buffer::Buffer;
use crate::render::camera::Camera;
use crate::render::context::RenderContext;
use crate::render::instance::*;
use crate::render::light::Light;
use crate::render::model;
use crate::render::resources::{AlphaMode, ResourceManager};
use crate::render::texture;
use crate::render::uniform::{CameraUniform, LightsUniform};
use model::Vertex;

pub struct RenderManager {
    is_surface_configured: bool,
    render_pipeline: wgpu::RenderPipeline,
    camera_buffer: Buffer<CameraUniform>,
    depth_texture: texture::Texture,
    lights_uniform: LightsUniform,
    lights_buffer: Buffer<LightsUniform>,
    global_bg: GlobalBindGroup,
    pub static_instance_buffer: InstanceBuffer,
    pub dyn_instance_buffer: InstanceBuffer,
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
        let dyn_instance_buffer =
            InstanceBuffer::new(&context.device, &instance_buffer_bind_group_layout, 64);
        let static_instance_buffer =
            InstanceBuffer::new(&context.device, &instance_buffer_bind_group_layout, 64);
        let global_bg = GlobalBindGroup::new(
            &context.device,
            &global_bind_group_layout,
            &camera_buffer,
            &lights_buffer,
        );

        let render_pipeline_layout = RenderPipelineLayout::new(
            &context.device,
            &global_bind_group_layout,
            &material_bind_group_layout,
            &instance_buffer_bind_group_layout,
        );

        let depth_texture = texture::Texture::create_depth_texture(context, "depth_texture");

        let render_pipeline = {
            let shader = wgpu::include_wgsl!("shader.wgsl");
            create_render_pipeline(
                &context.device,
                &render_pipeline_layout,
                context.config.format,
                Some(texture::Texture::DEPTH_FORMAT),
                &[Some(model::ModelVertex::desc())],
                shader,
            )
        };

        Ok(Self {
            is_surface_configured: false,
            render_pipeline,
            global_bg,
            camera_buffer,
            lights_uniform,
            lights_buffer,
            depth_texture,
            static_instance_buffer,
            dyn_instance_buffer,
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
    pub fn set_dynamic_instances(
        &mut self,
        context: &RenderContext,
        resources: &ResourceManager,
        instances: &[Instance],
    ) {
        self.dyn_instance_buffer.rebuild(
            &context.device,
            &context.queue,
            &self.instance_buffer_bind_group_layout,
            instances,
            resources,
        );
    }
    pub fn set_static_instances(
        &mut self,
        context: &RenderContext,
        resources: &ResourceManager,
        instances: &[Instance],
    ) {
        self.static_instance_buffer.rebuild(
            &context.device,
            &context.queue,
            &self.instance_buffer_bind_group_layout,
            instances,
            resources,
        );
    }

    pub fn render(
        &mut self,
        context: &RenderContext,
        resources: &ResourceManager,
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

        render_pass.set_pipeline(&self.render_pipeline);

        render_pass.set_bind_group(GlobalBindGroup::GROUP, self.global_bg.bind_group(), &[]);

        self.static_instance_buffer
            .draw(&mut render_pass, resources);
        self.dyn_instance_buffer.draw(&mut render_pass, resources);

        drop(render_pass);

        context.queue.submit(std::iter::once(encoder.finish()));
        context.queue.present(output);
        Ok(())
    }
}
