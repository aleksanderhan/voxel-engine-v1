use std::collections::HashMap;

use glam::{IVec3, Quat, Vec3};

use crate::svo::brick::{Brick, BrickId, BRICK_SIZE};
use crate::svo::coords::{brick_local_from_voxel, VoxelCoord};
use crate::svo::pool::BrickPool;

#[derive(Debug, Clone, Copy)]
pub struct RigidTransform {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl RigidTransform {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

pub struct DynamicComponent {
    pub transform: RigidTransform,
    pub bricks: HashMap<IVec3, BrickId>,
    pub summary_l2: HashMap<IVec3, crate::svo::brick::BrickSummary>,
    pub summary_l1: HashMap<IVec3, crate::svo::brick::BrickSummary>,
    pub summary_l0: crate::svo::brick::BrickSummary,
}

impl DynamicComponent {
    pub fn new(transform: RigidTransform) -> Self {
        Self {
            transform,
            bricks: HashMap::new(),
            summary_l2: HashMap::new(),
            summary_l1: HashMap::new(),
            summary_l0: crate::svo::brick::BrickSummary::empty(),
        }
    }

    pub fn set_voxel(&mut self, pool: &mut BrickPool, voxel: VoxelCoord, density: i8, material: u8) {
        let (brick_coord, in_brick) = brick_local_from_voxel(voxel, BRICK_SIZE);
        let brick_id = self.bricks.entry(brick_coord).or_insert_with(|| pool.allocate());
        let brick = &mut pool.bricks[*brick_id];
        brick.set_voxel(in_brick.x, in_brick.y, in_brick.z, density, material);
        brick.recompute_summary();
    }

    pub fn rebuild_summaries(&mut self, pool: &BrickPool) {
        let mut l2 = HashMap::new();
        for (coord, id) in &self.bricks {
            let cell = *coord / 2;
            let entry = l2.entry(cell).or_insert_with(Vec::new);
            entry.push(pool.bricks[*id].summary);
        }
        self.summary_l2 = l2
            .into_iter()
            .map(|(k, v)| (k, crate::svo::brick::BrickSummary::from_children(v.into_iter())))
            .collect();

        let mut l1 = HashMap::new();
        for (coord, summary) in &self.summary_l2 {
            let cell = *coord / 2;
            let entry = l1.entry(cell).or_insert_with(Vec::new);
            entry.push(*summary);
        }
        self.summary_l1 = l1
            .into_iter()
            .map(|(k, v)| (k, crate::svo::brick::BrickSummary::from_children(v.into_iter())))
            .collect();

        self.summary_l0 = crate::svo::brick::BrickSummary::from_children(
            self.summary_l1.values().copied(),
        );
    }

    pub fn remove_brick(&mut self, coord: IVec3, pool: &mut BrickPool) {
        if let Some(id) = self.bricks.remove(&coord) {
            pool.release(id);
        }
    }

    pub fn voxel_world_to_local(&self, world: VoxelCoord) -> VoxelCoord {
        let translated = Vec3::new(world.x as f32, world.y as f32, world.z as f32)
            - self.transform.translation;
        let rotated = self.transform.rotation.conjugate() * translated;
        IVec3::new(rotated.x.round() as i32, rotated.y.round() as i32, rotated.z.round() as i32)
    }

    pub fn voxel_local_to_world(&self, local: VoxelCoord) -> VoxelCoord {
        let rotated = self.transform.rotation
            * Vec3::new(local.x as f32, local.y as f32, local.z as f32);
        let translated = rotated + self.transform.translation;
        IVec3::new(
            translated.x.round() as i32,
            translated.y.round() as i32,
            translated.z.round() as i32,
        )
    }

    pub fn sample_density(&self, pool: &BrickPool, voxel: VoxelCoord) -> i8 {
        let (brick_coord, in_brick) = brick_local_from_voxel(voxel, BRICK_SIZE);
        if let Some(id) = self.bricks.get(&brick_coord) {
            let brick = &pool.bricks[*id];
            let idx = Brick::index(in_brick.x, in_brick.y, in_brick.z);
            return brick.density[idx];
        }
        i8::MIN
    }

    pub fn is_empty(&self) -> bool {
        self.summary_l0.state == crate::svo::brick::BrickState::Empty
    }
}
