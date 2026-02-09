use std::collections::HashMap;

use glam::IVec3;

use crate::svo::brick::{BrickState, BRICK_VOLUME};
use crate::svo::chunk::CHUNK_SIZE;
use crate::svo::chunk::{Chunk, BRICKS_PER_AXIS, BRICKS_PER_CHUNK};
use crate::svo::world::World;

pub const VIEW_RADIUS_CHUNKS: i32 = 7;
pub const VIEW_DIAMETER_CHUNKS: i32 = VIEW_RADIUS_CHUNKS * 2 + 1;
pub const VIEW_SIZE: i32 = CHUNK_SIZE * VIEW_DIAMETER_CHUNKS;

const WINDOW_CHUNK_COUNT: usize = (VIEW_DIAMETER_CHUNKS as usize)
    * (VIEW_DIAMETER_CHUNKS as usize)
    * (VIEW_DIAMETER_CHUNKS as usize);
const BRICK_PACKED_STRIDE_U32: usize = (BRICK_VOLUME + 3) / 4;

pub struct ChunkManager {
    pub brick_index_buffer: wgpu::Buffer,
    pub brick_materials_buffer: wgpu::Buffer,
    brick_indices: Vec<u32>,
    brick_materials: Vec<u32>,
    brick_materials_capacity: usize,
    last_center: Option<IVec3>,
    window_origin: IVec3,
    chunk_wrap_offset: IVec3,
    device: wgpu::Device,
}

