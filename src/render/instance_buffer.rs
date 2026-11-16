use std::ops::{Bound, RangeBounds};

use crate::{
    math::Transform,
    render::{
        asset_manager::AssetManager,
        bindgroup::{InstanceBufferBindGroup, InstanceBufferBindGroupLayout},
        context::RenderContext,
        model::NodeKind,
        pipeline::MaterialPipelines,
    },
};

use super::material::AlphaMode;
use crate::render::asset_id::{MaterialId, MeshId, ModelId};

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub struct AlphaModeMask: u8 {
        const OPAQUE = 1 << 0;
        const MASK   = 1 << 1;
        const BLEND  = 1 << 2;
        const SOLID  = Self::OPAQUE.bits() | Self::MASK.bits();
    }
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub model: ModelId,
    pub transform: Transform,
}
impl Instance {
    fn to_raw(&self) -> InstanceData {
        let model = self.transform.mat4();
        let normal = glam::Mat3::from_mat4(model).inverse().transpose();
        let normal_padded = [
            normal.x_axis.extend(0.).to_array(),
            normal.y_axis.extend(0.).to_array(),
            normal.z_axis.extend(0.).to_array(),
        ];
        InstanceData {
            model: model.to_cols_array_2d(),
            normal: normal_padded,
        }
    }
}

use glam::{Mat3, Mat4, Vec3};

