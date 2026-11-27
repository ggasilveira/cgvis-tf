//! Grass: blades generated in the vertex shader (`grass.wgsl`) on a grid of
//! cells around the camera. Nothing per blade is stored anywhere.
//!
//! Each frame `update` picks the cells within `RADIUS` that are in the view
//! frustum, and how many blades each needs (fewer with distance, none where
//! the mask is black). `draw` issues one instanced draw per cell. The blades
//! sit on a heightmap baked once from the ground mesh (`set_ground`).
//!
//! Draw after the opaque geometry and before the sky, with the global bind
//! group (camera, lights) already set.

use super::bindgroup::GlobalBindGroupLayout;
use super::camera::Camera;
use super::context::RenderContext;
use super::pipeline::MaterialPipelines;
use glam::{IVec2, Vec2, Vec3, Vec4};

/// Blade vertices: a strip of pairs up the blade, then the tip.
const SEGMENTS: u32 = 4; // grass.wgsl SEGMENTS
const BLADE_VERTICES: u32 = 2 * SEGMENTS + 1;
/// Tallest blade, for culling (grass.wgsl BLADE_MAX_HEIGHT, plus some slack).
const BLADE_MAX_HEIGHT: f32 = 1.2;
/// Heightmap value for "no ground here" (grass.wgsl HEIGHT_NONE).
const HEIGHT_NONE: f32 = -1e30;

const CELL_SIZE: f32 = 8.0;
/// Blades thin out with distance and are gone past this radius.
const RADIUS: f32 = 48.0;
/// Blades in a cell next to the camera: 16 per square meter.
const BLADES_PER_CELL: u32 = 1024;
/// Most cells that can be within RADIUS of the camera.
const MAX_CELLS: usize = {
    let side = 2 * (RADIUS / CELL_SIZE) as usize + 2;
    side * side
};
/// Heightmap resolution, in meters per texel.
const HEIGHT_TEXEL: f32 = 0.5;
const HEIGHTMAP_MAX_SIZE: u32 = 4096;

/// Fraction of blades kept at `dist` from the camera (grass.wgsl `density`).
fn density(dist: f32) -> f32 {
    let t = ((dist - 0.2 * RADIUS) / (0.8 * RADIUS)).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// What grass grows on, handed over by `Context::set_grass_ground`.
pub struct GrassGround {
    /// World-space triangles of the ground.
    pub triangles: Vec<[Vec3; 3]>,
    /// Grass density seen from above, stretched over the ground's xz bounds:
    /// white is full grass, black none. The image's top row is the ground's
    /// -z edge, so a top view in Blender (+y up) lines up with it.
    pub mask: Option<image::GrayImage>,
}

/// Highest mask value in each cell, so cells with no grass are skipped
/// before any blade is generated.
struct CellMask {
    /// Grid coordinates of the first cell.
    first: IVec2,
    width: i32,
    height: i32,
    max: Vec<f32>,
}

impl CellMask {
    /// `origin` and `size` are the world xz area the mask covers.
    fn new(mask: &image::GrayImage, origin: Vec2, size: Vec2) -> Self {
        let first = (origin / CELL_SIZE).floor().as_ivec2();
        let last = ((origin + size) / CELL_SIZE).floor().as_ivec2();
        let (width, height) = (last.x - first.x + 1, last.y - first.y + 1);
        let mut max = vec![0.0f32; (width * height) as usize];
        let pixel = size / Vec2::new(mask.width() as f32, mask.height() as f32);
        for (x, y, value) in mask.enumerate_pixels() {
            let value = value.0[0] as f32 / 255.0;
            if value == 0.0 {
                continue;
            }
            // The shader filters bilinearly, so a pixel reaches up to one
            // pixel away from its center: count it in every cell it reaches.
            let center = origin + (Vec2::new(x as f32, y as f32) + 0.5) * pixel;
            let lo = ((center - pixel) / CELL_SIZE).floor().as_ivec2() - first;
            let hi = ((center + pixel) / CELL_SIZE).floor().as_ivec2() - first;
            for cz in lo.y.max(0)..=hi.y.min(height - 1) {
                for cx in lo.x.max(0)..=hi.x.min(width - 1) {
                    let m = &mut max[(cz * width + cx) as usize];
                    *m = m.max(value);
                }
            }
        }
        Self {
            first,
            width,
            height,
            max,
        }
    }

    fn get(&self, cell: IVec2) -> f32 {
        let c = cell - self.first;
        if c.x < 0 || c.y < 0 || c.x >= self.width || c.y >= self.height {
            return 0.0;
        }
        self.max[(c.y * self.width + c.x) as usize]
    }
}

/// Matches `GrassUniform` in grass.wgsl.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GrassUniform {
    height_origin: [f32; 2],
    height_texel: f32,
    time: f32,
    wind_dir: [f32; 2],
    cell_size: f32,
    radius: f32,
    blades_per_cell: u32,
    _pad: u32,
    mask_size: [f32; 2],
}

