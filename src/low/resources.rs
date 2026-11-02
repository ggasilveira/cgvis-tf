use crate::low::context::Context;
use crate::low::model;
use crate::low::texture::{CreateTextureArgs, Texture};
use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshHandle(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureHandle(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialHandle(usize);
pub struct ModelHandle(usize);
pub struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_elements: u32,
    material: MaterialHandle,
}
pub struct Material {
    diffuse: TextureHandle,
    normal: TextureHandle,
}
pub struct Model {
    meshes: Vec<Mesh>,
}

pub struct ResourceManager {
    context: Arc<Context>,
    textures: Vec<Texture>,
    materials: Vec<Material>,
    models: Vec<Model>,
    res_path: PathBuf,
}
impl ResourceManager {
    pub fn new(context: Arc<Context>, resources_path: &str) -> Self {
        let p = PathBuf::from(resources_path);
        assert!(p.is_dir(), "resources path must be a directory");
        Self {
            context,
            textures: Vec::new(),
            materials: Vec::new(),
            models: Vec::new(),
            res_path: p,
        }
    }
    /// Adds an already created texture to the manager
    fn add_texture(&mut self, texture: Texture) -> TextureHandle {
        self.textures.push(texture);
        TextureHandle(self.textures.len() - 1)
    }
    fn add_material(&mut self, material: Material) -> MaterialHandle {
        self.materials.push(material);
        MaterialHandle(self.materials.len() - 1)
    }
    /// Loads an image as a texture into the GPU and returns its handle.
    /// The filename must be relative to the resources path
    pub fn load_texture(
        &mut self,
        filename: &str,
        args: CreateTextureArgs,
    ) -> anyhow::Result<TextureHandle> {
        let filepath = self.res_path.join(filename);

        let bytes = std::fs::read(filepath)?;
        let txt = Texture::from_bytes(&self.context, &bytes, args, Some(filename))?;
        Ok(self.add_texture(txt))
    }

    /// Loads a model from a .obj file. Group and object diretives are both treated
    /// as meshes of a single object.
    pub fn load_obj_model(&mut self, filename: &str) -> anyhow::Result<ModelHandle> {
        let obj_text = std::fs::read_to_string(filename)?;
        let obj_cursor = Cursor::new(obj_text);
        let mut obj_reader = BufReader::new(obj_cursor);

        let (models, obj_materials) =
            tobj::load_obj_buf(&mut obj_reader, &tobj::GPU_LOAD_OPTIONS, |p| {
                let res_path = self.res_path.join(p);
                let mat_text = std::fs::read_to_string(res_path)
                    .map_err(|_| tobj::LoadError::OpenFileFailed)?;
                tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
            })?;
        let obj_materials = obj_materials?;
        let mut materials = Vec::new();
        for material in obj_materials {
            anyhow::ensure!(
                material.diffuse_texture.is_some(),
                "Materials must have a diffuse texture"
            );
            let diffuse_file = material.diffuse_texture.unwrap();
            let normal_file = material.normal_texture;
            let diffuse_texture = self.load_texture(&diffuse_file, Texture::DEFAULT_ARGS)?;

            // if there isn't a normal file, we'll create a normal "flat" texture
            let normal_texture = match normal_file {
                Some(f) => self.load_texture(&f, Texture::NORMAL_ARGS)?,
                None => {
                    let diffuse_size = self.textures[diffuse_texture.0].texture.size();
                    // a flat normal texture has all normal vectors as unit Z.
                    // X: -1 to +1 :  Red:     0 to 255
                    //Y: -1 to +1 :  Green:   0 to 255
                    //Z:  0 to -1 :  Blue:  128 to 255
                    // to have XYZ = (0, 0, -1) (pointing towards the viewer),
                    // we have RGB = (128, 128, 255)
                    let normal = image::Rgb([128_u8, 128_u8, 255_u8]);
                    let img = image::ImageBuffer::from_pixel(
                        diffuse_size.width,
                        diffuse_size.height,
                        normal,
                    );
                    let txt = Texture::from_image(
                        &self.context,
                        &img.into(),
                        Texture::NORMAL_ARGS,
                        None,
                    )?;
                    self.add_texture(txt)
                }
            };
            materials.push(self.add_material(Material {
                diffuse: diffuse_texture,
                normal: normal_texture,
            }));
        }
        let meshes = models
            .into_iter()
            .map(|m| {
                let mut vertices = (0..m.mesh.positions.len() / 3)
                    .map(|i| model::ModelVertex {
                        position: [
                            m.mesh.positions[i * 3],
                            m.mesh.positions[i * 3 + 1],
                            m.mesh.positions[i * 3 + 2],
                        ],
                        tex_coords: [m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1]],
                        normal: [
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        ],
                        // we'll calculate these later
                        tangent: [0.; 3],
                        bitangent: [0.; 3],
                    })
                    .collect::<Vec<_>>();

                build_normal_space(&mut vertices, &m.mesh.indices);

                let vertex_buffer =
                    self.context
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Vertex Buffer", filename)),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                let index_buffer =
                    self.context
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&format!("{:?} Index Buffer", filename)),
                            contents: bytemuck::cast_slice(&m.mesh.indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });

                Mesh {
                    vertex_buffer,
                    index_buffer,
                    num_elements: m.mesh.indices.len() as u32,
                    material: materials[m.mesh.material_id.unwrap_or(0)],
                }
            })
            .collect::<Vec<_>>();
        self.models.push(Model { meshes });
        Ok(ModelHandle(self.models.len() - 1))
    }
}

fn build_normal_space(vertices: &mut [model::ModelVertex], indices: &[u32]) {
    let mut triangles_included = vec![0; vertices.len()];
    for c in indices.chunks(3) {
        let v0 = vertices[c[0] as usize];
        let v1 = vertices[c[1] as usize];
        let v2 = vertices[c[2] as usize];

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

        vertices[c[0] as usize].tangent =
            (tangent + Vec3::from(vertices[c[0] as usize].tangent)).into();
        vertices[c[1] as usize].tangent =
            (tangent + Vec3::from(vertices[c[1] as usize].tangent)).into();
        vertices[c[2] as usize].tangent =
            (tangent + Vec3::from(vertices[c[2] as usize].tangent)).into();
        vertices[c[0] as usize].bitangent =
            (bitangent + Vec3::from(vertices[c[0] as usize].bitangent)).into();
        vertices[c[1] as usize].bitangent =
            (bitangent + Vec3::from(vertices[c[1] as usize].bitangent)).into();
        vertices[c[2] as usize].bitangent =
            (bitangent + Vec3::from(vertices[c[2] as usize].bitangent)).into();

        // used to average the tangents/bitangents
        triangles_included[c[0] as usize] += 1;
        triangles_included[c[1] as usize] += 1;
        triangles_included[c[2] as usize] += 1;
    }

    // average the tangents/bitangents
    for (i, n) in triangles_included.into_iter().enumerate() {
        let denom = 1.0 / n as f32;
        let v = &mut vertices[i];
        v.tangent = (Vec3::from(v.tangent) * denom).into();
        v.bitangent = (Vec3::from(v.bitangent) * denom).into();
    }
}
