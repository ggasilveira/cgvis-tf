use crate::render::instance::{InstanceMap, StaticInstanceBuffer};
use crate::render::uniform::{CameraUniform, LightsUniform};
use crate::render::{buffer::Buffer, texture::Texture};

/// Bind group layout that gets updated once per material/mesh.
/// group(1).
pub struct MaterialBindGroupLayout {
    layout: wgpu::BindGroupLayout,
}
impl MaterialBindGroupLayout {
    pub const GROUP: u32 = 1;
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
    pub const GROUP: u32 = MaterialBindGroupLayout::GROUP;
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
    pub const GROUP: u32 = 1;
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
                // instance buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
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
    pub const GROUP: u32 = GlobalBindGroupLayout::GROUP;
    fn new_raw(
        device: &wgpu::Device,
        layout: &GlobalBindGroupLayout,
        camera_buffer: &Buffer<CameraUniform>,
        lights_buffer: &Buffer<LightsUniform>,
        instance_buffer: &wgpu::Buffer,
        label: Option<&str>,
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: instance_buffer.as_entire_binding(),
                },
            ],
            label,
        });
        Self { bind_group }
    }
    pub fn new_static(
        device: &wgpu::Device,
        layout: &GlobalBindGroupLayout,
        camera_buffer: &Buffer<CameraUniform>,
        lights_buffer: &Buffer<LightsUniform>,
        static_instance_buffer: &StaticInstanceBuffer,
    ) -> Self {
        Self::new_raw(
            device,
            layout,
            camera_buffer,
            lights_buffer,
            static_instance_buffer.buffer(),
            Some("Static Global Bind Group"),
        )
    }
    pub fn new_dynamic(
        device: &wgpu::Device,
        layout: &GlobalBindGroupLayout,
        camera_buffer: &Buffer<CameraUniform>,
        lights_buffer: &Buffer<LightsUniform>,
        dyn_instance_map: &InstanceMap,
    ) -> Self {
        Self::new_raw(
            device,
            layout,
            camera_buffer,
            lights_buffer,
            dyn_instance_map.buffer(),
            Some("Dynamic Global Bind Group"),
        )
    }
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
