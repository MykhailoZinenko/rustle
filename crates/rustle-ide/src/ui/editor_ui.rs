use eframe::egui::{Color32, RichText, TextStyle, Ui};

use crate::events::app_events::AppEvent;
use crate::state::app_state::AppState;
use crate::theme::ThemePalette;
use crate::ui::editor_code_view_ui::{draw_editor_code_view, line_count};
use crate::ui::editor_status_bar_ui::{EditorStatusSnapshot, draw_editor_status_bar};
use crate::ui::editor_tabs_ui::draw_editor_tabs;

pub fn draw_editor(
    ui: &mut Ui,
    app_state: &mut AppState,
    theme: &ThemePalette,
    events: &mut Vec<AppEvent>,
) {
    let editor = &mut app_state.editor;
    if editor.tabs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No file opened").color(Color32::GRAY));
        });
        return;
    }

    draw_editor_tabs(ui, editor, theme, events);
    ui.add_space(2.0);

    if let Some(active_index) = editor.active_index() {
        let row_height = ui.text_style_height(&TextStyle::Monospace);
        let line_count = line_count(&editor.tabs[active_index].buffer);
        let status_snapshot = EditorStatusSnapshot::from_tab(&editor.tabs[active_index]);
        let body_rect = ui.available_rect_before_wrap();
        let status_height = (ui.text_style_height(&TextStyle::Body) + 10.0).max(24.0);
        let code_rect = eframe::egui::Rect::from_min_max(
            body_rect.min,
            eframe::egui::pos2(body_rect.right(), (body_rect.bottom() - status_height).max(body_rect.top())),
        );
        let status_rect = eframe::egui::Rect::from_min_max(
            eframe::egui::pos2(body_rect.left(), code_rect.bottom()),
            body_rect.max,
        );

        ui.scope_builder(
            eframe::egui::UiBuilder::new()
                .max_rect(code_rect)
                .layout(eframe::egui::Layout::top_down(eframe::egui::Align::Min)),
            |ui| {
                draw_editor_code_view(
                    ui,
                    editor,
                    &mut app_state.suggestions,
                    &mut app_state.pending_editor_command,
                    theme,
                    active_index,
                    row_height,
                    line_count,
                );
            },
        );

        ui.scope_builder(
            eframe::egui::UiBuilder::new()
                .max_rect(status_rect)
                .layout(eframe::egui::Layout::top_down(eframe::egui::Align::Min)),
            |ui| {
                draw_editor_status_bar(ui, theme, &status_snapshot, line_count);
            },
        );

        ui.allocate_rect(body_rect, eframe::egui::Sense::hover());
    }
}
