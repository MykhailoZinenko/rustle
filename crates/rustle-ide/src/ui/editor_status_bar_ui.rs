use std::path::PathBuf;

use eframe::egui::{Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui};

use crate::state::editor_state::Cursor;
use crate::theme::ThemePalette;

pub struct EditorStatusSnapshot {
    pub file_path: Option<PathBuf>,
    pub file_name: String,
    pub cursor: Cursor,
    pub is_dirty: bool,
}

pub fn draw_editor_status_bar(
    ui: &mut Ui,
    theme: &ThemePalette,
    tab: &EditorStatusSnapshot,
    line_count: usize,
) {
    let left = if let Some(path) = &tab.file_path {
        path.display().to_string()
    } else {
        tab.file_name.clone()
    };
    let dirty = if tab.is_dirty { "  Edited" } else { "" };
    let right = format!(
        "Ln {}, Col {}  |  {} lines",
        tab.cursor.line + 1,
        tab.cursor.column + 1,
        line_count
    );

    Frame::new()
        .fill(theme.status_bg)
        .stroke(Stroke::new(1.0, theme.status_bg))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{left}{dirty}"))
                        .monospace()
                        .strong()
                        .color(Color32::WHITE),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(right).monospace().color(Color32::WHITE));
                });
            });
        });
}
