use glam::IVec3;

use crate::svo::chunk::CHUNK_SIZE;
use crate::svo::world::World;

pub const VIEW_RADIUS_CHUNKS: i32 = 1;
pub const VIEW_DIAMETER_CHUNKS: i32 = VIEW_RADIUS_CHUNKS * 2 + 1;
pub const VIEW_SIZE: i32 = CHUNK_SIZE * VIEW_DIAMETER_CHUNKS;
pub const VIEW_VOLUME: usize = (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize);

pub struct ChunkManager {
    pub buffer: wgpu::Buffer,
    data: Vec<u32>,
}

impl ChunkManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let data = vec![0u32; VIEW_VOLUME];
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Buffer"),
            size: (VIEW_VOLUME * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { buffer, data }
    }

    pub fn update_from_world(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        center_chunk: IVec3,
    ) {
        let origin = (center_chunk - IVec3::splat(VIEW_RADIUS_CHUNKS)) * CHUNK_SIZE;
        let size = VIEW_SIZE as usize;
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let voxel = origin + IVec3::new(x as i32, y as i32, z as i32);
                    let density = world.sample_density(voxel);
                    let material = if density >= 0 {
                        let index = world.sample_material(voxel);
                        world.palette[index as usize]
                    } else {
                        0
                    };
                    let idx = x + size * (y + size * z);
                    self.data[idx] = material;
                }
            }
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.data));
    }
}
