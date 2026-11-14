use crate::{
    math::Transform,
    render::resources::{NodeKind, ResourceManager},
};
use std::collections::HashMap;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use glam::prelude::*;

use crate::render::{
    buffer::Buffer,
    resources::{MeshHandle, ModelHandle},
};

#[derive(Debug, Clone)]
pub struct Instance {
    pub model: ModelHandle,
    pub transform: Transform,
}
impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        let model = self.transform.mat4();
        let normal = glam::Mat3::from_mat4(model).inverse().transpose();
        let normal_padded = [
            normal.x_axis.extend(0.).to_array(),
            normal.y_axis.extend(0.).to_array(),
            normal.z_axis.extend(0.).to_array(),
        ];
        InstanceRaw {
            model: model.to_cols_array_2d(),
            normal: normal_padded,
        }
    }
}

/// This structure represents the `InstanceInput` in the shader
/// ```wgsl
/// struct InstaceInput {
///     model: mat4x4<f32>,
///     normal: mat3x3<f32>,
/// }
///```
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 4]; 3],
}
impl InstanceRaw {
    pub fn _desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in the shader.
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials, we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5, not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct MeshRange {
    pub first: u32,
    pub count: u32,
}
impl MeshRange {
    pub fn instance_range(&self) -> std::ops::Range<u32> {
        self.first..(self.first + self.count)
    }
}
fn fill_staging(
    resources: &ResourceManager,
    staging: &mut Vec<InstanceRaw>,
    mesh_ranges: &mut HashMap<MeshHandle, MeshRange>,
    model_map: &HashMap<ModelHandle, Vec<Instance>>,
) {
    for (model, instances) in model_map.iter() {
        let model = resources.get_model(*model);
        for node in model.nodes.iter() {
            if let NodeKind::Mesh(meshes) = node.kind {
                let world = 
                for &mesh in meshes.iter() {

                }
            }
        }
        if instances.is_empty() {
            continue;
        }
        let first = staging.len() as u32;
        // append raw instances to byte vector
        for inst in instances.iter().map(|i| i.to_raw()) {
            staging.push(inst);
        }

        mesh_ranges.insert(
            *model,
            MeshRange {
                first,
                count: instances.len() as u32,
            },
        );
    }
}

pub struct InstanceMap {
    model_map: HashMap<ModelHandle, Vec<Instance>>,
    ranges: HashMap<MeshHandle, MeshRange>,
    staging: Vec<InstanceRaw>,
    buffer: Option<wgpu::Buffer>,
    buffer_len: usize,
    buffer_cap: usize,
    // `true` if the buffer was recreated and the bind group
    // needs to be redone
    dirty: bool,
}
impl InstanceMap {
    pub fn new() -> Self {
        Self {
            buffer: None,
            buffer_len: 0,
            buffer_cap: 0,
            model_map: HashMap::new(),
            ranges: HashMap::new(),
            staging: Vec::new(),
            dirty: false,
        }
    }
    pub fn add_instance(&mut self, instance: Instance) {
        self.model_map.entry(instance.model).or_default().push(instance);
    }
    pub fn clear_instances(&mut self) {
        for insts in self.model_map.values_mut() {
            insts.clear();
        }
    }
    pub fn mesh_range(&self, mesh: MeshHandle) -> Option<MeshRange> {
        self.ranges.get(&mesh).copied()
    }
    pub fn mesh_ranges(&self) -> impl Iterator<Item = (MeshHandle, MeshRange)> {
        self.ranges.iter().map(|(h, r)| (*h, *r))
    }
    pub fn update_buffer(&mut self, queue: &wgpu::Queue, device: &wgpu::Device) {
        // send instance map to staging buffer
        self.staging.clear();
        self.ranges.clear();
        fill_staging(&mut self.staging, &mut self.ranges, &self.model_map);

        let buf = self.buffer.take();
        match buf {
            // if the buffer wasn't created yet
            None => {
                // initial capacity: 64kb
                let cap = self.staging.len().max(1 << 16);
                let b = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("dynamic instance buffer"),
                    mapped_at_creation: false,
                    size: cap as u64,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                });
                queue.write_buffer(&b, 0, bytemuck::cast_slice(&self.staging));
                self.buffer = Some(b);
                self.buffer_cap = cap;
                self.buffer_len = self.staging.len();
                self.dirty = true;
            }
            Some(b) => {
                // if we have enough space
                if self.buffer_cap >= self.staging.len() {
                    queue.write_buffer(&b, 0, bytemuck::cast_slice(&self.staging));
                    self.buffer = Some(b);
                    self.buffer_len = self.staging.len();
                } else {
                    let cap = self.buffer_cap * 2;
                    let b = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("dynamic instance buffer"),
                        mapped_at_creation: false,
                        size: cap as u64,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    });
                    queue.write_buffer(&b, 0, bytemuck::cast_slice(&self.staging));
                    self.buffer = Some(b);
                    self.buffer_cap = cap;
                    self.buffer_len = self.staging.len();
                    self.dirty = true;
                }
            }
        }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        self.buffer
            .as_ref()
            .expect("Buffer should be updated at least once before being retrieved")
    }
    /// `true` if the buffer was recreated and needs to be rebound
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    /// clears the `dirty` bit. Call this after recreating the BindGroup.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}
