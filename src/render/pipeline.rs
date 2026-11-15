//! Render pipelines for the three glTF alpha modes, each in a single-sided
//! (back-face culled) and a double-sided (no culling) variant: 6 in total,
//! all built from one shader module.
//!
//! Shader entry points expected in the module:
//!   vs_main  - shared vertex shader
//!   fs_main  - opaque and blend
//!   fs_mask  - alpha test with `discard`
//!
//! Written against wgpu 24/25. In other versions some field names differ
//! (e.g. entry_point was a plain &str before 22, `cache` was added in 22).
//!
use crate::render::bindgroup::{
    GlobalBindGroup, GlobalBindGroupLayout, InstanceBufferBindGroupLayout, MaterialBindGroupLayout,
};
use crate::render::model;
use crate::render::resources::AlphaMode;
use crate::render::texture;
use crate::render::uniform::{CameraUniform, LightsUniform};
use model::Vertex;

struct RenderPipelineLayout {
    layout: wgpu::PipelineLayout,
}
impl RenderPipelineLayout {
    pub fn new(
        device: &wgpu::Device,
        global_layout: &GlobalBindGroupLayout,
        material_layout: &MaterialBindGroupLayout,
        instance_buffer_layout: &InstanceBufferBindGroupLayout,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render_pipeline_layout"),
            bind_group_layouts: &[
                Some(global_layout.layout()),
                Some(material_layout.layout()),
                Some(instance_buffer_layout.layout()),
            ],
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
    alpha_mode: AlphaMode,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);
    let frag = match alpha_mode {
        AlphaMode::Blend | AlphaMode::Opaque => "fs_main",
        AlphaMode::Mask => "fs_mask",
    };

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
            entry_point: Some(frag),
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
            depth_write_enabled: Some(alpha_mode != AlphaMode::Blend),
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

pub struct MaterialPipelines {
    // [alpha_mode][double_sided as usize]
    pipelines: [[wgpu::RenderPipeline; 2]; 3],
}

impl MaterialPipelines {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let make = |mode: AlphaMode, double_sided: bool| {
            Self::create(
                device,
                shader,
                layout,
                vertex_layouts,
                color_format,
                mode,
                double_sided,
            )
        };
        Self {
            pipelines: [
                [
                    make(AlphaMode::Opaque, false),
                    make(AlphaMode::Opaque, true),
                ],
                [make(AlphaMode::Mask, false), make(AlphaMode::Mask, true)],
                [make(AlphaMode::Blend, false), make(AlphaMode::Blend, true)],
            ],
        }
    }

    pub fn get(&self, mode: AlphaMode, double_sided: bool) -> &wgpu::RenderPipeline {
        &self.pipelines[mode as usize][double_sided as usize]
    }

    fn create(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        vertex_layouts: &[wgpu::VertexBufferLayout],
        color_format: wgpu::TextureFormat,
        mode: AlphaMode,
        double_sided: bool,
    ) -> wgpu::RenderPipeline {
        // Everything that differs between the alpha modes is in this match.
        let (fragment_entry, blend, depth_write) = match mode {
            AlphaMode::Opaque => ("fs_main", None, true),
            AlphaMode::Mask => ("fs_mask", None, true),
            AlphaMode::Blend => ("fs_main", Some(wgpu::BlendState::ALPHA_BLENDING), false),
        };

        let label = format!(
            "material {:?}{}",
            mode,
            if double_sided { " double-sided" } else { "" }
        );

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fragment_entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // glTF uses counter-clockwise front faces, same as wgpu's default
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: if double_sided {
                    None
                } else {
                    Some(wgpu::Face::Back)
                },
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: depth_write,
                // Blend still tests against opaque depth, it just doesn't write.
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Draw loop: batches sorted by (alpha_mode, double_sided, material, mesh),
// so the pipeline only changes a handful of times per pass.
//
// let mut current: Option<(AlphaMode, bool)> = None;
// for batch in &batches {
//     let material = &materials[batch.material.0 as usize];
//     let key = (material.alpha_mode, material.double_sided);
//     if current != Some(key) {
//         pass.set_pipeline(pipelines.get(key.0, key.1));
//         current = Some(key);
//     }
//     // material bind group, vertex/index buffers, draw_indexed ...
// }
// ---------------------------------------------------------------------------
