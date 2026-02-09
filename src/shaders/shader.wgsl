struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    fps: f32,
    camera_pos: vec4<f32>,
    camera_forward: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
    chunk_origin: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<storage, read> chunk_data: array<u32>;

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

fn voxel_index(voxel: vec3<i32>) -> u32 {
    let size = 64;
    return u32(voxel.x + size * (voxel.y + size * voxel.z));
}

fn load_material(voxel: vec3<i32>) -> u32 {
    if any(voxel < vec3<i32>(0)) || any(voxel >= vec3<i32>(64)) {
        return 0u;
    }
    let idx = voxel_index(voxel);
    return chunk_data[idx];
}

fn estimate_normal(voxel: vec3<i32>) -> vec3<f32> {
    let dx = f32(load_material(voxel + vec3<i32>(1, 0, 0))) - f32(load_material(voxel - vec3<i32>(1, 0, 0)));
    let dy = f32(load_material(voxel + vec3<i32>(0, 1, 0))) - f32(load_material(voxel - vec3<i32>(0, 1, 0)));
    let dz = f32(load_material(voxel + vec3<i32>(0, 0, 1))) - f32(load_material(voxel - vec3<i32>(0, 0, 1)));
    return normalize(vec3<f32>(dx, dy, dz));
}

fn ray_voxel(ro: vec3<f32>, rd: vec3<f32>) -> vec4<f32> {
    let bounds_min = vec3<f32>(0.0, 0.0, 0.0);
    let bounds_max = vec3<f32>(64.0, 64.0, 64.0);
    let inv_dir = 1.0 / rd;
    let t0 = (bounds_min - ro) * inv_dir;
    let t1 = (bounds_max - ro) * inv_dir;
    let tmin = max(max(min(t0.x, t1.x), min(t0.y, t1.y)), min(t0.z, t1.z));
    let tmax = min(min(max(t0.x, t1.x), max(t0.y, t1.y)), max(t0.z, t1.z));
    if tmax < max(tmin, 0.0) {
        return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
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

    for (var i = 0; i < 512; i = i + 1) {
        let mat = load_material(voxel);
        if mat > 0u {
            return vec4<f32>(t, f32(mat), f32(voxel.x), f32(voxel.y));
        }
        if tmax_vec.x < tmax_vec.y {
            if tmax_vec.x < tmax_vec.z {
                voxel.x = voxel.x + step.x;
                t = tmax_vec.x;
                tmax_vec.x = tmax_vec.x + tdelta.x;
            } else {
                voxel.z = voxel.z + step.z;
                t = tmax_vec.z;
                tmax_vec.z = tmax_vec.z + tdelta.z;
            }
        } else {
            if tmax_vec.y < tmax_vec.z {
                voxel.y = voxel.y + step.y;
                t = tmax_vec.y;
                tmax_vec.y = tmax_vec.y + tdelta.y;
            } else {
                voxel.z = voxel.z + step.z;
                t = tmax_vec.z;
                tmax_vec.z = tmax_vec.z + tdelta.z;
            }
        }
        if t > tmax {
            break;
        }
    }
    return vec4<f32>(-1.0, 0.0, 0.0, 0.0);
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
    if hit.x < 0.0 {
        let sky = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.1, 0.2, 0.4), clamp(uv.y + 0.2, 0.0, 1.0));
        return vec4<f32>(sky, 1.0);
    }

    let p = ro + rd * hit.x;
    let voxel = vec3<i32>(floor(p));
    let normal = estimate_normal(voxel);
    let light_dir = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse = max(dot(normal, light_dir), 0.0);
    let ambient = 0.25;

    let palette = vec3<f32>(0.2, 0.6, 0.9);
    let material = hit.y / 255.0;
    let base_color = mix(vec3<f32>(0.1, 0.1, 0.1), palette, material);

    let color = base_color * (ambient + diffuse);
    return vec4<f32>(color, 1.0);
}
