struct Uniforms {
    viewport: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shape_params: vec4<f32>,
    @location(3) size_px: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shape_params: vec4<f32>,
) -> VertexOutput {
    let quad_indices = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let quad_pos = quad_indices[vertex_index];

    // Expand quad by a margin so fragments exist beyond the shape boundary for AA.
    let aa_margin = 4.0 / uniforms.viewport;
    let expanded_size = size + aa_margin;
    let clip_pos = center + quad_pos * expanded_size;

    // Scale UV so |uv| = 1 maps to the *original* shape boundary (not expanded edge).
    // This leaves the expansion area at |uv| > 1 for the AA fade to complete smoothly.
    let uv_scale = expanded_size / max(size, vec2<f32>(0.0001));

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_pos, 0.0, 1.0);
    out.uv = quad_pos * uv_scale;
    out.color = color;
    out.shape_params = shape_params;
    out.size_px = size * uniforms.viewport * 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let shape_type = i32(in.shape_params.x);
    let render_mode = i32(in.shape_params.y);
    let stroke_width = in.shape_params.z;
    let corner_radius = in.shape_params.w;

    let uv = in.uv;
    let size_px = in.size_px;

    let aa = 1.5 / max(min(size_px.x, size_px.y), 1.0);

    var d: f32;

    switch shape_type {
        case 0: {
            // Circle
            d = length(uv) - 1.0;
        }
        case 1: {
            // Rect with optional corner radius
            let cr = corner_radius;
            let q = abs(uv) - (vec2<f32>(1.0) - cr);
            d = min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - cr;
        }
        default: {
            d = length(uv) - 1.0;
        }
    }

    var alpha: f32;

    switch render_mode {
        case 0, 3: {
            // Fill / SDF
            alpha = 1.0 - smoothstep(-aa, aa, d);
        }
        case 1: {
            // Outline
            let outline_width = aa * 2.0;
            alpha = 1.0 - smoothstep(-aa, aa, abs(d) - outline_width);
        }
        case 2: {
            // Stroke with user-specified width (pixels → UV space)
            let w = stroke_width / max(size_px.x, size_px.y);
            alpha = 1.0 - smoothstep(-aa, aa, abs(d) - w);
        }
        default: {
            alpha = 1.0 - smoothstep(-aa, aa, d);
        }
    }

    if alpha < 0.001 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
