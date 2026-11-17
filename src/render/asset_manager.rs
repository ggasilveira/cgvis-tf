use super::asset_id::*;
use super::bindgroup::MaterialBindGroupLayout;
use super::buffer::Buffer;
use super::context::RenderContext;
use super::material::{AlphaMode, Material};
use super::model::{Mesh, Model, Node, NodeKind};
use super::texture::{CreateTextureArgs, Texture};
use super::uniform::MaterialUniform;
use super::vertex::*;
use anyhow::Context;
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::path::PathBuf;
pub struct AssetManager {
    textures: Vec<Texture>,
    materials: Vec<Material>,
    material_bind_group_layout: MaterialBindGroupLayout,
    meshes: Vec<Mesh>,
    models: Vec<Model>,
    res_path: PathBuf,
    default_base_texture: TextureId,
    default_normal_texture: TextureId,
    default_material: MaterialId,
}
impl AssetManager {
    pub fn new(ctx: &RenderContext, assets_path: &str) -> anyhow::Result<Self> {
        let p = PathBuf::from(assets_path);
        assert!(p.is_dir(), "resources path must be a directory");
        let mut out = Self {
            textures: Vec::new(),
            materials: Vec::new(),
            material_bind_group_layout: MaterialBindGroupLayout::new(&ctx.device),
            models: Vec::new(),
            meshes: Vec::new(),
            res_path: p,
            // we'll set these after
            default_base_texture: TextureId(0),
            default_normal_texture: TextureId(0),
            default_material: MaterialId(0),
        };
        out.default_base_texture = out.create_color_texture(
            ctx,
            image::Rgba([255, 255, 255, 255]),
            Texture::DEFAULT_ARGS,
        )?;
        out.default_normal_texture =
            out.create_color_texture(ctx, image::Rgba([128, 128, 255, 255]), Texture::NORMAL_ARGS)?;
        out.create_default_material(ctx);
        Ok(out)
    }
    fn create_color_texture(
        &mut self,
        ctx: &RenderContext,
        color: image::Rgba<u8>,
        args: CreateTextureArgs,
    ) -> anyhow::Result<TextureId> {
        Ok(self.add_texture(Texture::create_color(ctx, color, args)?))
    }
    fn create_default_material(&mut self, ctx: &RenderContext) -> MaterialId {
        self.add_material(Material::new(
            ctx,
            self,
            &self.material_bind_group_layout,
            Material::DEFAULT_SETTINGS,
            AlphaMode::Opaque,
            false,
            self.default_base_texture,
            self.default_normal_texture,
        ))
    }
    /// Adds an already created texture to the manager
    fn add_texture(&mut self, texture: Texture) -> TextureId {
        self.textures.push(texture);
        TextureId(self.textures.len() - 1)
    }
    fn add_mesh(&mut self, mesh: Mesh) -> MeshId {
        self.meshes.push(mesh);
        MeshId(self.meshes.len() - 1)
    }
    fn add_material(&mut self, material: Material) -> MaterialId {
        self.materials.push(material);
        MaterialId(self.materials.len() - 1)
    }
    pub fn get_model(&self, handle: ModelId) -> &Model {
        &self.models[handle.0]
    }
    pub fn get_material(&self, handle: MaterialId) -> &Material {
        &self.materials[handle.0]
    }
    pub fn get_texture(&self, handle: TextureId) -> &Texture {
        &self.textures[handle.0]
    }
    pub fn get_mesh(&self, handle: MeshId) -> &Mesh {
        &self.meshes[handle.0]
    }
    pub fn models(&self) -> impl Iterator<Item = (ModelId, &Model)> {
        self.models.iter().enumerate().map(|(i, m)| (ModelId(i), m))
    }
    /// Loads an image as a texture into the GPU and returns its handle.
    /// The filename must be relative to the resources path
    pub fn load_texture(
        &mut self,
        ctx: &RenderContext,
        filename: &str,
        args: CreateTextureArgs,
    ) -> anyhow::Result<TextureId> {
        let filepath = self.res_path.join(filename);

        let bytes = std::fs::read(filepath)?;
        let txt = Texture::from_bytes(ctx, &bytes, args, Some(filename))?;
        Ok(self.add_texture(txt))
    }
    fn gltf_primitive2mesh(
        &self,
        ctx: &RenderContext,
        materials: &HashMap<usize, MaterialId>,
        buffers: &[gltf::buffer::Data],
        primitive: gltf::Primitive,
    ) -> anyhow::Result<Mesh> {
        let mat = primitive
            .material()
            .index()
            .map(|i| materials[&i])
            .unwrap_or(self.default_material);

        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        let mut vertices = Vec::new();
        // read indices
        let indices: Vec<_> = reader
            .read_indices()
            .context("GLTF mesh without indices")?
            .into_u32()
            .map(|i| ModelIndex(i))
            .collect();

        let positions = reader
            .read_positions()
            .context("GLTF mesh without vertex positions")?;
        let normals = reader
            .read_normals()
            .context("GLTF mesh without vertex normals")?;

        let tex_coords = reader.read_tex_coords(0);

        // 1. Read vertex positions ([f32; 3])
        for pos in positions {
            vertices.push(ModelVertex {
                position: pos,
                tex_coords: [0.; 2],
                normal: [0.; 3],
                // we'll build these later
                tangent: [0.; 3],
                bitangent: [0.; 3],
            });
        }

        for (i, norm) in normals.enumerate() {
            vertices[i].normal = norm;
        }

        if let Some(tex_coords) = tex_coords {
            for (i, uv) in tex_coords.into_f32().enumerate() {
                vertices[i].tex_coords = uv;
            }
        }
        build_normal_space(&mut vertices, &indices);
        Ok(Mesh {
            vertex_buffer: Buffer::new(&ctx.device, &vertices),
            index_buffer: Buffer::new(&ctx.device, &indices),
            material: mat,
            num_elements: indices.len() as u32,
        })
    }
    fn visit_glb_node(
        &mut self,
        ctx: &RenderContext,
        node: gltf::Node,
        model_nodes: &mut Vec<Node>,
        parent: Option<usize>,
        materials: &HashMap<usize, MaterialId>,
        buffers: &[gltf::buffer::Data],
    ) -> anyhow::Result<usize> {
        let node_index = model_nodes.len();
        let node_name = node
            .name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("node_{node_index}"));

