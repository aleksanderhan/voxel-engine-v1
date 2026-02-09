use glam::IVec3;
use rayon::prelude::*;

use crate::svo::chunk::CHUNK_SIZE;
use crate::svo::world::World;

pub const VIEW_RADIUS_CHUNKS: i32 = 2;
pub const VIEW_DIAMETER_CHUNKS: i32 = VIEW_RADIUS_CHUNKS * 2 + 1;
pub const VIEW_SIZE: i32 = CHUNK_SIZE * VIEW_DIAMETER_CHUNKS;
pub const VIEW_VOLUME: usize = (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize);

pub struct ChunkManager {
    pub buffer: wgpu::Buffer,
    data: Vec<u32>,
    last_center: Option<IVec3>,
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
        Self {
            buffer,
            data,
            last_center: None,
        }
    }

    pub fn update_from_world(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        center_chunk: IVec3,
    ) {
        let size = VIEW_SIZE as usize;
        let origin = (center_chunk - IVec3::splat(VIEW_RADIUS_CHUNKS)) * CHUNK_SIZE;
        let plane = size * size;
        let mut next_data = vec![0u32; VIEW_VOLUME];
        let mut copied_bounds = None;

        if let Some(last_center) = self.last_center {
            let delta_chunks = center_chunk - last_center;
            let delta_voxels = delta_chunks * CHUNK_SIZE;
            if delta_voxels.abs().cmple(IVec3::splat(VIEW_SIZE)).all() {
                let dx = delta_voxels.x;
                let dy = delta_voxels.y;
                let dz = delta_voxels.z;

                let src_x_start = dx.max(0) as usize;
                let src_y_start = dy.max(0) as usize;
                let src_z_start = dz.max(0) as usize;
                let src_x_end = (size as i32 + dx.min(0)) as usize;
                let src_y_end = (size as i32 + dy.min(0)) as usize;
                let src_z_end = (size as i32 + dz.min(0)) as usize;

                let dst_x_start = (-dx).max(0) as usize;
                let dst_y_start = (-dy).max(0) as usize;
                let dst_z_start = (-dz).max(0) as usize;
                let dst_x_end = dst_x_start + (src_x_end - src_x_start);
                let dst_y_end = dst_y_start + (src_y_end - src_y_start);
                let dst_z_end = dst_z_start + (src_z_end - src_z_start);

                for z in 0..(src_z_end - src_z_start) {
                    let src_z = src_z_start + z;
                    let dst_z = dst_z_start + z;
                    for y in 0..(src_y_end - src_y_start) {
                        let src_y = src_y_start + y;
                        let dst_y = dst_y_start + y;
                        let src_offset = src_x_start + size * (src_y + size * src_z);
                        let dst_offset = dst_x_start + size * (dst_y + size * dst_z);
                        let count = src_x_end - src_x_start;
                        next_data[dst_offset..dst_offset + count]
                            .copy_from_slice(&self.data[src_offset..src_offset + count]);
                    }
                }

                copied_bounds = Some((
                    dst_x_start,
                    dst_x_end,
                    dst_y_start,
                    dst_y_end,
                    dst_z_start,
                    dst_z_end,
                ));
            }
        }

        match copied_bounds {
            Some((x0, x1, y0, y1, z0, z1)) => {
                next_data
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(idx, slot)| {
                        let x = idx % size;
                        let y = (idx / size) % size;
                        let z = idx / plane;
                        if x >= x0 && x < x1 && y >= y0 && y < y1 && z >= z0 && z < z1 {
                            return;
                        }
                        let voxel = origin + IVec3::new(x as i32, y as i32, z as i32);
                        let density = world.sample_density(voxel);
                        let material = if density >= 0 {
                            let index = world.sample_material(voxel);
                            world.palette[index as usize]
                        } else {
                            0
                        };
                        *slot = material;
                    });
            }
            None => {
                next_data
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(idx, slot)| {
                        let x = idx % size;
                        let y = (idx / size) % size;
                        let z = idx / plane;
                        let voxel = origin + IVec3::new(x as i32, y as i32, z as i32);
                        let density = world.sample_density(voxel);
                        let material = if density >= 0 {
                            let index = world.sample_material(voxel);
                            world.palette[index as usize]
                        } else {
                            0
                        };
                        *slot = material;
                    });
            }
        }

        self.data = next_data;
        self.last_center = Some(center_chunk);
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.data));
    }
}
