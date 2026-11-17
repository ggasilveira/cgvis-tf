//! Procedural night sky, drawn as a fullscreen triangle at the far plane.
//! Must be drawn after opaque geometry and before transparent geometry, with
//! the global bind group (camera) already set.

use super::bindgroup::GlobalBindGroupLayout;
use super::pipeline::MaterialPipelines;

pub struct Skybox {
    pipeline: wgpu::RenderPipeline,
}

impl Skybox {
    pub fn new(
        device: &wgpu::Device,
        global_layout: &GlobalBindGroupLayout,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("skybox shader"),
            source: super::shader_source(include_str!("skybox.wgsl")),
        });
        let mut bind_groups = [None; 1];
        bind_groups[GlobalBindGroupLayout::GROUP as usize] = Some(global_layout.layout());
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skybox pipeline layout"),
            bind_group_layouts: &bind_groups,
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skybox"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: MaterialPipelines::DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                // The triangle sits exactly at depth 1.0, the clear value:
                // it passes only where nothing was drawn.
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1);
    }
}
