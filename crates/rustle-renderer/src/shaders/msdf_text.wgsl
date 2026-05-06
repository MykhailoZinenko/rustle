@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) px_range: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) px_range: f32,
) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    out.px_range = px_range;
    return out;
}

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let msd = textureSample(atlas_tex, atlas_sampler, in.uv);
    let sd = median(msd.r, msd.g, msd.b);
    let screen_px_dist = in.px_range * (sd - 0.5);
    let alpha = clamp(screen_px_dist + 0.5, 0.0, 1.0);

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
