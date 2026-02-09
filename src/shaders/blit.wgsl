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
@group(0) @binding(1)
var color_texture: texture_2d<f32>;
@group(0) @binding(2)
var color_sampler: sampler;

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

fn pack3x5(r0: u32, r1: u32, r2: u32, r3: u32, r4: u32) -> u32 {
  // Each row is 3 bits wide. We store rows bottom-to-top in chunks of 3 bits:
  // bits [0..2]   = row0 (top)
  // bits [3..5]   = row1
  // bits [6..8]   = row2
  // bits [9..11]  = row3
  // bits [12..14] = row4 (bottom)
  // Within each row, bit2 is the LEFT pixel, bit0 is the RIGHT pixel.
  return (r0 & 7u) | ((r1 & 7u) << 3u) | ((r2 & 7u) << 6u) | ((r3 & 7u) << 9u) | ((r4 & 7u) << 12u);
}

fn glyph_mask(code: u32) -> u32 {
  // Normalize to uppercase
  var c = code;
  if (c >= 97u && c <= 122u) { c = c - 32u; }

  switch (c) {
    // space / punctuation
    case 32u: { return pack3x5(0u,0u,0u,0u,0u); }                // ' '
    case 45u: { return pack3x5(0u,0u,7u,0u,0u); }                // '-'
    case 46u: { return pack3x5(0u,0u,0u,0u,2u); }                // '.'
    case 58u: { return pack3x5(0u,2u,0u,2u,0u); }                // ':'

    // digits (optional; you already draw FPS with digit_mask)
    case 48u: { return pack3x5(2u,5u,5u,5u,2u); }                // '0'
    case 49u: { return pack3x5(2u,6u,2u,2u,7u); }                // '1'
    case 50u: { return pack3x5(6u,1u,2u,4u,7u); }                // '2'
    case 51u: { return pack3x5(6u,1u,2u,1u,6u); }                // '3'
    case 52u: { return pack3x5(5u,5u,7u,1u,1u); }                // '4'
    case 53u: { return pack3x5(7u,4u,6u,1u,6u); }                // '5'
    case 54u: { return pack3x5(3u,4u,6u,5u,2u); }                // '6'
    case 55u: { return pack3x5(7u,1u,2u,2u,2u); }                // '7'
    case 56u: { return pack3x5(2u,5u,2u,5u,2u); }                // '8'
    case 57u: { return pack3x5(2u,5u,3u,1u,6u); }                // '9'

    // letters needed for GRASS / LIGHT / DIRT / STONE / WOOD / PLACE / DIG
    case 65u: { return pack3x5(2u,5u,7u,5u,5u); }                // 'A'
    case 67u: { return pack3x5(3u,4u,4u,4u,3u); }                // 'C'
    case 68u: { return pack3x5(6u,5u,5u,5u,6u); }                // 'D'
    case 69u: { return pack3x5(7u,4u,6u,4u,7u); }                // 'E'
    case 71u: { return pack3x5(3u,4u,5u,5u,3u); }                // 'G'
    case 72u: { return pack3x5(5u,5u,7u,5u,5u); }                // 'H'
    case 73u: { return pack3x5(7u,2u,2u,2u,7u); }                // 'I'
    case 70u: { return pack3x5(7u,4u,6u,4u,4u); }                // 'F'
    case 76u: { return pack3x5(4u,4u,4u,4u,7u); }                // 'L'
    case 79u: { return pack3x5(2u,5u,5u,5u,2u); }                // 'O'
    case 80u: { return pack3x5(6u,5u,6u,4u,4u); }                // 'P'
    case 82u: { return pack3x5(6u,5u,6u,5u,5u); }                // 'R'
    case 83u: { return pack3x5(3u,4u,2u,1u,6u); }                // 'S'
    case 84u: { return pack3x5(7u,2u,2u,2u,2u); }                // 'T'
    case 78u: { return pack3x5(5u, 7u, 7u, 7u, 5u); } // 'N'
    case 87u: { return pack3x5(5u, 5u, 5u, 7u, 7u); } // 'W'
    case 86u: { return pack3x5(5u,5u,5u,5u,2u); }                // 'V'
    case 88u: { return pack3x5(5u,5u,2u,5u,5u); }                // 'X'
    
    default: { return 0u; }
  }
}

