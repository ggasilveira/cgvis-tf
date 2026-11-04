use crate::low::buffer::Buffer;
use crate::low::camera::{Camera, CameraController};
use crate::low::context::RenderContext;
use crate::low::layout::{GlobalBindGroup, GlobalBindGroupLayout, RenderPipelineLayout};
use crate::low::model;
use crate::low::resources::{MeshHandle, ModelHandle, ResourceManager};
use crate::low::texture;
use crate::low::uniform::{CameraUniform, LightsUniform};
use crate::low::{instance::*, layout};
use model::Vertex;
use std::collections::HashMap;

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &RenderPipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[Option<wgpu::VertexBufferLayout>],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout.layout()),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: vertex_layouts,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

pub struct RenderState {
    is_surface_configured: bool,
    render_pipeline: wgpu::RenderPipeline,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: Buffer<CameraUniform>,
    global_bind_group: GlobalBindGroup,
    camera_controller: CameraController,
    depth_texture: texture::Texture,
    lights_uniform: LightsUniform,
    lights_buffer: Buffer<LightsUniform>,
    //light_render_pipeline: wgpu::RenderPipeline,
}

impl RenderState {
    pub fn new(context: &RenderContext) -> anyhow::Result<Self> {
        let material_bind_group_layout = layout::MaterialBindGroupLayout::new(&context.device);

        let camera = Camera {
            eye: glam::Vec3::new(0.0, 1.0, 2.0),
            target: glam::Vec3::ZERO,
            aspect: context.config.width as f32 / context.config.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update(&camera);
        let camera_buffer = Buffer::new(&context.device, &camera_uniform);
        let lights_uniform = LightsUniform::new();
        let lights_buffer = Buffer::new(&context.device, &lights_uniform);

        let global_bind_group_layout = GlobalBindGroupLayout::new(&context.device);
        let global_bind_group = GlobalBindGroup::new(
            &context.device,
            &global_bind_group_layout,
            &camera_buffer,
            &lights_buffer,
        );

        let render_pipeline_layout = RenderPipelineLayout::new(
            &context.device,
            &global_bind_group_layout,
            &material_bind_group_layout,
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
        // lib.rs
        // let light_render_pipeline = {
        //     let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        //         label: Some("Light Pipeline Layout"),
        //         bind_group_layouts: &[Some(&cam_bind_group_layout), Some(&light_bind_group_layout)],
        //         immediate_size: 0,
        //     });
        //     let shader = wgpu::ShaderModuleDescriptor {
        //         label: Some("Light Shader"),
        //         source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
        //     };
        //     create_render_pipeline(
        //         &device,
        //         &layout,
        //         config.format,
        //         Some(texture::Texture::DEPTH_FORMAT),
        //         &[Some(model::ModelVertex::desc())],
        //         shader,
        //     )
        // };

        // let obj_model =
        //     resources::load_model("data/cube.obj", &device, &queue, &texture_bind_group_layout)
        //         .unwrap();

        Ok(Self {
            is_surface_configured: false,
            render_pipeline,
            global_bind_group,
            camera,
            camera_uniform,
            camera_buffer,
            lights_uniform,
            lights_buffer,
            camera_controller: CameraController::new(0.2),
            depth_texture,
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
    pub fn update_cam_uniform(&mut self, context: &RenderContext, cam_uniform: CameraUniform) {
        self.camera_buffer.update(&context.queue, &cam_uniform);
    }
    fn render(
        &mut self,
        context: &RenderContext,
        resources: &ResourceManager,
        commands: impl FnOnce(&mut RenderCommands),
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

        let mut render_commands = RenderCommands {
            render_pass,
            resources,
        };
        commands(&mut render_commands);

        // use model::DrawLight;
        // render_pass.set_pipeline(&self.light_render_pipeline);
        // render_pass.draw_light_model(
        //     &self.obj_model,
        //     &self.cam_bind_group,
        //     &self.light_bind_group,
        // );

        drop(render_commands);

        context.queue.submit(std::iter::once(encoder.finish()));
        context.queue.present(output);
        Ok(())
    }
}

pub struct RenderCommands<'a> {
    render_state: &'a mut RenderState,
    render_pass: wgpu::RenderPass<'a>,
    resources: &'a ResourceManager,
}
impl<'a> RenderCommands<'a> {
    pub fn draw_model(
        &mut self,
        model: ModelHandle,
        instances: &[TransformRaw],
        num_instances: u32,
    ) {
        let model = self.resources.get_model(model);
        self.render_pass
            .set_vertex_buffer(1, instances.buffer().slice(..));

        for mesh in model.meshes.iter() {
            let material = self.resources.get_material(mesh.material);
            self.render_pass
                .set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.render_pass
                .set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.render_pass
                .set_bind_group(1, material.bind_group.bind_group(), &[]);
            self.render_pass
                .draw_indexed(0..mesh.num_elements, 0, 0..num_instances);
        }
    }
}
