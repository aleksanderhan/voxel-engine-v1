use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    _padding: f32,
}

impl Uniforms {
    fn new(size: PhysicalSize<u32>, time: f32) -> Self {
        Self {
            resolution: [size.width as f32, size.height as f32],
            time,
            _padding: 0.0,
        }
    }

    fn update(&mut self, size: PhysicalSize<u32>, time: f32) {
        self.resolution = [size.width as f32, size.height as f32];
        self.time = time;
    }
}

pub struct UniformBuffer {
    pub buffer: wgpu::Buffer,
    uniforms: Uniforms,
}

impl UniformBuffer {
    pub fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        let uniforms = Uniforms::new(size, 0.0);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self { buffer, uniforms }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, size: PhysicalSize<u32>, time: f32) {
        self.uniforms.update(size, time);
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.uniforms));
    }
}