/// The surface the grass grows on.
struct Ground {
    bind_group: wgpu::BindGroup,
    /// World xz of texel (0, 0).
    origin: Vec2,
    /// World xz past the last texel.
    end: Vec2,
    min_y: f32,
    max_y: f32,
    /// None without a mask: grass everywhere.
    cell_mask: Option<CellMask>,
}

pub struct Grass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    cells_buffer: wgpu::Buffer,
    ground: Option<Ground>,
    /// Blades to draw in each cell of `cells_buffer`, this frame.
    cell_blades: Vec<u32>,
}

impl Grass {
    pub fn new(
        device: &wgpu::Device,
        global_layout: &GlobalBindGroupLayout,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grass bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        // R32Float: read with textureLoad, filtered in the shader
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grass shader"),
            source: super::shader_source(include_str!("grass.wgsl")),
        });
        let mut bind_groups = [None; 2];
        bind_groups[GlobalBindGroupLayout::GROUP as usize] = Some(global_layout.layout());
        bind_groups[1] = Some(&layout);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grass pipeline layout"),
            bind_group_layouts: &bind_groups,
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grass"),
            layout: Some(&pipeline_layout),
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
            primitive: wgpu::PrimitiveState {
                // Each instance is its own strip: strips don't join across instances.
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                // blades are seen from both sides
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: MaterialPipelines::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grass uniform"),
            size: std::mem::size_of::<GrassUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cells_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("grass cells"),
            size: (MAX_CELLS * std::mem::size_of::<[f32; 2]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            layout,
            uniform_buffer,
            cells_buffer,
            ground: None,
            cell_blades: Vec::new(),
        }
    }

    /// Bakes the ground's height from its world-space triangles. Grass grows
    /// wherever the triangles cover, at the height of the highest one, and
    /// where the mask allows it.
    pub fn set_ground(&mut self, ctx: &RenderContext, ground: &GrassGround) {
        let triangles = &ground.triangles;
        let mut min = Vec3::INFINITY;
        let mut max = Vec3::NEG_INFINITY;
        for p in triangles.iter().flatten() {
            min = min.min(*p);
            max = max.max(*p);
        }
        if triangles.is_empty() {
            self.ground = None;
            return;
        }

        let extent = Vec2::new(max.x - min.x, max.z - min.z);
        let texel = HEIGHT_TEXEL.max(extent.max_element() / (HEIGHTMAP_MAX_SIZE - 1) as f32);
        let width = (extent.x / texel).ceil() as u32 + 1;
        let height = (extent.y / texel).ceil() as u32 + 1;
        let origin = Vec2::new(min.x, min.z);

        // Rasterize the triangles from above: each texel center inside a
        // triangle gets the triangle's height there.
        let mut heights = vec![HEIGHT_NONE; (width * height) as usize];
        for [a, b, c] in triangles {
            let to_texels = |p: &Vec3| (Vec2::new(p.x, p.z) - origin) / texel;
            let (pa, pb, pc) = (to_texels(a), to_texels(b), to_texels(c));
            let area = (pb - pa).perp_dot(pc - pa);
            // vertical triangles (walls) cover no ground
            if area.abs() < 1e-6 {
                continue;
            }
            let lo = pa.min(pb).min(pc).ceil().max(Vec2::ZERO);
            let hi = pa
                .max(pb)
                .max(pc)
                .floor()
                .min(Vec2::new((width - 1) as f32, (height - 1) as f32));
            for z in lo.y as u32..=hi.y as u32 {
                for x in lo.x as u32..=hi.x as u32 {
                    let p = Vec2::new(x as f32, z as f32);
                    // barycentric weights; a small tolerance closes seams
                    // between neighbouring triangles
                    let wa = (pc - pb).perp_dot(p - pb) / area;
                    let wb = (pa - pc).perp_dot(p - pc) / area;
                    let wc = 1.0 - wa - wb;
                    if wa < -1e-4 || wb < -1e-4 || wc < -1e-4 {
                        continue;
                    }
                    let y = wa * a.y + wb * b.y + wc * c.y;
                    let h = &mut heights[(z * width + x) as usize];
                    *h = h.max(y);
                }
            }
        }

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grass heightmap"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&heights),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Without a mask, a single white texel: grass everywhere.
        let white = image::GrayImage::from_pixel(1, 1, image::Luma([255]));
        let mask = ground.mask.as_ref().unwrap_or(&white);
        let mask_view = Self::create_mask_texture(ctx, mask);
        let mask_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("grass mask sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mask_size = extent;
        let cell_mask = ground
            .mask
            .as_ref()
            .map(|mask| CellMask::new(mask, origin, mask_size));

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grass bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.cells_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&mask_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&mask_sampler),
                },
            ],
        });
        self.ground = Some(Ground {
            bind_group,
            origin,
            end: origin + Vec2::new(width as f32, height as f32) * texel,
            min_y: min.y,
            max_y: max.y,
            cell_mask,
        });
        let uniform = GrassUniform {
            height_origin: origin.to_array(),
            height_texel: texel,
            time: 0.0,
            wind_dir: Vec2::new(0.8, 0.6).to_array(),
            cell_size: CELL_SIZE,
            radius: RADIUS,
            blades_per_cell: BLADES_PER_CELL,
            _pad: 0,
            mask_size: mask_size.to_array(),
        };
        ctx.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    /// R8Unorm (linear, not sRGB): a painted 50% gray means half the blades.
    fn create_mask_texture(ctx: &RenderContext, mask: &image::GrayImage) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: mask.width(),
            height: mask.height(),
            depth_or_array_layers: 1,
        };
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("grass mask"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mask.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mask.width()),
                rows_per_image: Some(mask.height()),
            },
            size,
        );
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Picks this frame's cells around the camera. `time` (seconds) drives the wind.
    pub fn update(&mut self, ctx: &RenderContext, camera: &Camera, time: f32) {
        self.cell_blades.clear();
        let Some(ground) = &self.ground else {
            return;
        };

        let frustum = Frustum::new(camera.build_matrix());
        let eye = Vec2::new(camera.eye.x, camera.eye.z);
        let first = ((eye - RADIUS) / CELL_SIZE).floor();
        let last = ((eye + RADIUS) / CELL_SIZE).floor();
        let mut cells = Vec::with_capacity(MAX_CELLS);
        for z in first.y as i32..=last.y as i32 {
            for x in first.x as i32..=last.x as i32 {
                let min = Vec2::new(x as f32, z as f32) * CELL_SIZE;
                let max = min + CELL_SIZE;
                if max.cmplt(ground.origin).any() || min.cmpgt(ground.end).any() {
                    continue;
                }
                // The cell's nearest point and densest mask pixel decide how
                // many blades it needs; the shader thins each blade by its
                // own distance and mask value.
                let painted = ground
                    .cell_mask
                    .as_ref()
                    .map_or(1.0, |m| m.get(IVec2::new(x, z)));
                let blades = (density(eye.clamp(min, max).distance(eye))
                    * painted
                    * BLADES_PER_CELL as f32)
                    .ceil() as u32;
                let visible = frustum.intersects_box(
                    Vec3::new(min.x, ground.min_y, min.y),
                    Vec3::new(max.x, ground.max_y + BLADE_MAX_HEIGHT, max.y),
                );
                if blades > 0 && visible && cells.len() < MAX_CELLS {
                    cells.push(min.to_array());
                    self.cell_blades.push(blades);
                }
            }
        }

        if !cells.is_empty() {
            ctx.queue
                .write_buffer(&self.cells_buffer, 0, bytemuck::cast_slice(&cells));
        }
        let time_offset = std::mem::offset_of!(GrassUniform, time) as u64;
        ctx.queue
            .write_buffer(&self.uniform_buffer, time_offset, bytemuck::bytes_of(&time));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass) {
        let Some(ground) = &self.ground else {
            return;
        };
        if self.cell_blades.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(1, &ground.bind_group, &[]);
        for (cell, &blades) in self.cell_blades.iter().enumerate() {
            let first = cell as u32 * BLADES_PER_CELL;
            pass.draw(0..BLADE_VERTICES, first..first + blades);
        }
    }
}

