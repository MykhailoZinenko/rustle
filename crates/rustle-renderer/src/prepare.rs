use rustle_lang::{DrawCommand, RenderMode, ShapeData, ShapeDesc, origin_offset};

use crate::instance::{PolygonVertex, SdfInstance};

/// GPU-ready data for one frame.
pub struct PreparedFrame {
    pub sdf_instances: Vec<SdfInstance>,
    pub polygon_vertices: Vec<PolygonVertex>,
    pub polygon_indices: Vec<u32>,
}

/// Convert a slice of `DrawCommand`s into GPU instance/vertex data.
#[must_use]
pub fn prepare(commands: &[DrawCommand]) -> PreparedFrame {
    let mut sdf_instances = Vec::new();
    let mut polygon_vertices = Vec::new();
    let mut polygon_indices = Vec::new();

    for cmd in commands {
        let DrawCommand::DrawShape(data) = cmd else {
            continue;
        };
        match &data.desc {
            ShapeDesc::Circle { .. } | ShapeDesc::Rect { .. } | ShapeDesc::Line { .. } => {
                if let Some(inst) = build_sdf_instance(data) {
                    sdf_instances.push(inst);
                }
            }
            ShapeDesc::Polygon(pts) => {
                build_polygon(data, pts, &mut polygon_vertices, &mut polygon_indices);
            }
            _ => {}
        }
    }

    PreparedFrame {
        sdf_instances,
        polygon_vertices,
        polygon_indices,
    }
}

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

fn stroke_width(mode: &RenderMode) -> f32 {
    match mode {
        RenderMode::Stroke(w) => *w as f32,
        _ => 0.0,
    }
}

fn color_f32(data: &ShapeData) -> [f32; 4] {
    [
        data.color[0] as f32,
        data.color[1] as f32,
        data.color[2] as f32,
        data.color[3] as f32,
    ]
}

/// Accumulate translation (NDC) and scale from transforms.
fn accumulate_transforms(data: &ShapeData) -> (f64, f64, f64, f64) {
    let meta = &data.coord_meta;
    let mut tx = 0.0;
    let mut ty = 0.0;
    let mut sx = 1.0;
    let mut sy = 1.0;
    for t in &data.transforms {
        tx += meta.w_to_ndc(t.tx);
        ty += meta.h_to_ndc(t.ty);
        sx *= t.sx;
        sy *= t.sy;
    }
    (tx, ty, sx, sy)
}

fn build_sdf_instance(data: &ShapeData) -> Option<SdfInstance> {
    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);

    match &data.desc {
        ShapeDesc::Circle { center, radius } => {
            let cx = meta.x_to_ndc(center.0) + tx;
            let cy = meta.y_to_ndc(center.1) + ty;
            let r_ndc_x = meta.w_to_ndc(*radius) * sx;
            let r_ndc_y = meta.h_to_ndc(*radius) * sy;

            Some(SdfInstance {
                center: [cx as f32, cy as f32],
                size: [r_ndc_x as f32, r_ndc_y as f32],
                color: color_f32(data),
                shape_params: [
                    0.0, // circle
                    render_mode_code(&data.render_mode),
                    stroke_width(&data.render_mode),
                    0.0, // corner_radius
                ],
                line_dir: [0.0; 4],
            })
        }
        ShapeDesc::Rect { center, size, origin } => {
            let hw_ndc = meta.w_to_ndc(size.0 / 2.0);
            let hh_ndc = meta.h_to_ndc(size.1 / 2.0);
            let (off_x, off_y) = origin_offset(origin, hw_ndc, hh_ndc);

            let cx = meta.x_to_ndc(center.0) + off_x + tx;
            let cy = meta.y_to_ndc(center.1) + off_y + ty;
            let w_ndc = hw_ndc * sx;
            let h_ndc = hh_ndc * sy;

            Some(SdfInstance {
                center: [cx as f32, cy as f32],
                size: [w_ndc as f32, h_ndc as f32],
                color: color_f32(data),
                shape_params: [
                    1.0, // rect
                    render_mode_code(&data.render_mode),
                    stroke_width(&data.render_mode),
                    0.0, // corner_radius
                ],
                line_dir: [0.0; 4],
            })
        }
        ShapeDesc::Line { from, to } => {
            let x0 = meta.x_to_ndc(from.0);
            let y0 = meta.y_to_ndc(from.1);
            let x1 = meta.x_to_ndc(to.0);
            let y1 = meta.y_to_ndc(to.1);

            let mx = f64::midpoint(x0, x1) + tx;
            let my = f64::midpoint(y0, y1) + ty;
            let dx = (x1 - x0) * sx;
            let dy = (y1 - y0) * sy;
            let half_len = (dx * dx + dy * dy).sqrt() / 2.0;
            // Line thickness: use stroke width or a default thin line
            let thickness = if let RenderMode::Stroke(w) = &data.render_mode {
                meta.h_to_ndc(*w) * sy
            } else {
                meta.h_to_ndc(1.0) * sy
            };

            Some(SdfInstance {
                center: [mx as f32, my as f32],
                size: [half_len as f32, thickness as f32],
                color: color_f32(data),
                shape_params: [
                    2.0, // line
                    render_mode_code(&data.render_mode),
                    stroke_width(&data.render_mode),
                    0.0,
                ],
                line_dir: [dx as f32, dy as f32, 0.0, 0.0],
            })
        }
        _ => None,
    }
}

fn build_polygon(
    data: &ShapeData,
    pts: &[(f64, f64)],
    vertices: &mut Vec<PolygonVertex>,
    indices: &mut Vec<u32>,
) {
    if pts.len() < 3 {
        return;
    }

    let meta = &data.coord_meta;
    let (tx, ty, sx, sy) = accumulate_transforms(data);
    let color = color_f32(data);

    // Compute centroid for scale pivot
    let centroid_x: f64 = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let centroid_y: f64 = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;
    let cx_ndc = meta.x_to_ndc(centroid_x);
    let cy_ndc = meta.y_to_ndc(centroid_y);

    let base = vertices.len() as u32;

    for &(px, py) in pts {
        let mut nx = meta.x_to_ndc(px);
        let mut ny = meta.y_to_ndc(py);
        // Apply scale around centroid
        nx = cx_ndc + (nx - cx_ndc) * sx;
        ny = cy_ndc + (ny - cy_ndc) * sy;
        // Apply translation
        nx += tx;
        ny += ty;

        vertices.push(PolygonVertex {
            position: [nx as f32, ny as f32],
            color,
        });
    }

    // Fan triangulation from first vertex
    let n = pts.len() as u32;
    for i in 1..n - 1 {
        indices.push(base);
        indices.push(base + i);
        indices.push(base + i + 1);
    }
}

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
        // Center at origin → NDC (0, 0)
        assert!((inst.center[0]).abs() < 1e-6);
        assert!((inst.center[1]).abs() < 1e-6);
        // radius 50 in 800px wide → w_to_ndc = 2*50/800 = 0.125
        let expected_w = 2.0 * 50.0 / 800.0;
        let expected_h = 2.0 * 50.0 / 600.0;
        assert!((f64::from(inst.size[0]) - expected_w).abs() < 1e-5);
        assert!((f64::from(inst.size[1]) - expected_h).abs() < 1e-5);
    }

    #[test]
    fn polygon_four_points() {
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
        // Fan triangulation: 4 pts → 2 triangles → 6 indices
        assert_eq!(frame.polygon_indices.len(), 6);
    }
}
