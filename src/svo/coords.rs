use glam::IVec3;

pub type VoxelCoord = IVec3;

pub fn div_floor(value: i32, divisor: i32) -> (i32, i32) {
    let mut quotient = value / divisor;
    let mut remainder = value % divisor;
    if remainder < 0 {
        quotient -= 1;
        remainder += divisor;
    }
    (quotient, remainder)
}

pub fn chunk_local_from_voxel(voxel: VoxelCoord, chunk_size: i32) -> (IVec3, IVec3) {
    let (cx, lx) = div_floor(voxel.x, chunk_size);
    let (cy, ly) = div_floor(voxel.y, chunk_size);
    let (cz, lz) = div_floor(voxel.z, chunk_size);
    (IVec3::new(cx, cy, cz), IVec3::new(lx, ly, lz))
}

pub fn brick_local_from_voxel(local_voxel: IVec3, brick_size: i32) -> (IVec3, IVec3) {
    let (bx, ix) = div_floor(local_voxel.x, brick_size);
    let (by, iy) = div_floor(local_voxel.y, brick_size);
    let (bz, iz) = div_floor(local_voxel.z, brick_size);
    (IVec3::new(bx, by, bz), IVec3::new(ix, iy, iz))
}
