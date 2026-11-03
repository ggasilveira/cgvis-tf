use crate::low::uniform::{CameraUniform, LightsUniform};
use crate::low::{buffer::Buffer, texture::Texture};

/// Bind group layout that gets updated once per material/mesh.
/// group(1).
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
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&Self::descriptor());
        Self { layout }
    }
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}
/// group(1)
pub struct MaterialBindGroup {
    bind_group: wgpu::BindGroup,
}
impl MaterialBindGroup {
    pub fn new(
        device: &wgpu::Device,
        layout: &MaterialBindGroupLayout,
        diffuse: &Texture,
        normal: &Texture,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

/// Bind group layout that gets updated once per frame.
/// group(0). Contains camera and lights.
pub struct GlobalBindGroupLayout {
    layout: wgpu::BindGroupLayout,
}
impl GlobalBindGroupLayout {
    // Camera bind group layout. Will be in group 0
    pub fn descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // camera uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // lights storage buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("global_bind_group_layout"),
        }
    }
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&Self::descriptor());
        Self { layout }
    }
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

/// Bind group layout that gets updated once per frame.
/// group(0). Contains camera and lights.
pub struct GlobalBindGroup {
    bind_group: wgpu::BindGroup,
}
impl GlobalBindGroup {
    pub fn new(
        device: &wgpu::Device,
        layout: &GlobalBindGroupLayout,
        camera_buffer: &Buffer<CameraUniform>,
        lights_buffer: &Buffer<LightsUniform>,
    ) -> Self {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &layout.layout,
            // camera
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.buffer().as_entire_binding(),
                },
            ],
            label: Some("Global Bind Group"),
        });
        Self { bind_group }
    }
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

pub struct RenderPipelineLayout {
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
