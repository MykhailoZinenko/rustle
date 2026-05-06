// SDF instanced quad shader for Rustle renderer
// Renders circles, rects, and lines using signed distance fields.

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
    @location(3) line_dir: vec4<f32>,
    @location(4) size_px: vec2<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shape_params: vec4<f32>,
    @location(4) line_dir: vec4<f32>,
) -> VertexOutput {
    // Generate quad from 6 vertices (two triangles)
    // Triangle 1: 0,1,2  Triangle 2: 2,1,3
    // Positions: (-1,-1), (1,-1), (-1,1), (1,1)
    let quad_indices = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let quad_pos = quad_indices[vertex_index];

    // AA margin: 2 pixels in NDC
    let aa_margin = 2.0 / uniforms.viewport;

    // Expand quad by AA margin
    let expanded_size = size + aa_margin;
    let clip_pos = center + quad_pos * expanded_size;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_pos, 0.0, 1.0);
    out.uv = quad_pos;
    out.color = color;
    out.shape_params = shape_params;
    out.line_dir = line_dir;
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

    // Anti-aliasing width in UV space — use the thinnest axis so AA is wide enough
    let aa = 1.5 / max(min(size_px.x, size_px.y), 1.0);

    var d: f32;

    switch shape_type {
        case 0: {
            // Circle: distance from center minus radius (radius = 1 in UV space)
            d = length(uv) - 1.0;
        }
        case 1: {
            // Rect: box SDF with optional corner radius
            let cr = corner_radius;
            let q = abs(uv) - (vec2<f32>(1.0) - cr);
            d = min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - cr;
        }
        case 2: {
            // Line: SDF for an arbitrary-angle line segment.
            // line_dir.xy = normalized direction (NDC), line_dir.z = half-length (NDC),
            // line_dir.w = thickness (NDC).
            let dir = in.line_dir.xy;
            let half_len_ndc = in.line_dir.z;
            let thickness_ndc = in.line_dir.w;

            // Fragment position in NDC-offset from the quad center
            // uv is [-1,1], size (half-extent in NDC) = size_px / (viewport * 0.5)
            let size_ndc = size_px / (uniforms.viewport * 0.5);
            let p_ndc = uv * size_ndc;

            // Project onto line direction: along-line and perpendicular components
            let along = dot(p_ndc, dir);
            let perp = dot(p_ndc, vec2<f32>(-dir.y, dir.x));

            // Signed distance to the line segment in NDC
            let clamped_along = clamp(along, -half_len_ndc, half_len_ndc);
            let dist_ndc = length(vec2<f32>(along - clamped_along, perp)) - thickness_ndc;

            // Convert NDC distance to UV-space for consistent AA with other shapes.
            // 1 UV unit spans size_ndc, so scale = 1 / min(size_ndc) to use thinnest axis.
            let min_size = max(min(size_ndc.x, size_ndc.y), 0.0001);
            d = dist_ndc / min_size;
        }
        default: {
            d = length(uv) - 1.0;
        }
    }

    var alpha: f32;

    switch render_mode {
        case 0, 3: {
            // Fill / SDF visualization
            alpha = 1.0 - smoothstep(-aa, aa, d);
        }
        case 1: {
            // Outline (default 1px width in UV space)
            let outline_width = aa * 2.0;
            alpha = 1.0 - smoothstep(-aa, aa, abs(d) - outline_width);
        }
        case 2: {
            // Stroke with user-specified width
            // Convert stroke_width from pixels to UV space
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
