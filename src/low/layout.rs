use crate::low::{context::Context, texture::Texture};
pub struct MaterialBindGroupLayout {
    layout: wgpu::BindGroupLayout,
}
impl MaterialBindGroupLayout {
    pub fn descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("material_bind_group_layout"),
        }
    }
    pub fn new(context: &Context) -> Self {
        let layout = context.device.create_bind_group_layout(&Self::descriptor());
        Self { layout }
    }
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}
pub struct MaterialBindGroup {
    bind_group: wgpu::BindGroup,
}
impl MaterialBindGroup {
    pub fn new(
        context: &Context,
        layout: &MaterialBindGroupLayout,
        diffuse: &Texture,
        normal: &Texture,
    ) -> Self {
        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &layout.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&normal.sampler),
                    },
                ],
                label: None,
            });
        Self { bind_group }
    }
}

pub struct CameraBindGroupLayout {
    layout: wgpu::BindGroupLayout,
}
impl CameraBindGroupLayout {
    // Camera bind group layout. Will be in group 0
    pub fn descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("cam_bind_group_layout"),
        }
    }
}
pub struct CameraBindGroup {
    bind_group: wgpu::BindGroup,
}
impl CameraBindGroup {
    pub fn new(
        context: &Context,
        layout: &CameraBindGroupLayout,
        resource: wgpu::BindingResource,
    ) -> Self {
        let bind_group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &layout.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource,
                }],
                label: Some("cam_bind_group"),
            });
        Self { bind_group }
    }
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

// pub fn light_bind_group_layout() -> wgpu::BindGroupLayoutDescriptor<'static> {
//     wgpu::BindGroupLayoutDescriptor {
//         entries: &[wgpu::BindGroupLayoutEntry {
//             binding: 0,
//             visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
//             ty: wgpu::BindingType::Buffer {
//                 ty: wgpu::BufferBindingType::Uniform,
//                 has_dynamic_offset: false,
//                 min_binding_size: None,
//             },
//             count: None,
//         }],
//         label: None,
//     }
// }
