use glam::IVec3;

use crate::svo::brick::{BrickId, BrickState, BrickSummary, BRICK_SIZE};
use crate::svo::pool::BrickPool;

pub const CHUNK_SIZE: i32 = 64;
pub const BRICKS_PER_AXIS: i32 = CHUNK_SIZE / BRICK_SIZE;
pub const BRICKS_PER_CHUNK: usize = (BRICKS_PER_AXIS as usize)
    * (BRICKS_PER_AXIS as usize)
    * (BRICKS_PER_AXIS as usize);

pub const L2_AXIS: i32 = 4;
pub const L1_AXIS: i32 = 2;

#[derive(Clone)]
pub struct Chunk {
    pub bricks: Vec<Option<BrickId>>,
    pub summary_l2: Vec<BrickSummary>,
    pub summary_l1: Vec<BrickSummary>,
    pub summary_l0: BrickSummary,
}

impl Chunk {
    pub fn new_empty() -> Self {
        Self {
            bricks: vec![None; BRICKS_PER_CHUNK],
            summary_l2: vec![BrickSummary::empty(); (L2_AXIS * L2_AXIS * L2_AXIS) as usize],
            summary_l1: vec![BrickSummary::empty(); (L1_AXIS * L1_AXIS * L1_AXIS) as usize],
            summary_l0: BrickSummary::empty(),
        }
    }

    pub fn brick_index(local_brick: IVec3) -> usize {
        let x = local_brick.x as usize;
        let y = local_brick.y as usize;
        let z = local_brick.z as usize;
        x + (BRICKS_PER_AXIS as usize) * (y + (BRICKS_PER_AXIS as usize) * z)
    }

    pub fn l2_index(cell: IVec3) -> usize {
        let x = cell.x as usize;
        let y = cell.y as usize;
        let z = cell.z as usize;
        x + (L2_AXIS as usize) * (y + (L2_AXIS as usize) * z)
    }

    pub fn l1_index(cell: IVec3) -> usize {
        let x = cell.x as usize;
        let y = cell.y as usize;
        let z = cell.z as usize;
        x + (L1_AXIS as usize) * (y + (L1_AXIS as usize) * z)
    }

    pub fn ensure_brick(&mut self, pool: &mut BrickPool, local_brick: IVec3) -> BrickId {
        let idx = Self::brick_index(local_brick);
        if let Some(id) = self.bricks[idx] {
            return id;
        }
        let id = pool.allocate();
        self.bricks[idx] = Some(id);
        id
    }

    pub fn update_summaries_for_brick(&mut self, pool: &BrickPool, local_brick: IVec3) {
        let l2_cell = local_brick / 2;
        self.summary_l2[Self::l2_index(l2_cell)] = self.aggregate_l2(pool, l2_cell);

        let l1_cell = l2_cell / 2;
        self.summary_l1[Self::l1_index(l1_cell)] = self.aggregate_l1(l1_cell);

        self.summary_l0 = self.aggregate_l0();
    }

    pub fn recompute_all_summaries(&mut self, pool: &BrickPool) {
        for z in 0..L2_AXIS {
            for y in 0..L2_AXIS {
                for x in 0..L2_AXIS {
                    let cell = IVec3::new(x, y, z);
                    let idx = Self::l2_index(cell);
                    self.summary_l2[idx] = self.aggregate_l2(pool, cell);
                }
            }
        }

        for z in 0..L1_AXIS {
            for y in 0..L1_AXIS {
                for x in 0..L1_AXIS {
                    let cell = IVec3::new(x, y, z);
                    let idx = Self::l1_index(cell);
                    self.summary_l1[idx] = self.aggregate_l1(cell);
                }
            }
        }

        self.summary_l0 = self.aggregate_l0();
    }

    fn aggregate_l2(&self, pool: &BrickPool, cell: IVec3) -> BrickSummary {
        let mut summaries = Vec::with_capacity(8);
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    let brick_coord = cell * 2 + IVec3::new(dx, dy, dz);
                    let idx = Self::brick_index(brick_coord);
                    if let Some(id) = self.bricks[idx] {
                        summaries.push(pool.bricks[id].summary);
                    } else {
                        summaries.push(BrickSummary::empty());
                    }
                }
            }
        }
        BrickSummary::from_children(summaries.into_iter())
    }

    fn aggregate_l1(&self, cell: IVec3) -> BrickSummary {
        let mut summaries = Vec::with_capacity(8);
        for dz in 0..2 {
            for dy in 0..2 {
                for dx in 0..2 {
                    let l2_cell = cell * 2 + IVec3::new(dx, dy, dz);
                    let idx = Self::l2_index(l2_cell);
                    summaries.push(self.summary_l2[idx]);
                }
            }
        }
        BrickSummary::from_children(summaries.into_iter())
    }

    fn aggregate_l0(&self) -> BrickSummary {
        BrickSummary::from_children(self.summary_l1.iter().copied())
    }

    pub fn is_empty(&self) -> bool {
        self.summary_l0.state == BrickState::Empty
    }
}
