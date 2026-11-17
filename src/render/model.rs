use super::asset_id::{MaterialId, MeshId};
use super::buffer::Buffer;
use super::vertex::*;
use glam::{Mat4, Vec3};

pub struct Mesh {
    pub vertex_buffer: Buffer<[ModelVertex]>,
    pub index_buffer: Buffer<[ModelIndex]>,
    pub num_elements: u32,
    pub material: MaterialId,
}

pub struct Model {
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Mesh(Vec<MeshId>),
    /// A `COL_` node: not drawn. Holds its walls as model-space segments
    /// (vertical triangles flattened to a line on the xz plane).
    Collider(Vec<[Vec3; 2]>),
    Empty,
}
pub struct Node {
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub local: Mat4,       // relative from parent, straight from GLTF
    pub model_space: Mat4, // precomputed, relative to model root
    pub kind: NodeKind,
}
