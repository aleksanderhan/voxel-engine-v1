use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use glam::IVec3;
use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};

use crate::svo::chunk::CHUNK_SIZE;
use crate::svo::world::World;

pub const VIEW_RADIUS_CHUNKS: i32 = 2;
pub const VIEW_DIAMETER_CHUNKS: i32 = VIEW_RADIUS_CHUNKS * 2 + 1;
pub const VIEW_SIZE: i32 = CHUNK_SIZE * VIEW_DIAMETER_CHUNKS;
pub const VIEW_VOLUME: usize = (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize)
    * (VIEW_SIZE as usize);

const CHUNK_VOLUME: usize = (CHUNK_SIZE as usize)
    * (CHUNK_SIZE as usize)
    * (CHUNK_SIZE as usize);
const WINDOW_CHUNK_COUNT: usize = (VIEW_DIAMETER_CHUNKS as usize)
    * (VIEW_DIAMETER_CHUNKS as usize)
    * (VIEW_DIAMETER_CHUNKS as usize);
const CPU_VOXEL_BUDGET: usize = CHUNK_VOLUME * 2;
const GPU_UPLOAD_BUDGET_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkUnit {
    chunk_offset: IVec3,
    priority: i32,
}

impl Ord for WorkUnit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.chunk_offset.z.cmp(&other.chunk_offset.z))
            .then_with(|| self.chunk_offset.y.cmp(&other.chunk_offset.y))
            .then_with(|| self.chunk_offset.x.cmp(&other.chunk_offset.x))
    }
}

impl PartialOrd for WorkUnit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirtyRange {
    start: usize,
    end: usize,
}

impl DirtyRange {
    fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

pub struct ChunkManager {
    pub buffer: wgpu::Buffer,
    data: Vec<u32>,
    chunk_states: Vec<u8>,
    work_queue: BinaryHeap<WorkUnit>,
    urgent_queue: VecDeque<WorkUnit>,
    pending_dirty: Vec<DirtyRange>,
    merged_dirty: Vec<DirtyRange>,
    scratch_dirty: Vec<DirtyRange>,
    last_center: Option<IVec3>,
    window_origin: IVec3,
    chunk_wrap_offset: IVec3,
    pool: ThreadPool,
}

impl ChunkManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let data = vec![0u32; VIEW_VOLUME];
        let chunk_states = vec![0u8; WINDOW_CHUNK_COUNT];
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chunk Buffer"),
            size: (VIEW_VOLUME * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut work_queue = BinaryHeap::with_capacity(WINDOW_CHUNK_COUNT);
        let urgent_queue = VecDeque::with_capacity(WINDOW_CHUNK_COUNT);
        let pending_dirty = Vec::with_capacity(WINDOW_CHUNK_COUNT * CHUNK_SIZE as usize);
        let merged_dirty = Vec::with_capacity(WINDOW_CHUNK_COUNT * CHUNK_SIZE as usize);
        let scratch_dirty = Vec::with_capacity(WINDOW_CHUNK_COUNT * CHUNK_SIZE as usize);
        let pool = ThreadPoolBuilder::new()
            .thread_name(|idx| format!("chunk-worker-{idx}"))
            .build()
            .expect("Failed to create chunk worker pool");

        work_queue.clear();

        Self {
            buffer,
            data,
            chunk_states,
            work_queue,
            urgent_queue,
            pending_dirty,
            merged_dirty,
            scratch_dirty,
            last_center: None,
            window_origin: IVec3::ZERO,
            chunk_wrap_offset: IVec3::ZERO,
            pool,
        }
    }

    pub fn update_frame(
        &mut self,
        queue: &wgpu::Queue,
        world: &World,
        center_chunk: IVec3,
    ) {
        self.publish_center(center_chunk);
        self.pump_cpu(world);
        self.pump_gpu(queue);
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

        let delta_chunks = center_chunk - last_center;
        if delta_chunks
            .abs()
            .cmpge(IVec3::splat(VIEW_DIAMETER_CHUNKS))
            .any()
        {
            self.reset_window(center_chunk, new_origin, new_wrap);
            return;
        }

        self.window_origin = new_origin;
        self.chunk_wrap_offset = new_wrap;
        self.enqueue_exposed(delta_chunks);
        self.last_center = Some(center_chunk);
    }

