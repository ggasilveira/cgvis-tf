use crate::low::bindgroup::{
    GlobalBindGroup, GlobalBindGroupLayout, MaterialBindGroup, MaterialBindGroupLayout,
};
use crate::low::buffer::Buffer;
use crate::low::camera::Camera;
use crate::low::context::RenderContext;
use crate::low::instance::*;
use crate::low::model;
use crate::low::resources::ResourceManager;
use crate::low::texture;
use crate::low::uniform::{CameraUniform, LightsUniform};
use model::Vertex;

struct RenderPipelineLayout {
    layout: wgpu::PipelineLayout,
}
impl RenderPipelineLayout {
    pub fn new(
        device: &wgpu::Device,
        global_layout: &GlobalBindGroupLayout,
        material_layout: &MaterialBindGroupLayout,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render_pipeline_layout"),
            bind_group_layouts: &[Some(global_layout.layout()), Some(material_layout.layout())],
            immediate_size: 0,
        });
        Self { layout }
    }
    pub fn layout(&self) -> &wgpu::PipelineLayout {
        &self.layout
    }
}
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
    depth_texture: texture::Texture,
    lights_uniform: LightsUniform,
    lights_buffer: Buffer<LightsUniform>,
    static_global_bg: GlobalBindGroup,
    dynamic_global_bg: GlobalBindGroup,
    dyn_instance_map: DynamicInstanceMap,
    static_instance_buffer: StaticInstanceBuffer,
    global_bind_group_layout: GlobalBindGroupLayout,
    material_bind_group_layout: MaterialBindGroupLayout,
}

impl RenderState {
    pub fn new(context: &RenderContext) -> anyhow::Result<Self> {
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

        let material_bind_group_layout = MaterialBindGroupLayout::new(&context.device);
        let global_bind_group_layout = GlobalBindGroupLayout::new(&context.device);
        let dyn_instance_map = DynamicInstanceMap::new();
        let static_instance_buffer = StaticInstanceMap::new().bake(&context.device);
        let static_global_bg = GlobalBindGroup::new_static(
            &context.device,
            &global_bind_group_layout,
            &camera_buffer,
            &lights_buffer,
            &static_instance_buffer,
        );
        let dynamic_global_bg = GlobalBindGroup::new_dynamic(
            &context.device,
            &global_bind_group_layout,
            &camera_buffer,
            &lights_buffer,
            &dyn_instance_map,
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

        Ok(Self {
            is_surface_configured: false,
            render_pipeline,
            static_global_bg,
            dynamic_global_bg,
            camera,
            camera_uniform,
            camera_buffer,
            lights_uniform,
            lights_buffer,
            depth_texture,
            dyn_instance_map,
            static_instance_buffer,
            global_bind_group_layout,
            material_bind_group_layout,
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
    pub fn dyn_instance_map(&mut self) -> &mut DynamicInstanceMap {
        &mut self.dyn_instance_map
    }
    pub fn set_static_instance_buffer(
        &mut self,
        ctx: &RenderContext,
        buffer: StaticInstanceBuffer,
    ) {
        self.static_instance_buffer = buffer;
        self.rebind_static(ctx);
    }
    fn rebind_static(&mut self, ctx: &RenderContext) {
        self.static_global_bg = GlobalBindGroup::new_static(
            &ctx.device,
            &self.global_bind_group_layout,
            &self.camera_buffer,
            &self.lights_buffer,
            &self.static_instance_buffer,
        );
    }
    fn rebind_dynamic(&mut self, ctx: &RenderContext) {
        self.dynamic_global_bg = GlobalBindGroup::new_dynamic(
            &ctx.device,
            &self.global_bind_group_layout,
            &self.camera_buffer,
            &self.lights_buffer,
            &self.dyn_instance_map,
        );
    }
    fn render(
        &mut self,
        context: &RenderContext,
        resources: &ResourceManager,
    ) -> anyhow::Result<()> {
        context.window.request_redraw();

        if !self.is_surface_configured {
            return Ok(());
        }

        // update the global bind group
        if self.dyn_instance_map.is_dirty() {
            self.dyn_instance_map.clear_dirty();
            self.rebind_dynamic(context);
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

        // we set once for drawing static instances
        render_pass.set_bind_group(
            GlobalBindGroup::GROUP,
            self.static_global_bg.bind_group(),
            &[],
        );

        for (mesh_handle, mesh_range) in self.static_instance_buffer.mesh_ranges() {
            let mesh = resources.get_mesh(mesh_handle);
            let material = resources.get_material(mesh.material);
            render_pass.set_bind_group(
                MaterialBindGroup::GROUP,
                material.bind_group.bind_group(),
                &[],
            );
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_elements, 0, mesh_range.instance_range());
        }

        // and another for the dynamic instances
        render_pass.set_bind_group(
            GlobalBindGroup::GROUP,
            self.dynamic_global_bg.bind_group(),
            &[],
        );

        for (mesh_handle, mesh_range) in self.dyn_instance_map.mesh_ranges() {
            let mesh = resources.get_mesh(mesh_handle);
            let material = resources.get_material(mesh.material);
            render_pass.set_bind_group(
                MaterialBindGroup::GROUP,
                material.bind_group.bind_group(),
                &[],
            );
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.num_elements, 0, mesh_range.instance_range());
        }

        // use model::DrawLight;
        // render_pass.set_pipeline(&self.light_render_pipeline);
        // render_pass.draw_light_model(
        //     &self.obj_model,
        //     &self.cam_bind_group,
        //     &self.light_bind_group,
        // );
        drop(render_pass);

        context.queue.submit(std::iter::once(encoder.finish()));
        context.queue.present(output);
        Ok(())
    }
}
