const VIEW_SIZE: i32 = 960;
const VIEW_SIZE_F: f32 = 960.0;
const CHUNK_SIZE: i32 = 64;
const CHUNK_SIZE_F: f32 = 64.0;
const VIEW_DIAMETER_CHUNKS: i32 = 15;
const REGION_SIZE_CHUNKS: i32 = 4;
const VIEW_DIAMETER_REGIONS: i32 =
    (VIEW_DIAMETER_CHUNKS + REGION_SIZE_CHUNKS - 1) / REGION_SIZE_CHUNKS;
const BRICK_SIZE: i32 = 8;
const BRICK_SIZE_F: f32 = 8.0;
const BRICKS_PER_AXIS: i32 = CHUNK_SIZE / BRICK_SIZE;
const BRICKS_PER_CHUNK: i32 = BRICKS_PER_AXIS * BRICKS_PER_AXIS * BRICKS_PER_AXIS;
const BRICK_VOLUME: i32 = BRICK_SIZE * BRICK_SIZE * BRICK_SIZE;
const BRICK_STRIDE_U32: i32 = (BRICK_VOLUME + 3) / 4;
const VIEW_BRICKS: i32 = VIEW_SIZE / BRICK_SIZE;
const REGION_SIZE_VOXELS: i32 = REGION_SIZE_CHUNKS * CHUNK_SIZE;
const REGION_SIZE_VOXELS_F: f32 = f32(REGION_SIZE_VOXELS);
const REGION_COUNT: i32 = VIEW_DIAMETER_REGIONS;
const REGION_COUNT_F: f32 = f32(REGION_COUNT);

struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    fps: f32,
    camera_pos: vec4<f32>,
    camera_forward: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
    chunk_origin: vec4<f32>,
    chunk_wrap_offset: vec4<i32>,
    region_wrap_offset: vec4<i32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<storage, read> chunk_brick_indices: array<u32>;

@group(0) @binding(2)
var<storage, read> brick_materials: array<u32>;

@group(0) @binding(3)
var<storage, read> palette: array<u32>;

@group(0) @binding(4)
var<storage, read> chunk_occupancy: array<u32>;

@group(0) @binding(5)
var<storage, read> region_occupancy: array<u32>;

@group(0) @binding(6)
var output_texture: texture_storage_2d<rgba8unorm, write>;

struct Hit {
    t: f32,
    material: u32,
    normal: vec3<f32>,
};

fn chunk_index(chunk: vec3<i32>) -> i32 {
    let wrapped_chunk = (chunk + uniforms.chunk_wrap_offset.xyz) % VIEW_DIAMETER_CHUNKS;
    return wrapped_chunk.x
        + VIEW_DIAMETER_CHUNKS * (wrapped_chunk.y + VIEW_DIAMETER_CHUNKS * wrapped_chunk.z);
}

fn region_index(region: vec3<i32>) -> i32 {
    let wrapped_region = (region + uniforms.region_wrap_offset.xyz) % VIEW_DIAMETER_REGIONS;
    return wrapped_region.x
        + VIEW_DIAMETER_REGIONS * (wrapped_region.y + VIEW_DIAMETER_REGIONS * wrapped_region.z);
}

fn brick_atlas_id(brick: vec3<i32>) -> u32 {
    if any(brick < vec3<i32>(0)) || any(brick >= vec3<i32>(VIEW_BRICKS)) {
        return 0u;
    }
    let chunk = brick / BRICKS_PER_AXIS;
    let local_brick = brick - chunk * BRICKS_PER_AXIS;
    let chunk_idx = chunk_index(chunk);
    let brick_index = local_brick.x
        + BRICKS_PER_AXIS * (local_brick.y + BRICKS_PER_AXIS * local_brick.z);
    let indirection_index = chunk_idx * BRICKS_PER_CHUNK + brick_index;
    return chunk_brick_indices[u32(indirection_index)];
}

fn material_from_atlas(local_voxel: vec3<i32>, atlas_id: u32) -> u32 {
    if atlas_id == 0u {
        return 0u;
    }
    let local_index = local_voxel.x + BRICK_SIZE * (local_voxel.y + BRICK_SIZE * local_voxel.z);
    let word_index = local_index / 4;
    let shift = (local_index % 4) * 8;
    let atlas_index = (i32(atlas_id) - 1) * BRICK_STRIDE_U32 + word_index;
    let packed = brick_materials[u32(atlas_index)];
    return (packed >> u32(shift)) & 0xFFu;
}

