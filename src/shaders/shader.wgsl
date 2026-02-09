struct Uniforms {
    resolution: vec2<f32>,
    time: f32,
    fps: f32,
    camera_pos: vec4<f32>,
    camera_forward: vec4<f32>,
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

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

fn sd_sphere(p: vec3<f32>, center: vec3<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

fn sd_plane(p: vec3<f32>, height: f32) -> f32 {
    return p.y - height;
}

fn scene_sdf(p: vec3<f32>) -> vec2<f32> {
    let ground = sd_plane(p, 0.0);
    let sphere = sd_sphere(p, vec3<f32>(0.0, 1.5, 0.0), 1.0);
    if sphere < ground {
        return vec2<f32>(sphere, 1.0);
    }
    return vec2<f32>(ground, 0.0);
}

fn estimate_normal(p: vec3<f32>) -> vec3<f32> {
    let eps = 0.001;
    let dx = scene_sdf(p + vec3<f32>(eps, 0.0, 0.0)).x - scene_sdf(p - vec3<f32>(eps, 0.0, 0.0)).x;
    let dy = scene_sdf(p + vec3<f32>(0.0, eps, 0.0)).x - scene_sdf(p - vec3<f32>(0.0, eps, 0.0)).x;
    let dz = scene_sdf(p + vec3<f32>(0.0, 0.0, eps)).x - scene_sdf(p - vec3<f32>(0.0, 0.0, eps)).x;
    return normalize(vec3<f32>(dx, dy, dz));
}

fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> vec2<f32> {
    var t = 0.0;
    var material = -1.0;
    for (var i = 0; i < 96; i = i + 1) {
        let p = ro + rd * t;
        let res = scene_sdf(p);
        if res.x < 0.001 {
            material = res.y;
            break;
        }
        t = t + res.x;
        if t > 100.0 {
            break;
        }
    }
    return vec2<f32>(t, material);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let resolution = uniforms.resolution;
    let uv = input.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let aspect = resolution.x / resolution.y;
    let screen = vec2<f32>(uv.x * aspect, uv.y);

    let ro = uniforms.camera_pos.xyz;
    let forward = uniforms.camera_forward.xyz;
    let right = uniforms.camera_right.xyz;
    let up = uniforms.camera_up.xyz;
    let rd = normalize(screen.x * right + screen.y * up + 1.6 * forward);

    let hit = raymarch(ro, rd);
    if hit.y < 0.0 {
        let sky = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.1, 0.2, 0.4), clamp(uv.y + 0.2, 0.0, 1.0));
        return vec4<f32>(sky, 1.0);
    }

    let p = ro + rd * hit.x;
    let normal = estimate_normal(p);
    let light_dir = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse = max(dot(normal, light_dir), 0.0);
    let ambient = 0.25;

    var base_color = vec3<f32>(0.1, 0.6, 0.2);
    if hit.y > 0.5 {
        base_color = vec3<f32>(0.75, 0.75, 0.8);
    }

    let color = base_color * (ambient + diffuse);
    return vec4<f32>(color, 1.0);
}
