use rustle_lang::{DrawCommand, RenderMode, ShapeData, ShapeDesc, origin_offset};

use crate::instance::{LineVertex, PolygonVertex, SdfInstance};

pub struct PreparedFrame {
    pub(crate) sdf_instances: Vec<SdfInstance>,
    pub(crate) line_vertices: Vec<LineVertex>,
    pub(crate) line_indices: Vec<u32>,
    pub(crate) polygon_vertices: Vec<PolygonVertex>,
    pub(crate) polygon_indices: Vec<u32>,
}

#[must_use]
pub fn prepare(commands: &[DrawCommand]) -> PreparedFrame {
    let mut sdf_instances = Vec::new();
    let mut line_vertices = Vec::new();
    let mut line_indices = Vec::new();
    let mut polygon_vertices = Vec::new();
    let mut polygon_indices = Vec::new();

    for cmd in commands {
        let DrawCommand::DrawShape(data) = cmd else {
            continue;
        };
        match &data.desc {
            ShapeDesc::Circle { .. } => {
                if let Some(inst) = build_circle(data) {
                    sdf_instances.push(inst);
                }
            }
            ShapeDesc::Rect { .. } => {
                if let Some(inst) = build_rect(data) {
                    sdf_instances.push(inst);
                }
            }
            ShapeDesc::Line { .. } => {
                build_line(data, &mut line_vertices, &mut line_indices);
            }
            ShapeDesc::Polygon(pts) => match &data.render_mode {
                RenderMode::Outline | RenderMode::Stroke(_) => {
                    build_polygon_outline(data, pts, &mut line_vertices, &mut line_indices);
                }
                _ => {
                    build_polygon_fill(data, pts, &mut polygon_vertices, &mut polygon_indices);
                }
            },
            _ => {}
        }
    }

    PreparedFrame {
        sdf_instances,
        line_vertices,
        line_indices,
        polygon_vertices,
        polygon_indices,
    }
}

// ─── Coordinate helpers ──────────────────────────────────────────────────────

fn color_f32(data: &ShapeData) -> [f32; 4] {
    [
        data.color[0] as f32,
        data.color[1] as f32,
        data.color[2] as f32,
        data.color[3] as f32,
    ]
}

/// Accumulate translation and scale from transforms.
/// Returns (tx, ty) in user coords and (sx, sy) scale factors.
fn accumulate_transforms(data: &ShapeData) -> (f64, f64, f64, f64) {
    let mut tx = 0.0;
    let mut ty = 0.0;
    let mut sx = 1.0;
    let mut sy = 1.0;
    for t in &data.transforms {
        tx += t.tx;
        ty += t.ty;
        sx *= t.sx;
        sy *= t.sy;
    }
    (tx, ty, sx, sy)
}

/// Convert a user-coordinate point (with transforms applied) to NDC.
fn to_ndc(data: &ShapeData, x: f64, y: f64) -> (f64, f64) {
    let meta = &data.coord_meta;
    (meta.x_to_ndc(x), meta.y_to_ndc(y))
}

/// Apply transforms to a point relative to a pivot, return in user coords.
fn apply_transform(x: f64, y: f64, pivot_x: f64, pivot_y: f64, tx: f64, ty: f64, sx: f64, sy: f64) -> (f64, f64) {
    let rx = pivot_x + (x - pivot_x) * sx + tx;
    let ry = pivot_y + (y - pivot_y) * sy + ty;
    (rx, ry)
}

// ─── Render mode → SDF params ────────────────────────────────────────────────

#[expect(clippy::match_same_arms, reason = "non_exhaustive enum requires wildcard arm")]
fn render_mode_code(mode: &RenderMode) -> f32 {
    match mode {
        RenderMode::Fill => 0.0,
        RenderMode::Outline => 1.0,
        RenderMode::Stroke(_) => 2.0,
        RenderMode::Sdf => 3.0,
        _ => 0.0,
    }
}

fn stroke_width_px(mode: &RenderMode) -> f32 {
    match mode {
        RenderMode::Stroke(w) => *w as f32,
        _ => 0.0,
    }
}

// ─── Circle (SDF) ────────────────────────────────────────────────────────────

