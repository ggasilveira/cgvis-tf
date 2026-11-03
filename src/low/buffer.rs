use std::marker::PhantomData;

use wgpu::util::DeviceExt;

use crate::low::{
    instance::InstanceRaw,
    uniform::{CameraUniform, LightsUniform},
};

pub trait Bufferable {
    fn label(&self) -> Option<&str>;
    fn contents(&self) -> &[u8];
    fn usage(&self) -> wgpu::BufferUsages;
}

pub struct Buffer<T: Bufferable + ?Sized> {
    buffer: wgpu::Buffer,
    _phantom: PhantomData<T>,
}
impl<T: Bufferable + ?Sized> Buffer<T> {
    pub fn new(device: &wgpu::Device, data: &T) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: data.label(),
            contents: data.contents(),
            usage: data.usage(),
        });
        Self {
            buffer,
            _phantom: PhantomData::default(),
        }
    }
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
    pub fn update(&self, queue: &wgpu::Queue, data: &T) {
        queue.write_buffer(&self.buffer, 0, data.contents());
    }
}

impl Bufferable for CameraUniform {
    fn label(&self) -> Option<&str> {
        Some("Camera Uniform Buffer")
    }
    fn contents(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
    fn usage(&self) -> wgpu::BufferUsages {
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
    }
}
impl Bufferable for LightsUniform {
    fn label(&self) -> Option<&str> {
        Some("Lights Uniform Buffer")
    }
    fn contents(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
    fn usage(&self) -> wgpu::BufferUsages {
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
    }
}

impl Bufferable for [InstanceRaw] {
    fn label(&self) -> Option<&str> {
        Some("Instace Buffer")
    }
    fn contents(&self) -> &[u8] {
        bytemuck::cast_slice(&self)
    }
    fn usage(&self) -> wgpu::BufferUsages {
        wgpu::BufferUsages::VERTEX
    }
}