/// View frustum planes, pointing inwards (`dot(normal, p) + w >= 0` inside).
struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    /// From a view-projection matrix with depth in [0, 1].
    fn new(view_proj: glam::Mat4) -> Self {
        let (r0, r1, r2, r3) = (
            view_proj.row(0),
            view_proj.row(1),
            view_proj.row(2),
            view_proj.row(3),
        );
        Self {
            planes: [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2],
        }
    }

    fn intersects_box(&self, min: Vec3, max: Vec3) -> bool {
        self.planes.iter().all(|plane| {
            // the box corner furthest along the plane's normal
            let normal = plane.truncate();
            let corner = Vec3::select(normal.cmpge(Vec3::ZERO), max, min);
            normal.dot(corner) + plane.w >= 0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_mask_skips_only_cells_the_mask_cannot_reach() {
        // 32 x 32 m in 2 m pixels: grass on the left half (x < 16) only.
        let mask = image::GrayImage::from_fn(16, 16, |x, _| image::Luma([if x < 8 { 255 } else { 0 }]));
        let cells = CellMask::new(&mask, Vec2::ZERO, Vec2::splat(32.0));
        for z in 0..4 {
            assert_eq!(cells.get(IVec2::new(0, z)), 1.0);
            assert_eq!(cells.get(IVec2::new(1, z)), 1.0);
            // x 16..24: bilinear filtering bleeds the edge pixel into it
            assert_eq!(cells.get(IVec2::new(2, z)), 1.0);
            assert_eq!(cells.get(IVec2::new(3, z)), 0.0);
        }
        // outside the mask
        assert_eq!(cells.get(IVec2::new(-1, 0)), 0.0);
        assert_eq!(cells.get(IVec2::new(0, 9)), 0.0);
    }
}