fn build_circle(data: &ShapeData) -> Option<SdfInstance> {
    let ShapeDesc::Circle { center, radius } = &data.desc else {
        return None;
    };
    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);

    let cx = center.0 + tx;
    let cy = center.1 + ty;
    let (ndc_x, ndc_y) = to_ndc(data, cx, cy);
    let r_ndc_x = meta.w_to_ndc(*radius) * sx;
    let r_ndc_y = meta.h_to_ndc(*radius) * sy;

    Some(SdfInstance {
        center: [ndc_x as f32, ndc_y as f32],
        size: [r_ndc_x as f32, r_ndc_y as f32],
        color: color_f32(data),
        shape_params: [
            0.0,
            render_mode_code(&data.render_mode),
            stroke_width_px(&data.render_mode),
            0.0,
        ],
    })
}

// ─── Rect (SDF) ──────────────────────────────────────────────────────────────

fn build_rect(data: &ShapeData) -> Option<SdfInstance> {
    let ShapeDesc::Rect { center, size, origin } = &data.desc else {
        return None;
    };
    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);

    let hw_ndc = meta.w_to_ndc(size.0 / 2.0) * sx;
    let hh_ndc = meta.h_to_ndc(size.1 / 2.0) * sy;
    let (off_x, off_y) = origin_offset(origin, hw_ndc, hh_ndc);

    let cx = center.0 + tx;
    let cy = center.1 + ty;
    let (ndc_x, ndc_y) = to_ndc(data, cx, cy);

    Some(SdfInstance {
        center: [(ndc_x + off_x) as f32, (ndc_y + off_y) as f32],
        size: [hw_ndc as f32, hh_ndc as f32],
        color: color_f32(data),
        shape_params: [
            1.0,
            render_mode_code(&data.render_mode),
            stroke_width_px(&data.render_mode),
            0.0,
        ],
    })
}

// ─── Line (geometry quad) ────────────────────────────────────────────────────

fn line_thickness(data: &ShapeData) -> f64 {
    match &data.render_mode {
        RenderMode::Stroke(w) => *w,
        _ => 1.0,
    }
}

fn build_line(data: &ShapeData, vertices: &mut Vec<LineVertex>, indices: &mut Vec<u32>) {
    let ShapeDesc::Line { from, to } = &data.desc else {
        return;
    };
    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let pivot_x = (from.0 + to.0) / 2.0;
    let pivot_y = (from.1 + to.1) / 2.0;

    let (ax, ay) = apply_transform(from.0, from.1, pivot_x, pivot_y, tx, ty, sx, sy);
    let (bx, by) = apply_transform(to.0, to.1, pivot_x, pivot_y, tx, ty, sx, sy);

    let thickness = line_thickness(data);
    let color = color_f32(data);

    emit_line_quad(data, ax, ay, bx, by, thickness, color, vertices, indices);
}

/// Emit a single line-segment quad (4 vertices, 6 indices).
/// Endpoints are in user coordinates; perpendicular is computed in user space
/// (which is pixel-isotropic), then vertices are converted to NDC.
fn emit_line_quad(
    data: &ShapeData,
    ax: f64, ay: f64,
    bx: f64, by: f64,
    thickness: f64,
    color: [f32; 4],
    vertices: &mut Vec<LineVertex>,
    indices: &mut Vec<u32>,
) {
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        return;
    }

    let half_t = thickness / 2.0;
    let aa_pad = 1.5;
    let expand = half_t + aa_pad;

    // Perpendicular unit vector in user/pixel space (isotropic)
    let px = -dy / len;
    let py = dx / len;

    // Four corners: offset endpoints by perpendicular * expand
    let corners = [
        (ax - px * expand, ay - py * expand), // v0: start, left edge
        (ax + px * expand, ay + py * expand), // v1: start, right edge
        (bx - px * expand, by - py * expand), // v2: end, left edge
        (bx + px * expand, by + py * expand), // v3: end, right edge
    ];
    let dists = [
        -(expand as f32),
        expand as f32,
        -(expand as f32),
        expand as f32,
    ];

    let base = vertices.len() as u32;
    for (i, &(cx, cy)) in corners.iter().enumerate() {
        let (nx, ny) = to_ndc(data, cx, cy);
        vertices.push(LineVertex {
            position: [nx as f32, ny as f32],
            color,
            edge_dist: dists[i],
            half_thickness: half_t as f32,
        });
    }

    indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}

