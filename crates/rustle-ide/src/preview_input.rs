//! Map egui pointer coordinates into Rustle script space for the preview canvas.

use eframe::egui;
use rustle_lang::{CoordMeta, DrawCommand, Origin};

/// Last frame’s preview canvas geometry (native script resolution vs fitted rect).
#[derive(Clone, Copy)]
pub struct PreviewCanvasInfo {
    pub rect: egui::Rect,
    pub native: egui::Vec2,
    pub fitted: egui::Vec2,
}

pub fn first_coord_meta(commands: &[DrawCommand]) -> Option<CoordMeta> {
    commands.iter().find_map(|c| match c {
        DrawCommand::DrawShape(data) => Some(data.coord_meta.clone()),
        _ => None,
    })
}

/// Inverse of [`CoordMeta::x_to_screen_px`] / [`CoordMeta::y_to_screen_px`] for canvas-local pixels.
fn user_from_screen_px(meta: &CoordMeta, sx: f64, sy: f64) -> (f64, f64) {
    if meta.px_width <= 0.0 || meta.px_height <= 0.0 {
        return (sx, sy);
    }
    let ux = match meta.origin {
        Origin::Center | Origin::Top | Origin::Bottom => sx - meta.px_width / 2.0,
        Origin::TopLeft | Origin::BottomLeft | Origin::Left => sx,
        Origin::TopRight | Origin::BottomRight | Origin::Right => meta.px_width - sx,
    };
    let uy = match meta.origin {
        Origin::TopLeft | Origin::TopRight | Origin::Top => sy,
        Origin::Center | Origin::Left | Origin::Right => meta.px_height / 2.0 - sy,
        Origin::BottomLeft | Origin::BottomRight | Origin::Bottom => meta.px_height - sy,
    };
    (ux, uy)
}

/// Convert global pointer position into script coordinates, or pass through if layout is unknown.
pub fn map_screen_to_script(
    canvas: Option<&PreviewCanvasInfo>,
    screen_mx: f64,
    screen_my: f64,
    meta: Option<&CoordMeta>,
) -> (f64, f64) {
    let Some(c) = canvas else {
        return (screen_mx, screen_my);
    };
    let lx = screen_mx - f64::from(c.rect.min.x);
    let ly = screen_my - f64::from(c.rect.min.y);
    if c.fitted.x <= 0.0 || c.fitted.y <= 0.0 {
        return (lx, ly);
    }
    let scale_x = f64::from(c.native.x / c.fitted.x);
    let scale_y = f64::from(c.native.y / c.fitted.y);
    let px = lx * scale_x;
    let py = ly * scale_y;
    let Some(m) = meta else {
        return (px, py);
    };
    user_from_screen_px(m, px, py)
}
