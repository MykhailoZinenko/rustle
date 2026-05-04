use eframe::egui::{Color32, RichText, TextStyle, Ui};

use crate::events::app_events::AppEvent;
use crate::state::app_state::AppState;
use crate::ui::editor_code_view_ui::{draw_editor_code_view, line_count};
use crate::ui::editor_status_bar_ui::{EditorStatusSnapshot, draw_editor_status_bar};
use crate::ui::editor_tabs_ui::draw_editor_tabs;

pub fn draw_editor(ui: &mut Ui, app_state: &mut AppState, events: &mut Vec<AppEvent>) {
    let editor = &mut app_state.editor;
    if editor.tabs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No file opened").color(Color32::GRAY));
        });
        return;
    }

    draw_editor_tabs(ui, editor, events);
    ui.add_space(2.0);

    if let Some(active_index) = editor.active_index() {
        let row_height = ui.text_style_height(&TextStyle::Monospace);
        let line_count = line_count(&editor.tabs[active_index].buffer);
        let status_snapshot = EditorStatusSnapshot::from_tab(&editor.tabs[active_index]);

        draw_editor_code_view(
            ui,
            editor,
            &mut app_state.suggestions,
            active_index,
            row_height,
            line_count,
        );
        draw_editor_status_bar(ui, &status_snapshot, line_count);
    }
}