// ─── Polygon (fill) ──────────────────────────────────────────────────────────

fn build_polygon_fill(
    data: &ShapeData,
    pts: &[(f64, f64)],
    vertices: &mut Vec<PolygonVertex>,
    indices: &mut Vec<u32>,
) {
    if pts.len() < 3 {
        return;
    }

    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let color = color_f32(data);

    let centroid_x: f64 = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let centroid_y: f64 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;

    let base = vertices.len() as u32;

    for &(px, py) in pts {
        let (rx, ry) = apply_transform(px, py, centroid_x, centroid_y, tx, ty, sx, sy);
        let (nx, ny) = to_ndc(data, rx, ry);
        vertices.push(PolygonVertex {
            position: [nx as f32, ny as f32],
            color,
        });
    }

    let n = pts.len() as u32;
    for i in 1..n - 1 {
        indices.push(base);
        indices.push(base + i);
        indices.push(base + i + 1);
    }
}

// ─── Polygon (outline) ──────────────────────────────────────────────────────

fn build_polygon_outline(
    data: &ShapeData,
    pts: &[(f64, f64)],
    line_vertices: &mut Vec<LineVertex>,
    line_indices: &mut Vec<u32>,
) {
    if pts.len() < 2 {
        return;
    }

    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let color = color_f32(data);
    let thickness = line_thickness(data);

    let centroid_x: f64 = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let centroid_y: f64 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;

    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let (ax, ay) = apply_transform(pts[i].0, pts[i].1, centroid_x, centroid_y, tx, ty, sx, sy);
        let (bx, by) = apply_transform(pts[j].0, pts[j].1, centroid_x, centroid_y, tx, ty, sx, sy);

        emit_line_quad(data, ax, ay, bx, by, thickness, color, line_vertices, line_indices);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rustle_lang::{CoordMeta, Origin};

    fn meta_800x600() -> CoordMeta {
        CoordMeta {
            px_width: 800.0,
            px_height: 600.0,
            origin: Origin::Center,
        }
    }

    #[test]
    fn circle_at_origin_ndc() {
        let data = ShapeData {
            desc: ShapeDesc::Circle {
                center: (0.0, 0.0),
                radius: 50.0,
            },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 0.0, 0.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let frame = prepare(&cmds);

        assert_eq!(frame.sdf_instances.len(), 1);
        let inst = &frame.sdf_instances[0];
        assert!((inst.center[0]).abs() < 1e-6);
        assert!((inst.center[1]).abs() < 1e-6);
        let expected_w = 2.0 * 50.0 / 800.0;
        let expected_h = 2.0 * 50.0 / 600.0;
        assert!((f64::from(inst.size[0]) - expected_w).abs() < 1e-5);
        assert!((f64::from(inst.size[1]) - expected_h).abs() < 1e-5);
    }

    #[test]
    fn polygon_four_points_fill() {
        let pts = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
        let data = ShapeData {
            desc: ShapeDesc::Polygon(pts),
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [0.0, 1.0, 0.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let frame = prepare(&cmds);

        assert_eq!(frame.polygon_vertices.len(), 4);
        assert_eq!(frame.polygon_indices.len(), 6);
        assert!(frame.line_vertices.is_empty());
    }

    #[test]
    fn polygon_outline_produces_line_geometry() {
        let pts = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let data = ShapeData {
            desc: ShapeDesc::Polygon(pts),
            render_mode: RenderMode::Outline,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let frame = prepare(&cmds);

        assert!(frame.polygon_vertices.is_empty());
        // 3 edges × 4 vertices = 12
        assert_eq!(frame.line_vertices.len(), 12);
        // 3 edges × 6 indices = 18
        assert_eq!(frame.line_indices.len(), 18);
    }

    #[test]
    fn line_produces_quad() {
        let data = ShapeData {
            desc: ShapeDesc::Line {
                from: (0.0, 0.0),
                to: (100.0, 0.0),
            },
            render_mode: RenderMode::Fill,
            coord_meta: meta_800x600(),
            transforms: Vec::new(),
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let cmds = vec![DrawCommand::DrawShape(data)];
        let frame = prepare(&cmds);

        assert_eq!(frame.line_vertices.len(), 4);
        assert_eq!(frame.line_indices.len(), 6);
        assert!(frame.sdf_instances.is_empty());
    }
}