impl ChunkManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let brick_indices = vec![0u32; WINDOW_CHUNK_COUNT * BRICKS_PER_CHUNK];
        let brick_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Brick Indices"),
            size: (brick_indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let brick_materials = vec![0u32; 1];
        let brick_materials_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Brick Materials"),
            size: (brick_materials.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            brick_index_buffer,
            brick_materials_buffer,
            brick_indices,
            brick_materials,
            brick_materials_capacity: 1,
            last_center: None,
            window_origin: IVec3::ZERO,
            chunk_wrap_offset: IVec3::ZERO,
            device: device.clone(),
        }
    }

    pub fn update_frame(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        center_chunk: IVec3,
    ) -> bool {
        self.publish_center(center_chunk);
        let rebuild = self.rebuild_window(world);
        let recreated = self.sync_gpu(queue);
        rebuild || recreated
    }

    pub fn window_origin(&self) -> IVec3 {
        self.window_origin
    }

    pub fn chunk_wrap_offset(&self) -> IVec3 {
        self.chunk_wrap_offset
    }

    fn publish_center(&mut self, center_chunk: IVec3) {
        let window_origin_chunk = center_chunk - IVec3::splat(VIEW_RADIUS_CHUNKS);
        let new_origin = window_origin_chunk * CHUNK_SIZE;
        let new_wrap = Self::wrap_chunk(window_origin_chunk);
        if self.last_center.is_none() {
            self.reset_window(center_chunk, new_origin, new_wrap);
            return;
        }

        let last_center = self.last_center.unwrap();
        if last_center == center_chunk {
            return;
        }

        self.window_origin = new_origin;
        self.chunk_wrap_offset = new_wrap;
        self.last_center = Some(center_chunk);
    }

    fn reset_window(&mut self, center_chunk: IVec3, new_origin: IVec3, new_wrap: IVec3) {
        self.brick_indices.fill(0);
        self.brick_materials.clear();
        self.window_origin = new_origin;
        self.chunk_wrap_offset = new_wrap;
        self.last_center = Some(center_chunk);
    }

    fn rebuild_window(&mut self, world: &World) -> bool {
        self.brick_indices.fill(0);
        self.brick_materials.clear();

        let mut brick_map: HashMap<usize, u32> = HashMap::new();
        let mut next_brick_id: u32 = 1;
        let window_origin_chunk = self.window_origin / CHUNK_SIZE;
        let size = VIEW_DIAMETER_CHUNKS as usize;
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    let chunk_offset = IVec3::new(x as i32, y as i32, z as i32);
                    let world_chunk = window_origin_chunk + chunk_offset;
                    let idx = self.storage_chunk_index(self.storage_chunk_offset(chunk_offset));
                    let base_index = idx * BRICKS_PER_CHUNK;
                    let Some(chunk) = world.chunks.get(&world_chunk) else {
                        continue;
                    };
                    for bz in 0..BRICKS_PER_AXIS {
                        for by in 0..BRICKS_PER_AXIS {
                            for bx in 0..BRICKS_PER_AXIS {
                                let brick_coord = IVec3::new(bx, by, bz);
                                let brick_idx = Chunk::brick_index(brick_coord);
                                let Some(brick_id) = chunk.bricks[brick_idx] else {
                                    continue;
                                };
                                let brick = &world.brick_pool.bricks[brick_id];
                                if brick.summary.state == BrickState::Empty {
                                    continue;
                                }
                                let compact_id = if let Some(&compact) = brick_map.get(&brick_id) {
                                    compact
                                } else {
                                    let compact = next_brick_id;
                                    next_brick_id += 1;
                                    brick_map.insert(brick_id, compact);
                                    self.append_brick_materials(brick);
                                    compact
                                };
                                self.brick_indices[base_index + brick_idx] = compact_id;
                            }
                        }
                    }
                }
            }
        }

        if self.brick_materials.is_empty() {
            self.brick_materials.push(0);
        }

        true
    }

    fn append_brick_materials(&mut self, brick: &crate::svo::brick::Brick) {
        let mut packed = [0u32; BRICK_PACKED_STRIDE_U32];
        for (idx, material) in brick.material.iter().enumerate() {
            let density = brick.density[idx];
            let value = if density >= 0 { *material } else { 0 };
            let word = idx / 4;
            let shift = (idx % 4) * 8;
            packed[word] |= (value as u32) << shift;
        }
        self.brick_materials.extend_from_slice(&packed);
    }

    fn sync_gpu(&mut self, queue: &wgpu::Queue) -> bool {
        let mut recreated = false;
        let needed_materials = self.brick_materials.len().max(1);
        if needed_materials > self.brick_materials_capacity {
            self.brick_materials_capacity = needed_materials;
            self.brick_materials_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Chunk Brick Materials"),
                size: (self.brick_materials_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            recreated = true;
        }

        queue.write_buffer(
            &self.brick_index_buffer,
            0,
            bytemuck::cast_slice(&self.brick_indices),
        );
        queue.write_buffer(
            &self.brick_materials_buffer,
            0,
            bytemuck::cast_slice(&self.brick_materials),
        );

        recreated
    }

    fn chunk_index(x: usize, y: usize, z: usize, size: usize) -> usize {
        x + size * (y + size * z)
    }

    fn storage_chunk_offset(&self, chunk_offset: IVec3) -> IVec3 {
        let size = VIEW_DIAMETER_CHUNKS;
        let wrapped = chunk_offset + self.chunk_wrap_offset;
        IVec3::new(
            wrapped.x.rem_euclid(size),
            wrapped.y.rem_euclid(size),
            wrapped.z.rem_euclid(size),
        )
    }

    fn storage_chunk_index(&self, chunk_offset: IVec3) -> usize {
        Self::chunk_index(
            chunk_offset.x as usize,
            chunk_offset.y as usize,
            chunk_offset.z as usize,
            VIEW_DIAMETER_CHUNKS as usize,
        )
    }

    fn wrap_chunk(chunk_coord: IVec3) -> IVec3 {
        let size = VIEW_DIAMETER_CHUNKS;
        IVec3::new(
            chunk_coord.x.rem_euclid(size),
            chunk_coord.y.rem_euclid(size),
            chunk_coord.z.rem_euclid(size),
        )
    }
}