        let local = glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        let model_space =
            parent.map_or(glam::Mat4::IDENTITY, |p| model_nodes[p].model_space) * local;

        let kind =
            match node.mesh() {
                None => NodeKind::Empty,
                Some(mesh) if node_name.starts_with("COL_") => {
                    NodeKind::Collider(collider_walls(mesh, buffers, model_space)?)
                }
                Some(mesh) => {
                    let mut meshes = Vec::new();
                    // a GLTF primitive is our mesh, and a gltf mesh is our model
                    for primitive in mesh.primitives() {
                        meshes.push(self.add_mesh(
                            self.gltf_primitive2mesh(ctx, materials, buffers, primitive)?,
                        ));
                    }
                    NodeKind::Mesh(meshes)
                }
            };
        model_nodes.push(Node {
            name: node_name,
            parent,
            children: Vec::new(),
            local,
            model_space,
            kind,
        });

        for child in node.children() {
            let child_idx = self.visit_glb_node(
                ctx,
                child,
                model_nodes,
                Some(node_index),
                materials,
                buffers,
            )?;
            model_nodes[node_index].children.push(child_idx);
        }
        Ok(node_index)
    }

    pub fn load_glb_model(
        &mut self,
        ctx: &RenderContext,
        filename: &str,
    ) -> anyhow::Result<ModelId> {
        let (document, buffers, images) = gltf::import(self.res_path.join(filename))?;
        let scene = document
            .default_scene()
            .context(".glb file missing default scene")?;

        let mut materials = HashMap::new();

        for material in document.materials() {
            let pbr = material.pbr_metallic_roughness();
            let base_color_texture = match pbr.base_color_texture() {
                None => self.default_base_texture,
                Some(t) => {
                    let texture = t.texture();
                    let im = &images[texture.source().index()];
                    let img = create_img_from_gltf(im);
                    self.add_texture(Texture::from_image(ctx, &img, Texture::DEFAULT_ARGS, None)?)
                }
            };
            let (normal_texture, normal_scale) = match material.normal_texture() {
                None => (self.default_normal_texture, 1.0),
                Some(t) => {
                    let texture = t.texture();
                    let im = &images[texture.source().index()];
                    let img = create_img_from_gltf(im);
                    (
                        self.add_texture(Texture::from_image(
                            ctx,
                            &img,
                            Texture::NORMAL_ARGS,
                            None,
                        )?),
                        t.scale(),
                    )
                }
            };
            let (alpha_mode, alpha_cutoff) = match material.alpha_mode() {
                gltf::material::AlphaMode::Blend => (AlphaMode::Blend, 0.0),
                gltf::material::AlphaMode::Opaque => (AlphaMode::Opaque, 0.0),
                gltf::material::AlphaMode::Mask => {
                    (AlphaMode::Mask, material.alpha_cutoff().unwrap_or(0.5))
                }
            };
            let double_sided = material.double_sided();
            let settings = MaterialUniform {
                base_color: pbr.base_color_factor(),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                alpha_cutoff,
                normal_scale,
            };
            let mat = Material::new(
                ctx,
                self,
                &self.material_bind_group_layout,
                settings,
                alpha_mode,
                double_sided,
                base_color_texture,
                normal_texture,
            );
            materials.insert(material.index().unwrap(), self.add_material(mat));
        }
        let mut model = Model {
            nodes: Vec::new(),
            roots: Vec::new(),
        };
        for node in scene.nodes() {
            model.roots.push(self.visit_glb_node(
                ctx,
                node,
                &mut model.nodes,
                None,
                &materials,
                &buffers,
            )?);
        }
        let idx = self.models.len();
        self.models.push(model);
        Ok(ModelId(idx))
    }
}

