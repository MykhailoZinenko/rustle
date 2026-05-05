use eframe::egui::{self, Color32, Ui};
use rustle_lang::{DrawCommand, Origin, RenderMode, ShapeData, ShapeDesc};
use crate::theme::ThemePalette;

pub struct EguiPreviewRenderer;

impl EguiPreviewRenderer {
    pub fn draw(
        &self,
        ui: &mut Ui,
        commands: &[DrawCommand],
        fitted_size: Option<egui::Vec2>,
        theme: &ThemePalette,
    ) {
        let Some(native_size) = preview_native_size(commands) else {
            ui.label(egui::RichText::new("No shapes to render").color(theme.muted_text));
            return;
        };

        let canvas_size = fitted_size.unwrap_or(native_size);
        let (outer_rect, _) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
        let rect = egui::Rect::from_center_size(outer_rect.center(), canvas_size);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme.preview_bg);

        let scale_x = if native_size.x > 0.0 { rect.width() / native_size.x } else { 1.0 };
        let scale_y = if native_size.y > 0.0 { rect.height() / native_size.y } else { 1.0 };

        for command in commands {
            let DrawCommand::DrawShape(data) = command else {
                continue;
            };
            let points = tessellate_screen_px(data)
                .into_iter()
                .map(|(x, y)| {
                    egui::pos2(
                        rect.min.x + x as f32 * scale_x,
                        rect.min.y + y as f32 * scale_y,
                    )
                })
                .collect::<Vec<_>>();

            let fill_color = Color32::from_rgba_unmultiplied(180, 160, 255, 200);
            let stroke_color = Color32::from_rgba_unmultiplied(200, 180, 255, 255);
            let stroke_width = match &data.render_mode {
                RenderMode::Stroke(w) => *w as f32,
                _ => 1.5,
            };
            let stroke = egui::Stroke::new(stroke_width, stroke_color);

            if matches!(&data.desc, ShapeDesc::Line { .. }) {
                if points.len() >= 2 {
                    painter.line_segment([points[0], points[1]], stroke);
                }
            } else {
                match &data.render_mode {
                    RenderMode::Fill | RenderMode::Sdf => {
                        painter.add(egui::Shape::convex_polygon(points, fill_color, egui::Stroke::NONE));
                    }
                    RenderMode::Outline | RenderMode::Stroke(_) => {
                        painter.add(egui::Shape::closed_line(points, stroke));
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn fit_size(native_size: egui::Vec2, available_size: egui::Vec2) -> egui::Vec2 {
        if native_size.x <= 0.0 || native_size.y <= 0.0 {
            return native_size;
        }

        let width_scale = if available_size.x > 0.0 {
            available_size.x / native_size.x
        } else {
            1.0
        };
        let height_scale = if available_size.y > 0.0 {
            available_size.y / native_size.y
        } else {
            1.0
        };
        let scale = width_scale.min(height_scale).min(1.0);
        egui::vec2(native_size.x * scale, native_size.y * scale)
    }
}

fn preview_native_size(commands: &[DrawCommand]) -> Option<egui::Vec2> {
    commands.iter().find_map(|command| match command {
        DrawCommand::DrawShape(data) => Some(egui::vec2(
            data.coord_meta.px_width.max(400.0) as f32,
            data.coord_meta.px_height.max(400.0) as f32,
        )),
        _ => None,
    })
}

fn tessellate_screen_px(data: &ShapeData) -> Vec<(f64, f64)> {
    let m = &data.coord_meta;
    let sx = |x: f64| m.x_to_screen_px(x);
    let sy = |y: f64| m.y_to_screen_px(y);

    let verts = match &data.desc {
        ShapeDesc::Circle { center, radius } => (0..64usize)
            .map(|i| {
                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                (sx(center.0 + radius * t.cos()), sy(center.1 + radius * t.sin()))
            })
            .collect(),
        ShapeDesc::Rect { center, size, origin } => {
            let (w, h) = (size.0, size.1);
            let (ax, ay) = (sx(center.0), sy(center.1));
            let (min_x, max_x) = match origin {
                Origin::TopLeft | Origin::BottomLeft | Origin::Left => (ax, ax + w),
                Origin::TopRight | Origin::BottomRight | Origin::Right => (ax - w, ax),
                Origin::Center | Origin::Top | Origin::Bottom => (ax - w / 2.0, ax + w / 2.0),
            };
            let (min_y, max_y) = match origin {
                Origin::TopLeft | Origin::TopRight | Origin::Top => (ay, ay + h),
                Origin::BottomLeft | Origin::BottomRight | Origin::Bottom => (ay - h, ay),
                Origin::Center | Origin::Left | Origin::Right => (ay - h / 2.0, ay + h / 2.0),
            };
            vec![(min_x, min_y), (max_x, min_y), (max_x, max_y), (min_x, max_y)]
        }
        ShapeDesc::Line { from, to } => vec![(sx(from.0), sy(from.1)), (sx(to.0), sy(to.1))],
        ShapeDesc::Polygon(points) => points.iter().map(|(x, y)| (sx(*x), sy(*y))).collect(),
        _ => vec![],
    };

    let x_sign: f64 = match m.origin {
        Origin::TopRight | Origin::BottomRight | Origin::Right => -1.0,
        _ => 1.0,
    };
    let y_sign: f64 = if m.origin.is_y_down() { 1.0 } else { -1.0 };

    let mut result = verts;
    for td in &data.transforms {
        let tx_px = td.tx * x_sign;
        let ty_px = td.ty * y_sign;
        let n = result.len() as f64;
        let (sum_x, sum_y) = result.iter().fold((0.0, 0.0), |(ax, ay), (x, y)| (ax + x, ay + y));
        let (pivot_x, pivot_y) = (sum_x / n, sum_y / n);
        let a = -td.angle;
        let (cos_a, sin_a) = (a.cos(), a.sin());
        result = result
            .into_iter()
            .map(|(x, y)| {
                let dx = (x - pivot_x) * td.sx;
                let dy = (y - pivot_y) * td.sy;
                let rx = dx * cos_a - dy * sin_a;
                let ry = dx * sin_a + dy * cos_a;
                (pivot_x + rx + tx_px, pivot_y + ry + ty_px)
            })
            .collect();
    }
    result
}
