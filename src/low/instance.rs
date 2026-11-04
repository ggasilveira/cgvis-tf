use std::collections::HashMap;
use wgpu::util::DeviceExt;

use glam::prelude::*;

use crate::low::{buffer::Buffer, resources::MeshHandle};

#[derive(Debug, Clone)]
pub struct Instance {
    pub position: Vec3,
    pub scale: Vec3,
    pub rotation: Quat,
}
impl Instance {
    fn to_raw(&self) -> InstanceRaw {
        let model =
            glam::Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position);
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
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
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
pub struct MeshSliceInfo {
    pub buffer_byte_offset: u32,
    pub instance_count: u32,
}
pub struct StaticInstanceBuffer {
    buffer: wgpu::Buffer,
    mesh_infos: HashMap<MeshHandle, MeshSliceInfo>,
}
impl StaticInstanceBuffer {
    pub fn mesh_info(&self, mesh: MeshHandle) -> Option<MeshSliceInfo> {
        self.mesh_infos.get(&mesh).copied()
    }
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

pub struct StaticInstanceMap {
    mesh_map: HashMap<MeshHandle, Vec<Instance>>,
}
impl StaticInstanceMap {
    pub fn new() -> Self {
        Self {
            mesh_map: HashMap::new(),
        }
    }
    pub fn add_instance(&mut self, mesh: MeshHandle, instance_data: Instance) {
        self.mesh_map.entry(mesh).or_default().push(instance_data);
    }
    pub fn bake(&self, device: &wgpu::Device) -> StaticInstanceBuffer {
        const SLICE_ALIGN: usize = 256;
        let mut mesh_infos = HashMap::new();

        let mut staging_bytes: Vec<u8> = Vec::new();

        fill_staging(&mut staging_bytes, &mut mesh_infos, &self.mesh_map);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("static instance buffer"),
            contents: &staging_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });
        StaticInstanceBuffer { buffer, mesh_infos }
    }
}
fn fill_staging(
    staging_bytes: &mut Vec<u8>,
    mesh_infos: &mut HashMap<MeshHandle, MeshSliceInfo>,
    mesh_map: &HashMap<MeshHandle, Vec<Instance>>,
) {
    const SLICE_ALIGN: usize = 256;

    for (mesh, instances) in mesh_map.iter() {
        if instances.is_empty() {
            continue;
        }
        let current_offset = staging_bytes.len();
        assert_eq!(current_offset % SLICE_ALIGN, 0);

        // append raw instances to byte vector
        for inst in instances.iter().map(|i| i.to_raw()) {
            staging_bytes.extend_from_slice(bytemuck::bytes_of(&inst));
        }

        // fill the difference between the alignment and the byte vector
        // with 0's
        let remainder = staging_bytes.len() % SLICE_ALIGN;
        if remainder != 0 {
            let padding = SLICE_ALIGN - remainder;
            staging_bytes.resize(staging_bytes.len() + padding, 0);
        }

        mesh_infos.insert(
            *mesh,
            MeshSliceInfo {
                instance_count: instances.len() as u32,
                buffer_byte_offset: current_offset as u32,
            },
        );
    }
}

pub struct DynamicInstanceMap {
    mesh_map: HashMap<MeshHandle, Vec<Instance>>,
    mesh_info: HashMap<MeshHandle, MeshSliceInfo>,
    staging_bytes: Vec<u8>,
    buffer: Option<wgpu::Buffer>,
    buffer_len: u32,
}
impl DynamicInstanceMap {
    pub fn new() -> Self {
        Self {
            buffer: None,
            buffer_len: 0,
            mesh_map: HashMap::new(),
            mesh_info: HashMap::new(),
            staging_bytes: Vec::new(),
        }
    }
    pub fn add_instance(&mut self, mesh: MeshHandle, instance: Instance) {
        self.mesh_map.entry(mesh).or_default().push(instance);
    }
    pub fn clear_instances(&mut self) {
        self.mesh_map.clear();
    }
    pub fn update_buffer(&mut self, queue: &wgpu::Queue, device: &wgpu::Device) {}
    pub fn buffer(&self) -> &wgpu::Buffer {
        self.buffer
            .as_ref()
            .expect("Buffer should be updated at least once before being retrieved")
    }
}
