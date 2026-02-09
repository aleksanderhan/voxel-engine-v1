const VIEW_SIZE: i32 = 960;
const VIEW_SIZE_F: f32 = 960.0;
const CHUNK_SIZE: i32 = 64;
const VIEW_DIAMETER_CHUNKS: i32 = 15;
const BRICK_SIZE: i32 = 8;
const BRICKS_PER_AXIS: i32 = CHUNK_SIZE / BRICK_SIZE;
const BRICKS_PER_CHUNK: i32 = BRICKS_PER_AXIS * BRICKS_PER_AXIS * BRICKS_PER_AXIS;
const BRICK_VOLUME: i32 = BRICK_SIZE * BRICK_SIZE * BRICK_SIZE;
const BRICK_STRIDE_U32: i32 = (BRICK_VOLUME + 3) / 4;

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
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<storage, read> chunk_brick_indices: array<u32>;

@group(0) @binding(2)
var<storage, read> brick_materials: array<u32>;

@group(0) @binding(3)
var<storage, read> palette: array<u32>;
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var output: VertexOutput;
    let pos = positions[vertex_index];
    output.position = vec4<f32>(pos, 0.0, 1.0);
    output.uv = pos * 0.5 + vec2<f32>(0.5, 0.5);
    return output;
}

fn voxel_index(voxel: vec3<i32>) -> vec3<i32> {
    let chunk = voxel / CHUNK_SIZE;
    let local = voxel - chunk * CHUNK_SIZE;
    let wrapped_chunk = (chunk + uniforms.chunk_wrap_offset.xyz) % VIEW_DIAMETER_CHUNKS;
    let chunk_index =
        wrapped_chunk.x + VIEW_DIAMETER_CHUNKS * (wrapped_chunk.y + VIEW_DIAMETER_CHUNKS * wrapped_chunk.z);
    let brick_coord = local / BRICK_SIZE;
    let brick_index = brick_coord.x
        + BRICKS_PER_AXIS * (brick_coord.y + BRICKS_PER_AXIS * brick_coord.z);
    let in_brick = local - brick_coord * BRICK_SIZE;
    let local_index = in_brick.x + BRICK_SIZE * (in_brick.y + BRICK_SIZE * in_brick.z);
    return vec3<i32>(chunk_index, brick_index, local_index);
}

fn load_material(voxel: vec3<i32>) -> u32 {
    if any(voxel < vec3<i32>(0)) || any(voxel >= vec3<i32>(VIEW_SIZE)) {
        return 0u;
    }
    let indices = voxel_index(voxel);
    let chunk_index = indices.x;
    let brick_index = indices.y;
    let local_index = indices.z;
    let indirection_index = chunk_index * BRICKS_PER_CHUNK + brick_index;
    let atlas_id = chunk_brick_indices[u32(indirection_index)];
    if atlas_id == 0u {
        return 0u;
    }
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

struct Hit {
    t: f32,
    material: u32,
    normal: vec3<f32>,
};

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
    let step = vec3<i32>(select(-1, 1, rd.x >= 0.0), select(-1, 1, rd.y >= 0.0), select(-1, 1, rd.z >= 0.0));
    let next_boundary = vec3<f32>(
        f32(voxel.x + select(0, 1, rd.x >= 0.0)),
        f32(voxel.y + select(0, 1, rd.y >= 0.0)),
        f32(voxel.z + select(0, 1, rd.z >= 0.0)),
    );
    var tmax_vec = (next_boundary - ro) * inv_dir;
    let tdelta = abs(inv_dir);

    let start_mat = load_material(voxel);
    if start_mat > 0u {
        return Hit(t, start_mat, vec3<f32>(0.0, 0.0, 0.0));
    }
    var normal = vec3<f32>(0.0, 0.0, 0.0);
    for (var i = 0; i < 1024; i = i + 1) {
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
        let mat = load_material(voxel);
        if mat > 0u {
            return Hit(t, mat, normal);
        }
        if t > tmax {
            break;
        }
    }
    return Hit(-1.0, 0u, vec3<f32>(0.0, 0.0, 0.0));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let resolution = uniforms.resolution;
    let uv = input.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let aspect = resolution.x / resolution.y;
    let screen = vec2<f32>(uv.x * aspect, uv.y);

    let ro = uniforms.camera_pos.xyz - uniforms.chunk_origin.xyz;
    let forward = uniforms.camera_forward.xyz;
    let right = uniforms.camera_right.xyz;
    let up = uniforms.camera_up.xyz;
    let rd = normalize(screen.x * right + screen.y * up + 1.6 * forward);

    let hit = ray_voxel(ro, rd);
    if hit.t < 0.0 {
        let sky = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.1, 0.2, 0.4), clamp(uv.y + 0.2, 0.0, 1.0));
        return vec4<f32>(sky, 1.0);
    }

    let normal = hit.normal;
    let light_dir = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse = max(dot(normal, light_dir), 0.0);
    let ambient = 0.25;

    let base_color = palette_color(hit.material);

    let color = base_color * (ambient + diffuse);
    return vec4<f32>(color, 1.0);
}
