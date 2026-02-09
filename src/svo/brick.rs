use std::fmt;

pub type BrickId = usize;

pub const BRICK_SIZE: i32 = 8;
pub const BRICK_VOLUME: usize = (BRICK_SIZE as usize) * (BRICK_SIZE as usize) * (BRICK_SIZE as usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrickState {
    Empty,
    Full,
    Mixed,
}

#[derive(Clone, Copy)]
pub struct BrickSummary {
    pub min_d: i8,
    pub max_d: i8,
    pub state: BrickState,
}

impl BrickSummary {
    pub fn empty() -> Self {
        Self {
            min_d: i8::MIN,
            max_d: i8::MIN,
            state: BrickState::Empty,
        }
    }

    pub fn from_children(children: impl Iterator<Item = BrickSummary>) -> Self {
        let mut min_d = i8::MAX;
        let mut max_d = i8::MIN;
        let mut any = false;
        for child in children {
            any = true;
            min_d = min_d.min(child.min_d);
            max_d = max_d.max(child.max_d);
        }
        if !any {
            return Self::empty();
        }
        let state = if max_d < 0 {
            BrickState::Empty
        } else if min_d >= 0 {
            BrickState::Full
        } else {
            BrickState::Mixed
        };
        Self { min_d, max_d, state }
    }
}

impl fmt::Debug for BrickSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrickSummary")
            .field("min_d", &self.min_d)
            .field("max_d", &self.max_d)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone)]
pub struct Brick {
    pub density: [i8; BRICK_VOLUME],
    pub material: [u8; BRICK_VOLUME],
    pub summary: BrickSummary,
}

impl Brick {
    pub fn new_empty() -> Self {
        Self {
            density: [i8::MIN; BRICK_VOLUME],
            material: [0; BRICK_VOLUME],
            summary: BrickSummary::empty(),
        }
    }

    pub fn index(x: i32, y: i32, z: i32) -> usize {
        let x = x as usize;
        let y = y as usize;
        let z = z as usize;
        x + (BRICK_SIZE as usize) * (y + (BRICK_SIZE as usize) * z)
    }

    pub fn set_voxel(&mut self, x: i32, y: i32, z: i32, density: i8, material: u8) {
        let idx = Self::index(x, y, z);
        self.density[idx] = density;
        self.material[idx] = material;
    }

    pub fn recompute_summary(&mut self) {
        let mut min_d = i8::MAX;
        let mut max_d = i8::MIN;
        for &d in &self.density {
            min_d = min_d.min(d);
            max_d = max_d.max(d);
        }
        let state = if max_d < 0 {
            BrickState::Empty
        } else if min_d >= 0 {
            BrickState::Full
        } else {
            BrickState::Mixed
        };
        self.summary = BrickSummary { min_d, max_d, state };
    }
}
