use std::collections::{HashMap, HashSet};

use glam::{IVec3, Vec3};

use crate::svo::brick::{Brick, BRICK_SIZE};
use crate::svo::chunk::{Chunk, CHUNK_SIZE};
use crate::svo::coords::{brick_local_from_voxel, chunk_local_from_voxel, VoxelCoord};
use crate::svo::dynamic::{DynamicComponent, RigidTransform};
use crate::svo::pool::BrickPool;
use crate::svo::vox::{VoxFile, VoxModel};

pub struct World {
    pub chunks: HashMap<IVec3, Chunk>,
    pub brick_pool: BrickPool,
    pub dynamic_components: Vec<DynamicComponent>,
    pub palette: [u32; 256],
}

impl World {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            brick_pool: BrickPool::default(),
            dynamic_components: Vec::new(),
            palette: [0u32; 256],
        }
    }

    pub fn get_or_create_chunk(&mut self, chunk_coord: IVec3) -> &mut Chunk {
        self.chunks.entry(chunk_coord).or_insert_with(Chunk::new_empty)
    }

    pub fn set_voxel(&mut self, voxel: VoxelCoord, density: i8, material: u8) {
        let (chunk_coord, local_voxel) = chunk_local_from_voxel(voxel, CHUNK_SIZE);
        let (brick_coord, in_brick) = brick_local_from_voxel(local_voxel, BRICK_SIZE);
        let (pool, chunks) = (&mut self.brick_pool, &mut self.chunks);
        let chunk = chunks.entry(chunk_coord).or_insert_with(Chunk::new_empty);
        let brick_id = chunk.ensure_brick(pool, brick_coord);
        let brick = &mut pool.bricks[brick_id];
        brick.set_voxel(in_brick.x, in_brick.y, in_brick.z, density, material);
        brick.recompute_summary();
        chunk.update_summaries_for_brick(pool, brick_coord);
    }

    pub fn remove_voxel(&mut self, voxel: VoxelCoord) {
        self.set_voxel(voxel, i8::MIN, 0);
    }

    pub fn import_vox_file(&mut self, vox: &VoxFile, origin: VoxelCoord) {
        self.palette = vox.palette;
        for model in &vox.models {
            self.import_vox_model(model, origin);
        }
    }

    pub fn import_vox_model(&mut self, model: &VoxModel, origin: VoxelCoord) {
        let mut dirty_bricks: HashMap<IVec3, HashSet<IVec3>> = HashMap::new();
        for voxel in &model.voxels {
            let world = origin + IVec3::new(voxel.x as i32, voxel.z as i32, voxel.y as i32);
            let (chunk_coord, local_voxel) = chunk_local_from_voxel(world, CHUNK_SIZE);
            let (brick_coord, in_brick) = brick_local_from_voxel(local_voxel, BRICK_SIZE);
            let (pool, chunks) = (&mut self.brick_pool, &mut self.chunks);
            let chunk = chunks.entry(chunk_coord).or_insert_with(Chunk::new_empty);
            let brick_id = chunk.ensure_brick(pool, brick_coord);
            let brick = &mut pool.bricks[brick_id];
            brick.set_voxel(
                in_brick.x,
                in_brick.y,
                in_brick.z,
                i8::MAX,
                voxel.color_index,
            );
            dirty_bricks
                .entry(chunk_coord)
                .or_default()
                .insert(brick_coord);
        }

        for (chunk_coord, bricks) in dirty_bricks {
            if let Some(chunk) = self.chunks.get_mut(&chunk_coord) {
                for brick_coord in bricks {
                    if let Some(brick_id) =
                        chunk.bricks[Chunk::brick_index(brick_coord)]
                    {
                        self.brick_pool.bricks[brick_id].recompute_summary();
                    }
                    chunk.update_summaries_for_brick(&self.brick_pool, brick_coord);
                }
            }
        }
    }

    pub fn detach_component(&mut self, voxels: &[VoxelCoord]) -> DynamicComponent {
        let mut component = DynamicComponent::new(RigidTransform::identity());
        for voxel in voxels {
            let (chunk_coord, local_voxel) = chunk_local_from_voxel(*voxel, CHUNK_SIZE);
            let (brick_coord, in_brick) = brick_local_from_voxel(local_voxel, BRICK_SIZE);
            let brick_id = self
                .chunks
                .get(&chunk_coord)
                .and_then(|chunk| chunk.bricks[Chunk::brick_index(brick_coord)]);
            if let Some(brick_id) = brick_id {
                let idx = Brick::index(in_brick.x, in_brick.y, in_brick.z);
                let density = self.brick_pool.bricks[brick_id].density[idx];
                let material = self.brick_pool.bricks[brick_id].material[idx];
                if density >= 0 {
                    component.set_voxel(&mut self.brick_pool, *voxel, density, material);
                    let brick = &mut self.brick_pool.bricks[brick_id];
                    brick.density[idx] = i8::MIN;
                    brick.material[idx] = 0;
                    brick.recompute_summary();
                    if let Some(chunk) = self.chunks.get_mut(&chunk_coord) {
                        chunk.update_summaries_for_brick(&self.brick_pool, brick_coord);
                    }
                }
            }
        }
        component
    }

    pub fn compute_density_gradient(&self, voxel: VoxelCoord) -> Vec3 {
        let offsets = [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ];
        let mut samples = [i8::MIN; 6];
        for (i, offset) in offsets.iter().enumerate() {
            samples[i] = self.sample_density(voxel + *offset);
        }
        Vec3::new(
            (samples[0] as f32) - (samples[1] as f32),
            (samples[2] as f32) - (samples[3] as f32),
            (samples[4] as f32) - (samples[5] as f32),
        )
        .normalize_or_zero()
    }

    pub fn sample_density(&self, voxel: VoxelCoord) -> i8 {
        let (chunk_coord, local_voxel) = chunk_local_from_voxel(voxel, CHUNK_SIZE);
        let (brick_coord, in_brick) = brick_local_from_voxel(local_voxel, BRICK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_coord) {
            if let Some(brick_id) = chunk.bricks[Chunk::brick_index(brick_coord)] {
                let brick = &self.brick_pool.bricks[brick_id];
                let idx = Brick::index(in_brick.x, in_brick.y, in_brick.z);
                return brick.density[idx];
            }
        }
        i8::MIN
    }

    pub fn sample_material(&self, voxel: VoxelCoord) -> u8 {
        let (chunk_coord, local_voxel) = chunk_local_from_voxel(voxel, CHUNK_SIZE);
        let (brick_coord, in_brick) = brick_local_from_voxel(local_voxel, BRICK_SIZE);
        if let Some(chunk) = self.chunks.get(&chunk_coord) {
            if let Some(brick_id) = chunk.bricks[Chunk::brick_index(brick_coord)] {
                let brick = &self.brick_pool.bricks[brick_id];
                let idx = Brick::index(in_brick.x, in_brick.y, in_brick.z);
                return brick.material[idx];
            }
        }
        0
    }
}