// ---------------------------------------------------------------------------
// Per-entry GPU data. Layout matches the WGSL struct at the bottom.
// mat3x3 in WGSL has 16-byte columns, so the normal matrix is 3 padded vec4s.
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 3],
}
impl InstanceData {
    pub fn new(world: Mat4) -> Self {
        let n = Mat3::from_mat4(world).inverse().transpose();
        let c = |v: glam::Vec3| [v.x, v.y, v.z, 0.0];
        Self {
            model: world.to_cols_array_2d(),
            normal: [c(n.x_axis), c(n.y_axis), c(n.z_axis)],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SortHint {
    Material { material: MaterialId, mesh: MeshId }, // opaque, mask
    Distance(f32),                                   // blend
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SortKey(u64);
impl SortKey {
    const ALPHA_SHIFT: u64 = 62;
    const DS_SHIFT: u64 = 61;
    fn alpha_mode(self) -> AlphaMode {
        match self.0 >> Self::ALPHA_SHIFT {
            0 => AlphaMode::Opaque,
            1 => AlphaMode::Mask,
            _ => AlphaMode::Blend,
        }
    }
    fn double_sided(self) -> bool {
        (self.0 >> Self::DS_SHIFT) & 1 != 0
    }
    fn new(alpha_mode: AlphaMode, double_sided: bool, hint: SortHint) -> Self {
        let high =
            (alpha_mode as u64) << Self::ALPHA_SHIFT | (double_sided as u64) << Self::DS_SHIFT;
        let low = match hint {
            SortHint::Distance(d) => (!d.max(0.0).to_bits()) as u64, // farthest first
            SortHint::Material { material, mesh } => {
                (material.0 as u64 & 0x1FFF_FFFF) << 32 | mesh.0 as u64
            }
        };
        Self(high | low)
    }
}

/// One draw call: `draw_indexed(.., 0, first..first + count)` for this mesh.
#[derive(Clone, Copy, Debug)]
pub struct Batch {
    pub key: SortKey,
    pub material: MaterialId,
    pub mesh: MeshId,
    pub first: u32,
    pub count: u32,
}

/// One thing to draw, produced by walking instances and their model nodes.
pub struct DrawItem {
    key: SortKey,
    material: MaterialId,
    mesh: MeshId,
    world: Mat4,
}

/// Global instance buffer: one GPU storage buffer holding the transform of every
/// drawn (instance, mesh) pair, sorted so each mesh's entries are contiguous.
/// `batches` is the "map": mesh -> range of entries to draw.
///
/// Assumes glam for math, and handles that are Copy + Ord newtypes over an index:
///     #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
///     pub struct MeshHandle(pub u32);
pub struct InstanceBuffer {
    buffer: wgpu::Buffer,
    bind_group: InstanceBufferBindGroup,
    capacity: u64, // in entries
    draw_items: Vec<DrawItem>,
    batches: Vec<Batch>,
    // mode m occupies mode_offsets[m]..mode_offsets[m + 1].
    // the 4th entry is a sentinel containing the array length
    mode_offsets: [usize; 4],
    dirty: bool,
}

impl InstanceBuffer {
    const ENTRY_SIZE: u64 = std::mem::size_of::<InstanceData>() as u64;

    pub fn new(
        device: &wgpu::Device,
        layout: &InstanceBufferBindGroupLayout,
        capacity: u64,
    ) -> Self {
        let capacity = capacity.max(64);
        let (buffer, bind_group) = Self::create(device, layout, capacity);
        Self {
            buffer,
            bind_group,
            capacity,
            batches: Vec::new(),
            draw_items: Vec::new(),
            mode_offsets: [0; 4],
            dirty: true,
        }
    }

    fn create(
        device: &wgpu::Device,
        layout: &InstanceBufferBindGroupLayout,
        capacity: u64,
    ) -> (wgpu::Buffer, InstanceBufferBindGroup) {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance buffer"),
            size: capacity * Self::ENTRY_SIZE,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = InstanceBufferBindGroup::new(device, layout, &buffer);
        (buffer, bind_group)
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        self.bind_group.bind_group()
    }

    /// Sort the draw items, upload them, and rebuild the batch list.
    pub fn update(
        &mut self,
        context: &RenderContext,
        layout: &InstanceBufferBindGroupLayout,
        instances: &[Instance],
        camera_pos: Vec3,
        assets: &AssetManager,
    ) {
        self.collect_draws(instances, camera_pos, assets);
        self.upload(context, layout);
    }

    fn upload(&mut self, context: &RenderContext, layout: &InstanceBufferBindGroupLayout) {
        // Material first, then mesh: fewer bind group switches while drawing.
        self.draw_items.sort_unstable_by_key(|i| i.key);

        let mut data = Vec::with_capacity(self.draw_items.len());

        self.batches.clear();
        let mut next_mode = 0;

        for (index, item) in self.draw_items.iter().enumerate() {
            data.push(InstanceData::new(item.world));
            let mode = item.key.alpha_mode() as usize;
            // we're creating a new batch if anything needs to be changed in the pipeline
            let new_batch = match self.batches.last() {
                Some(b) => b.key != item.key,
                None => true,
            };
            if new_batch {
                while next_mode <= mode {
                    self.mode_offsets[next_mode] = self.batches.len();
                    next_mode += 1;
                }
                self.batches.push(Batch {
                    key: item.key,
                    material: item.material,
                    mesh: item.mesh,
                    first: index as u32,
                    count: 1,
                });
            } else {
                self.batches.last_mut().unwrap().count += 1;
            }
        }
        // modes after the last item (and the sentinel) end at batches.len()
        while next_mode <= 3 {
            self.mode_offsets[next_mode] = self.batches.len();
            next_mode += 1;
        }

        // Grow (doubling) when needed. The bind group must be recreated with it.
        let needed = data.len() as u64;
        if needed > self.capacity {
            self.capacity = needed.next_power_of_two();
            let (buffer, bind_group) = Self::create(&context.device, layout, self.capacity);
            self.buffer = buffer;
            self.bind_group = bind_group;
        }

        if !data.is_empty() {
            context
                .queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&data));
        }
        self.dirty = false;
    }

    /// Walk every instance's mesh nodes. Node transforms are precomputed
    /// (model_space), so this is a flat loop with no recursion.
    fn collect_draws(&mut self, instances: &[Instance], camera_pos: Vec3, assets: &AssetManager) {
        self.draw_items.clear();
        for instance in instances {
            let model = assets.get_model(instance.model);
            for node in &model.nodes {
                if let NodeKind::Mesh(node_meshes) = &node.kind {
                    let world = instance.transform.mat4() * node.model_space;
                    for &mesh in node_meshes {
                        let msh = assets.get_mesh(mesh);
                        let mat = assets.get_material(msh.material);
                        let hint = if mat.alpha_mode == AlphaMode::Blend {
                            SortHint::Distance((camera_pos - world.w_axis.truncate()).length())
                        } else {
                            SortHint::Material {
                                material: msh.material,
                                mesh,
                            }
                        };
                        self.draw_items.push(DrawItem {
                            key: SortKey::new(mat.alpha_mode, mat.double_sided, hint),
                            material: msh.material,
                            mesh,
                            world,
                        });
                    }
                }
            }
        }
    }
    /// Draw everything in one buffer. Call once for the static buffer and once
    /// for the dynamic one.
    pub fn draw_range<'a>(
        &self,
        pass: &mut wgpu::RenderPass<'a>,
        assets: &AssetManager,
        pipelines: &MaterialPipelines,
        modes: impl RangeBounds<AlphaMode>,
    ) {
        use super::bindgroup;
        // Turn the range of modes into [first, end) mode indices.
        let first = match modes.start_bound() {
            Bound::Included(&m) => m as usize,
            Bound::Excluded(&m) => m as usize + 1,
            Bound::Unbounded => 0,
        };
        let end = match modes.end_bound() {
            Bound::Included(&m) => m as usize + 1,
            Bound::Excluded(&m) => m as usize,
            Bound::Unbounded => 3,
        };
        if first >= end {
            return; // empty range
        }

        let batches = &self.batches[self.mode_offsets[first]..self.mode_offsets[end]];
        if batches.is_empty() {
            return;
        }

        pass.set_bind_group(bindgroup::INSTANCES_GROUP, self.bind_group(), &[]);

        let mut current_pipeline: Option<(AlphaMode, bool)> = None;
        let mut current_material: Option<MaterialId> = None;

        for batch in batches {
            // pipeline: change only when batch mode or culling changes.
            let pipeline_key = (batch.key.alpha_mode(), batch.key.double_sided());
            if current_pipeline != Some(pipeline_key) {
                pass.set_pipeline(pipelines.get(pipeline_key.0, pipeline_key.1));
                current_pipeline = Some(pipeline_key);
            }

            if current_material != Some(batch.material) {
                pass.set_bind_group(
                    bindgroup::MATERIAL_GROUP,
                    assets.get_material(batch.material).bind_group.bind_group(),
                    &[],
                );
                current_material = Some(batch.material);
            }

            // a batch hold all instances of this mesh
            let mesh = assets.get_mesh(batch.mesh);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.buffer().slice(..));
            pass.set_index_buffer(
                mesh.index_buffer.buffer().slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(
                0..mesh.num_elements,
                0,
                batch.first..batch.first + batch.count,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Matching WGSL:
//
// struct InstanceData {
//     model: mat4x4<f32>,
//     normal: mat3x3<f32>,
// };
// @group(2) @binding(0) var<storage, read> instances: array<InstanceData>;
//
// @vertex
// fn vs_main(v: VertexInput, @builtin(instance_index) i: u32) -> VertexOutput {
//     let inst = instances[i];            // i already includes batch.first
//     let world_pos = inst.model * vec4<f32>(v.position, 1.0);
//     let normal = normalize(inst.normal * v.normal);
//     ...
// }
//
// Bind group layout entry for group 2:
//     visibility: wgpu::ShaderStages::VERTEX,
//     ty: wgpu::BindingType::Buffer {
//         ty: wgpu::BufferBindingType::Storage { read_only: true },
//         has_dynamic_offset: false,
//         min_binding_size: None,
//     },
// ---------------------------------------------------------------------------