    fn reset_window(&mut self, center_chunk: IVec3, new_origin: IVec3, new_wrap: IVec3) {
        self.data.fill(0);
        self.chunk_states.fill(0);
        self.work_queue.clear();
        self.urgent_queue.clear();
        self.pending_dirty.clear();
        self.pending_dirty.push(DirtyRange {
            start: 0,
            end: VIEW_VOLUME,
        });
        self.window_origin = new_origin;
        self.chunk_wrap_offset = new_wrap;
        self.last_center = Some(center_chunk);
        self.enqueue_full_window();
    }

    fn enqueue_full_window(&mut self) {
        let size = VIEW_DIAMETER_CHUNKS as usize;
        for z in 0..size {
            for y in 0..size {
                for x in 0..size {
                    self.enqueue_chunk(IVec3::new(x as i32, y as i32, z as i32));
                }
            }
        }
    }

    fn enqueue_exposed(&mut self, delta_chunks: IVec3) {
        let size = VIEW_DIAMETER_CHUNKS as i32;
        if delta_chunks.x > 0 {
            let x = size - 1;
            for z in 0..size {
                for y in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        } else if delta_chunks.x < 0 {
            let x = 0;
            for z in 0..size {
                for y in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        }

        if delta_chunks.y > 0 {
            let y = size - 1;
            for z in 0..size {
                for x in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        } else if delta_chunks.y < 0 {
            let y = 0;
            for z in 0..size {
                for x in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        }

        if delta_chunks.z > 0 {
            let z = size - 1;
            for y in 0..size {
                for x in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        } else if delta_chunks.z < 0 {
            let z = 0;
            for y in 0..size {
                for x in 0..size {
                    self.enqueue_chunk_forced(IVec3::new(x, y, z));
                }
            }
        }
    }

    fn enqueue_chunk_forced(&mut self, chunk_offset: IVec3) {
        let idx = self.storage_chunk_index(self.storage_chunk_offset(chunk_offset));
        self.chunk_states[idx] = 0;
        self.enqueue_chunk(chunk_offset);
    }

    fn enqueue_chunk(&mut self, chunk_offset: IVec3) {
        let idx = self.storage_chunk_index(self.storage_chunk_offset(chunk_offset));
        if self.chunk_states[idx] != 0 {
            return;
        }
        self.chunk_states[idx] = 2;
        let center = IVec3::splat(VIEW_RADIUS_CHUNKS);
        let delta = chunk_offset - center;
        let dist2 = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
        let priority = -dist2;
        self.work_queue.push(WorkUnit {
            chunk_offset,
            priority,
        });
    }

    fn pump_cpu(&mut self, world: &World) {
        let mut budget = CPU_VOXEL_BUDGET;

        while budget > 0 {
            let work = if let Some(work) = self.urgent_queue.pop_front() {
                Some(work)
            } else {
                self.work_queue.pop()
            };

            let Some(work_unit) = work else {
                break;
            };

            if !self.is_chunk_in_window(work_unit.chunk_offset) {
                continue;
            }

            let idx = self.storage_chunk_index(self.storage_chunk_offset(work_unit.chunk_offset));
            if self.chunk_states[idx] == 1 {
                continue;
            }

            if budget < CHUNK_VOLUME {
                self.work_queue.push(work_unit);
                break;
            }

            let storage_chunk = self.storage_chunk_offset(work_unit.chunk_offset);
            let base_index = self.storage_chunk_index(storage_chunk) * CHUNK_VOLUME;
            let pool = &self.pool;
            let window_origin = self.window_origin;
            let chunk_offset = work_unit.chunk_offset;
            let chunk_slice = &mut self.data[base_index..base_index + CHUNK_VOLUME];
            Self::compute_chunk(pool, world, window_origin, chunk_offset, chunk_slice);
            let chunk_size = CHUNK_SIZE as usize;
            for z in 0..chunk_size {
                for y in 0..chunk_size {
                    let row_start = base_index + chunk_size * (y + chunk_size * z);
                    self.pending_dirty.push(DirtyRange {
                        start: row_start,
                        end: row_start + chunk_size,
                    });
                }
            }
            self.chunk_states[idx] = 1;
            budget = budget.saturating_sub(CHUNK_VOLUME);
        }
    }

    fn compute_chunk(
        pool: &ThreadPool,
        world: &World,
        window_origin: IVec3,
        chunk_offset: IVec3,
        chunk_slice: &mut [u32],
    ) {
        let chunk_base = window_origin + chunk_offset * CHUNK_SIZE;
        let chunk_size = CHUNK_SIZE as usize;
        let plane_size = chunk_size * chunk_size;

        pool.install(|| {
            chunk_slice
                .par_chunks_mut(plane_size)
                .enumerate()
                .for_each(|(z, plane)| {
                    let voxel_z = chunk_base.z + z as i32;
                    for y in 0..chunk_size {
                        let voxel_y = chunk_base.y + y as i32;
                        let row_start = y * chunk_size;
                        for x in 0..chunk_size {
                            let voxel_x = chunk_base.x + x as i32;
                            let voxel = IVec3::new(voxel_x, voxel_y, voxel_z);
                            let density = world.sample_density(voxel);
                            let material = if density >= 0 {
                                let index = world.sample_material(voxel);
                                world.palette[index as usize]
                            } else {
                                0
                            };
                            plane[row_start + x] = material;
                        }
                    }
                });
        });
    }

    fn pump_gpu(&mut self, queue: &wgpu::Queue) {
        if self.pending_dirty.is_empty() {
            return;
        }

        self.merged_dirty.clear();
        self.merge_dirty_ranges();

        let mut remaining_bytes = GPU_UPLOAD_BUDGET_BYTES;
        self.scratch_dirty.clear();

        for range in self.merged_dirty.drain(..) {
            if remaining_bytes == 0 {
                self.scratch_dirty.push(range);
                continue;
            }
            let bytes = range.len() * std::mem::size_of::<u32>();
            if bytes <= remaining_bytes {
                queue.write_buffer(
                    &self.buffer,
                    (range.start * std::mem::size_of::<u32>()) as u64,
                    bytemuck::cast_slice(&self.data[range.start..range.end]),
                );
                remaining_bytes = remaining_bytes.saturating_sub(bytes);
            } else {
                let max_elements = remaining_bytes / std::mem::size_of::<u32>();
                let end = range.start + max_elements.max(1);
                queue.write_buffer(
                    &self.buffer,
                    (range.start * std::mem::size_of::<u32>()) as u64,
                    bytemuck::cast_slice(&self.data[range.start..end]),
                );
                self.scratch_dirty.push(DirtyRange { start: end, end: range.end });
                remaining_bytes = 0;
            }
        }

        self.pending_dirty.clear();
        self.pending_dirty.append(&mut self.scratch_dirty);
    }

    fn merge_dirty_ranges(&mut self) {
        self.pending_dirty.sort_unstable_by(|a, b| a.start.cmp(&b.start));
        for range in self.pending_dirty.drain(..) {
            if let Some(last) = self.merged_dirty.last_mut() {
                if range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            self.merged_dirty.push(range);
        }
    }

    fn is_chunk_in_window(&self, chunk_offset: IVec3) -> bool {
        let size = VIEW_DIAMETER_CHUNKS;
        chunk_offset.x >= 0
            && chunk_offset.y >= 0
            && chunk_offset.z >= 0
            && chunk_offset.x < size
            && chunk_offset.y < size
            && chunk_offset.z < size
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