fn palette_color(index: u32) -> vec3<f32> {
    if index == 0u {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let color = palette[index];
    let r = f32(color & 0xFFu) / 255.0;
    let g = f32((color >> 8u) & 0xFFu) / 255.0;
    let b = f32((color >> 16u) & 0xFFu) / 255.0;
    return vec3<f32>(r, g, b);
}

fn ray_brick(
    ro: vec3<f32>,
    rd: vec3<f32>,
    t_start: f32,
    tmax_global: f32,
    brick: vec3<i32>,
    atlas_id: u32,
) -> Hit {
    let inv_dir = 1.0 / rd;
    let brick_min = brick * BRICK_SIZE;
    let brick_max = brick_min + BRICK_SIZE;
    var t = t_start;
    var p = ro + rd * t;
    var voxel = vec3<i32>(floor(p));
    voxel = clamp(voxel, brick_min, brick_max - vec3<i32>(1));
    let step = vec3<i32>(select(-1, 1, rd.x >= 0.0), select(-1, 1, rd.y >= 0.0), select(-1, 1, rd.z >= 0.0));
    let next_boundary = vec3<f32>(
        f32(voxel.x + select(0, 1, rd.x >= 0.0)),
        f32(voxel.y + select(0, 1, rd.y >= 0.0)),
        f32(voxel.z + select(0, 1, rd.z >= 0.0)),
    );
    var tmax_vec = (next_boundary - ro) * inv_dir;
    let tdelta = abs(inv_dir);

    var normal = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0; i < BRICK_VOLUME; i = i + 1) {
        let local_voxel = voxel - brick_min;
        let mat = material_from_atlas(local_voxel, atlas_id);
        if mat > 0u {
            return Hit(t, mat, normal);
        }
        if t > tmax_global {
            break;
        }
        if tmax_vec.x < tmax_vec.y {
            if tmax_vec.x < tmax_vec.z {
                voxel.x = voxel.x + step.x;
                t = tmax_vec.x;
                tmax_vec.x = tmax_vec.x + tdelta.x;
                normal = vec3<f32>(-f32(step.x), 0.0, 0.0);
            } else {
                voxel.z = voxel.z + step.z;
                t = tmax_vec.z;
                tmax_vec.z = tmax_vec.z + tdelta.z;
                normal = vec3<f32>(0.0, 0.0, -f32(step.z));
            }
        } else {
            if tmax_vec.y < tmax_vec.z {
                voxel.y = voxel.y + step.y;
                t = tmax_vec.y;
                tmax_vec.y = tmax_vec.y + tdelta.y;
                normal = vec3<f32>(0.0, -f32(step.y), 0.0);
            } else {
                voxel.z = voxel.z + step.z;
                t = tmax_vec.z;
                tmax_vec.z = tmax_vec.z + tdelta.z;
                normal = vec3<f32>(0.0, 0.0, -f32(step.z));
            }
        }
        if any(voxel < brick_min) || any(voxel >= brick_max) {
            break;
        }
    }
    return Hit(-1.0, 0u, vec3<f32>(0.0, 0.0, 0.0));
}