/// Triangles whose lowest point is higher than this above the bottom of the
/// collider mesh (e.g. the wall above a doorway) don't block movement.
const WALL_MAX_BASE: f32 = 1.0;

/// Flattens the vertical triangles of a `COL_` mesh into model-space wall
/// segments for the 2D (xz) collision.
fn collider_walls(
    mesh: gltf::Mesh,
    buffers: &[gltf::buffer::Data],
    model_space: glam::Mat4,
) -> anyhow::Result<Vec<[Vec3; 2]>> {
    let mut triangles = Vec::new();
    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let positions: Vec<Vec3> = reader
            .read_positions()
            .context("GLTF collider without vertex positions")?
            .map(|p| model_space.transform_point3(p.into()))
            .collect();
        let indices: Vec<u32> = reader
            .read_indices()
            .context("GLTF collider without indices")?
            .into_u32()
            .collect();
        for t in indices.as_chunks::<3>().0 {
            triangles.push(t.map(|i| positions[i as usize]));
        }
    }
    let floor = triangles
        .iter()
        .flatten()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);

    let mut walls = Vec::new();
    for [a, b, c] in triangles {
        let normal = (b - a).cross(c - a);
        let vertical = normal.y.abs() < 0.3 * normal.length();
        if !vertical || a.y.min(b.y).min(c.y) > floor + WALL_MAX_BASE {
            continue;
        }
        // The triangle projects to (almost) a line: keep its two farthest points.
        let flat = |p: Vec3| Vec2::new(p.x, p.z);
        let wall = [[a, b], [b, c], [c, a]]
            .into_iter()
            .max_by(|x, y| {
                let dx = flat(x[0]).distance_squared(flat(x[1]));
                let dy = flat(y[0]).distance_squared(flat(y[1]));
                dx.total_cmp(&dy)
            })
            .unwrap();
        walls.push(wall);
    }
    Ok(walls)
}

