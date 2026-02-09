use crate::svo::brick::{Brick, BrickId};

#[derive(Default)]
pub struct BrickPool {
    pub bricks: Vec<Brick>,
    free_list: Vec<BrickId>,
}

impl BrickPool {
    pub fn allocate(&mut self) -> BrickId {
        if let Some(id) = self.free_list.pop() {
            self.bricks[id] = Brick::new_empty();
            id
        } else {
            let id = self.bricks.len();
            self.bricks.push(Brick::new_empty());
            id
        }
    }

    pub fn release(&mut self, id: BrickId) {
        if id < self.bricks.len() {
            self.free_list.push(id);
        }
    }
}