fn ray_voxel(ro: vec3<f32>, rd: vec3<f32>) -> Hit {
    let bounds_min = vec3<f32>(0.0, 0.0, 0.0);
    let bounds_max = vec3<f32>(VIEW_SIZE_F, VIEW_SIZE_F, VIEW_SIZE_F);
    let inv_dir = 1.0 / rd;
    let t0 = (bounds_min - ro) * inv_dir;
    let t1 = (bounds_max - ro) * inv_dir;
    let tmin = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let tmax = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    if tmax < max(tmin, 0.0) {
        return Hit(-1.0, 0u, vec3<f32>(0.0, 0.0, 0.0));
    }

    var t = max(tmin, 0.0);
    var p = ro + rd * t;
    var voxel = vec3<i32>(floor(p));
    voxel = clamp(voxel, vec3<i32>(0), vec3<i32>(VIEW_SIZE - 1));
    var region = voxel / REGION_SIZE_VOXELS;
    let step_region = vec3<i32>(select(-1, 1, rd.x >= 0.0), select(-1, 1, rd.y >= 0.0), select(-1, 1, rd.z >= 0.0));
    let next_region_boundary = vec3<f32>(
        f32(region.x + select(0, 1, rd.x >= 0.0)) * REGION_SIZE_VOXELS_F,
        f32(region.y + select(0, 1, rd.y >= 0.0)) * REGION_SIZE_VOXELS_F,
        f32(region.z + select(0, 1, rd.z >= 0.0)) * REGION_SIZE_VOXELS_F,
    );
    var tmax_region = (next_region_boundary - ro) * inv_dir;
    let tdelta_region = abs(inv_dir) * REGION_SIZE_VOXELS_F;

    for (var region_step = 0; region_step < 128; region_step = region_step + 1) {
        if any(region < vec3<i32>(0)) || any(region >= vec3<i32>(REGION_COUNT)) {
            break;
        }
        let region_idx = region_index(region);
        if region_occupancy[u32(region_idx)] != 0u {
            var brick = voxel / BRICK_SIZE;
            let step_brick = vec3<i32>(select(-1, 1, rd.x >= 0.0), select(-1, 1, rd.y >= 0.0), select(-1, 1, rd.z >= 0.0));
            let step_chunk = vec3<i32>(select(-1, 1, rd.x >= 0.0), select(-1, 1, rd.y >= 0.0), select(-1, 1, rd.z >= 0.0));
            let next_brick_boundary = vec3<f32>(
                f32(brick.x + select(0, 1, rd.x >= 0.0)) * BRICK_SIZE_F,
                f32(brick.y + select(0, 1, rd.y >= 0.0)) * BRICK_SIZE_F,
                f32(brick.z + select(0, 1, rd.z >= 0.0)) * BRICK_SIZE_F,
            );
            var tmax_brick = (next_brick_boundary - ro) * inv_dir;
            let tdelta_brick = abs(inv_dir) * BRICK_SIZE_F;
            var chunk = brick / BRICKS_PER_AXIS;
            let next_chunk_boundary = vec3<f32>(
                f32(chunk.x + select(0, 1, rd.x >= 0.0)) * CHUNK_SIZE_F,
                f32(chunk.y + select(0, 1, rd.y >= 0.0)) * CHUNK_SIZE_F,
                f32(chunk.z + select(0, 1, rd.z >= 0.0)) * CHUNK_SIZE_F,
            );
            var tmax_chunk = (next_chunk_boundary - ro) * inv_dir;
            let tdelta_chunk = abs(inv_dir) * CHUNK_SIZE_F;

            for (var brick_step = 0; brick_step < 512; brick_step = brick_step + 1) {
                if any(brick < vec3<i32>(0)) || any(brick >= vec3<i32>(VIEW_BRICKS)) {
                    break;
                }
                let chunk_idx = chunk_index(chunk);
                if chunk_occupancy[u32(chunk_idx)] == 0u {
                    if tmax_chunk.x < tmax_chunk.y {
                        if tmax_chunk.x < tmax_chunk.z {
                            chunk.x = chunk.x + step_chunk.x;
                            t = tmax_chunk.x;
                            tmax_chunk.x = tmax_chunk.x + tdelta_chunk.x;
                        } else {
                            chunk.z = chunk.z + step_chunk.z;
                            t = tmax_chunk.z;
                            tmax_chunk.z = tmax_chunk.z + tdelta_chunk.z;
                        }
                    } else {
                        if tmax_chunk.y < tmax_chunk.z {
                            chunk.y = chunk.y + step_chunk.y;
                            t = tmax_chunk.y;
                            tmax_chunk.y = tmax_chunk.y + tdelta_chunk.y;
                        } else {
                            chunk.z = chunk.z + step_chunk.z;
                            t = tmax_chunk.z;
                            tmax_chunk.z = tmax_chunk.z + tdelta_chunk.z;
                        }
                    }
                    brick = vec3<i32>(floor(ro + rd * t)) / BRICK_SIZE;
                    let updated_brick_boundary = vec3<f32>(
                        f32(brick.x + select(0, 1, rd.x >= 0.0)) * BRICK_SIZE_F,
                        f32(brick.y + select(0, 1, rd.y >= 0.0)) * BRICK_SIZE_F,
                        f32(brick.z + select(0, 1, rd.z >= 0.0)) * BRICK_SIZE_F,
                    );
                    tmax_brick = (updated_brick_boundary - ro) * inv_dir;
                    continue;
                }
                let atlas_id = brick_atlas_id(brick);
                if atlas_id != 0u {
                    let hit = ray_brick(ro, rd, t, tmax, brick, atlas_id);
                    if hit.t >= 0.0 {
                        return hit;
                    }
                }
                if t > tmax {
                    break;
                }
                if tmax_brick.x < tmax_brick.y {
                    if tmax_brick.x < tmax_brick.z {
                        brick.x = brick.x + step_brick.x;
                        t = tmax_brick.x;
                        tmax_brick.x = tmax_brick.x + tdelta_brick.x;
                    } else {
                        brick.z = brick.z + step_brick.z;
                        t = tmax_brick.z;
                        tmax_brick.z = tmax_brick.z + tdelta_brick.z;
                    }
                } else {
                    if tmax_brick.y < tmax_brick.z {
                        brick.y = brick.y + step_brick.y;
                        t = tmax_brick.y;
                        tmax_brick.y = tmax_brick.y + tdelta_brick.y;
                    } else {
                        brick.z = brick.z + step_brick.z;
                        t = tmax_brick.z;
                        tmax_brick.z = tmax_brick.z + tdelta_brick.z;
                    }
                }
                chunk = brick / BRICKS_PER_AXIS;
                let updated_chunk_boundary = vec3<f32>(
                    f32(chunk.x + select(0, 1, rd.x >= 0.0)) * CHUNK_SIZE_F,
                    f32(chunk.y + select(0, 1, rd.y >= 0.0)) * CHUNK_SIZE_F,
                    f32(chunk.z + select(0, 1, rd.z >= 0.0)) * CHUNK_SIZE_F,
                );
                tmax_chunk = (updated_chunk_boundary - ro) * inv_dir;
                let region_exit = min(tmax_region.x, min(tmax_region.y, tmax_region.z));
                if t > region_exit {
                    break;
                }
            }
        }
        if t > tmax {
            break;
        }
        if tmax_region.x < tmax_region.y {
            if tmax_region.x < tmax_region.z {
                region.x = region.x + step_region.x;
                t = tmax_region.x;
                tmax_region.x = tmax_region.x + tdelta_region.x;
            } else {
                region.z = region.z + step_region.z;
                t = tmax_region.z;
                tmax_region.z = tmax_region.z + tdelta_region.z;
            }
        } else {
            if tmax_region.y < tmax_region.z {
                region.y = region.y + step_region.y;
                t = tmax_region.y;
                tmax_region.y = tmax_region.y + tdelta_region.y;
            } else {
                region.z = region.z + step_region.z;
                t = tmax_region.z;
                tmax_region.z = tmax_region.z + tdelta_region.z;
            }
        }
        voxel = vec3<i32>(floor(ro + rd * t));
    }
    return Hit(-1.0, 0u, vec3<f32>(0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let resolution = uniforms.resolution;
    if gid.x >= u32(resolution.x) || gid.y >= u32(resolution.y) {
        return;
    }
    let uv = (vec2<f32>(f32(gid.x), f32(gid.y)) / resolution) * 2.0 - vec2<f32>(1.0, 1.0);
    let aspect = resolution.x / resolution.y;
    let screen = vec2<f32>(uv.x * aspect, uv.y);

    let ro = uniforms.camera_pos.xyz - uniforms.chunk_origin.xyz;
    let forward = uniforms.camera_forward.xyz;
    let right = uniforms.camera_right.xyz;
    let up = uniforms.camera_up.xyz;
    let rd = normalize(screen.x * right + screen.y * up + 1.6 * forward);

    let hit = ray_voxel(ro, rd);
    var color = vec3<f32>(0.0, 0.0, 0.0);
    if hit.t < 0.0 {
        color = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.1, 0.2, 0.4), clamp(-uv.y + 0.2, 0.0, 1.0));
    } else {
        let normal = hit.normal;
        let light_dir = normalize(vec3<f32>(0.6, 1.0, 0.4));
        let diffuse = max(dot(normal, light_dir), 0.0);
        let ambient = 0.25;
        let base_color = palette_color(hit.material);
        color = base_color * (ambient + diffuse);
    }
    textureStore(output_texture, vec2<i32>(i32(gid.x), i32(gid.y)), vec4<f32>(color, 1.0));
}