fn build_normal_space(vertices: &mut [ModelVertex], indices: &[ModelIndex]) {
    let mut triangles_included = vec![0; vertices.len()];
    for c in indices.chunks(3) {
        let c0 = c[0].0 as usize;
        let c1 = c[1].0 as usize;
        let c2 = c[2].0 as usize;
        let v0 = vertices[c0];
        let v1 = vertices[c1];
        let v2 = vertices[c2];

        let pos0: Vec3 = v0.position.into();
        let pos1: Vec3 = v1.position.into();
        let pos2: Vec3 = v2.position.into();

        let uv0: Vec2 = v0.tex_coords.into();
        let uv1: Vec2 = v1.tex_coords.into();
        let uv2: Vec2 = v2.tex_coords.into();

        let delta_pos1 = pos1 - pos0;
        let delta_pos2 = pos2 - pos0;

        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        // tangent and bitangent calculation black magic. see learnwgpu:
        // https://sotrh.github.io/learn-wgpu/intermediate/tutorial11-normals
        let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
        let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
        // We flip the bitangent to enable right-handed normal
        // maps with wgpu texture coordinate system
        let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

        vertices[c0].tangent = (tangent + Vec3::from(vertices[c0].tangent)).into();
        vertices[c1].tangent = (tangent + Vec3::from(vertices[c1].tangent)).into();
        vertices[c2].tangent = (tangent + Vec3::from(vertices[c2].tangent)).into();
        vertices[c0].bitangent = (bitangent + Vec3::from(vertices[c0].bitangent)).into();
        vertices[c1].bitangent = (bitangent + Vec3::from(vertices[c1].bitangent)).into();
        vertices[c2].bitangent = (bitangent + Vec3::from(vertices[c2].bitangent)).into();

        // used to average the tangents/bitangents
        triangles_included[c0] += 1;
        triangles_included[c1] += 1;
        triangles_included[c2] += 1;
    }

    // average the tangents/bitangents
    for (i, n) in triangles_included.into_iter().enumerate() {
        let denom = 1.0 / n as f32;
        let v = &mut vertices[i];
        v.tangent = (Vec3::from(v.tangent) * denom).into();
        v.bitangent = (Vec3::from(v.bitangent) * denom).into();
    }
}

use image::{DynamicImage, ImageBuffer};
fn create_img_from_gltf(im: &gltf::image::Data) -> DynamicImage {
    use gltf::image::Format as GltfFmt;
    match im.format {
        GltfFmt::R8G8B8 => image::DynamicImage::ImageRgb8(
            ImageBuffer::from_raw(im.width, im.height, im.pixels.clone()).unwrap(),
        ),
        GltfFmt::R8G8B8A8 => image::DynamicImage::ImageRgba8(
            ImageBuffer::from_raw(im.width, im.height, im.pixels.clone()).unwrap(),
        ),
        GltfFmt::R16G16B16 => {
            let pixels: Vec<u16> = bytemuck::cast_slice(&im.pixels).to_vec();
            image::DynamicImage::ImageRgb16(
                ImageBuffer::from_raw(im.width, im.height, pixels).unwrap(),
            )
        }
        GltfFmt::R16G16B16A16 => {
            let pixels: Vec<u16> = bytemuck::cast_slice(&im.pixels).to_vec();
            image::DynamicImage::ImageRgba16(
                ImageBuffer::from_raw(im.width, im.height, pixels).unwrap(),
            )
        }
        GltfFmt::R32G32B32FLOAT => {
            let pixels: Vec<f32> = bytemuck::cast_slice(&im.pixels).to_vec();
            image::DynamicImage::ImageRgb32F(
                ImageBuffer::from_raw(im.width, im.height, pixels).unwrap(),
            )
        }
        GltfFmt::R32G32B32A32FLOAT => {
            let pixels: Vec<f32> = bytemuck::cast_slice(&im.pixels).to_vec();
            image::DynamicImage::ImageRgba32F(
                ImageBuffer::from_raw(im.width, im.height, pixels).unwrap(),
            )
        }
        _ => unimplemented!("invalid texture image format"),
    }
}
