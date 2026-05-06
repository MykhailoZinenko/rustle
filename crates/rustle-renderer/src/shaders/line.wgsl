struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) edge_dist: f32,
    @location(2) half_thickness: f32,
}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) edge_dist: f32,
    @location(3) half_thickness: f32,
) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    out.edge_dist = edge_dist;
    out.half_thickness = half_thickness;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = abs(in.edge_dist) - in.half_thickness;
    let alpha = 1.0 - smoothstep(-0.75, 0.75, d);

    if alpha < 0.001 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
