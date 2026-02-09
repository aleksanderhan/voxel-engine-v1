use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    resolution: [f32; 2],
    time: f32,
    fps: f32,
    camera_pos: [f32; 4],
    camera_forward: [f32; 4],
    camera_right: [f32; 4],
    camera_up: [f32; 4],
    chunk_origin: [f32; 4],
}

impl Uniforms {
    fn new(
        size: PhysicalSize<u32>,
        time: f32,
        fps: f32,
        camera_pos: [f32; 4],
        camera_forward: [f32; 4],
        camera_right: [f32; 4],
        camera_up: [f32; 4],
        chunk_origin: [f32; 4],
    ) -> Self {
        Self {
            resolution: [size.width as f32, size.height as f32],
            time,
            fps,
            camera_pos,
            camera_forward,
            camera_right,
            camera_up,
            chunk_origin,
        }
    }

    fn update(
        &mut self,
        size: PhysicalSize<u32>,
        time: f32,
        fps: f32,
        camera_pos: [f32; 4],
        camera_forward: [f32; 4],
        camera_right: [f32; 4],
        camera_up: [f32; 4],
        chunk_origin: [f32; 4],
    ) {
        self.resolution = [size.width as f32, size.height as f32];
        self.time = time;
        self.fps = fps;
        self.camera_pos = camera_pos;
        self.camera_forward = camera_forward;
        self.camera_right = camera_right;
        self.camera_up = camera_up;
        self.chunk_origin = chunk_origin;
    }
}

pub struct UniformBuffer {
    pub buffer: wgpu::Buffer,
    uniforms: Uniforms,
}

impl UniformBuffer {
    pub fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        let uniforms = Uniforms::new(
            size,
            0.0,
            0.0,
            [0.0, 2.5, 6.0, 0.0],
            [0.0, -0.2425, -0.9701, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.9701, -0.2425, 0.0],
            [0.0, 0.0, 0.0, 0.0],
        );
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        Self { buffer, uniforms }
    }

    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        size: PhysicalSize<u32>,
        time: f32,
        fps: f32,
        camera_pos: [f32; 4],
        camera_forward: [f32; 4],
        camera_right: [f32; 4],
        camera_up: [f32; 4],
        chunk_origin: [f32; 4],
    ) {
        self.uniforms.update(
            size,
            time,
            fps,
            camera_pos,
            camera_forward,
            camera_right,
            camera_up,
            chunk_origin,
        );
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.uniforms));
    }
}
