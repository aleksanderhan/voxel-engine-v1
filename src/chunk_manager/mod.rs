use glam::IVec3;

use crate::svo::chunk::CHUNK_SIZE;
use crate::svo::world::World;

pub const CHUNK_VOLUME: usize = (CHUNK_SIZE as usize)
    * (CHUNK_SIZE as usize)
    * (CHUNK_SIZE as usize);

pub struct ChunkManager {
    pub buffer: wgpu::Buffer,
    data: Vec<u32>,
}

impl ChunkManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let data = vec![0u32; CHUNK_VOLUME];
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Buffer"),
            size: (CHUNK_VOLUME * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, data }
    }

    pub fn update_from_world(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        chunk_coord: IVec3,
    ) {
        let origin = chunk_coord * CHUNK_SIZE;
        let size = CHUNK_SIZE as usize;
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let voxel = origin + IVec3::new(x as i32, y as i32, z as i32);
                    let density = world.sample_density(voxel);
                    let material = if density >= 0 {
                        world.sample_material(voxel)
                    } else {
                        0
                    };
                    let idx = x + size * (y + size * z);
                    self.data[idx] = material as u32;
                }
            }
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.data));
    }
}