fn glyph_sample(code: i32, local: vec2<f32>) -> f32 {
    if local.x < 0.0 || local.y < 0.0 || local.x >= 3.0 || local.y >= 5.0 {
        return 0.0;
    }

    let x = u32(floor(local.x));
    let y = u32(floor(local.y));

    // Flip X because glyph bits store bit2=LEFT ... bit0=RIGHT
    let bit_index = y * 3u + (2u - x);

    let mask = glyph_mask(u32(code));
    return select(0.0, 1.0, ((mask >> bit_index) & 1u) == 1u);
}



fn draw_glyph(id: i32, pixel: vec2<f32>, origin: vec2<f32>, scale: f32) -> f32 {
    let local = (pixel - origin) / scale;
    return glyph_sample(id, local);
}

fn draw_text(pixel: vec2<f32>, origin: vec2<f32>, scale: f32, ids: array<i32, 7>) -> f32 {
    var alpha = 0.0;
    var offset = 0.0;
    for (var i = 0; i < 7; i = i + 1) {
        let id = ids[i];
        if id >= 0 {
            let glyph_origin = origin + vec2<f32>(offset, 0.0);
            alpha = max(alpha, draw_glyph(id, pixel, glyph_origin, scale));
        }
        offset = offset + (3.0 * scale) + scale;
    }
    return alpha;
}

fn draw_crosshair(pixel: vec2<f32>, resolution: vec2<f32>) -> f32 {
    let center = resolution * 0.5;
    let diff = abs(pixel - center);
    let thickness = 1.0;
    let length = 8.0;
    let horiz = select(0.0, 1.0, diff.y <= thickness && diff.x <= length);
    let vert = select(0.0, 1.0, diff.x <= thickness && diff.y <= length);
    return max(horiz, vert);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let resolution = uniforms.resolution;
    let pixel = vec2<f32>(input.uv.x * resolution.x, (1.0 - input.uv.y) * resolution.y);
    let scene_color = textureSample(color_texture, color_sampler, input.uv).rgb;
    let scale = 8.0;
    let margin = 20.0;

    let fps_value = max(uniforms.fps, 0.0);
    let fps_int = u32(round(fps_value));
    let hundreds = i32((fps_int / 100u) % 10u);
    let tens = i32((fps_int / 10u) % 10u);
    let ones = i32(fps_int % 10u);
    let show_hundreds = fps_int >= 100u;
    let show_tens = fps_int >= 10u;
    let hundreds_id = select(-1, hundreds, show_hundreds);
    let tens_id = select(-1, tens, show_tens || show_hundreds);

    let glyph_count = 7.0;
    let total_width = glyph_count * (3.0 * scale + scale) - scale;
    let origin = vec2<f32>(resolution.x - margin - total_width, margin);

    let ids = array<i32, 7>(
        70,  // 'F'
        80,  // 'P'
        83,  // 'S'
        58,  // ':'
        select(-1, 48 + hundreds, show_hundreds),                 // '0' + digit
        select(-1, 48 + tens, show_tens || show_hundreds),        // '0' + digit
        48 + ones                                                  // '0' + digit
    );

    let hud_alpha = draw_text(pixel, origin, scale, ids);
    let crosshair_alpha = draw_crosshair(pixel, resolution);
    let alpha = max(hud_alpha, crosshair_alpha);
    let hud_color = vec3<f32>(1.0, 1.0, 1.0);
    let mixed = mix(scene_color, hud_color, alpha);
    return vec4<f32>(mixed, 1.0);
}
