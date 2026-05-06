use eframe::egui;
use egui::{Color32, RichText};

use rfd::FileDialog;

use crate::core::app_event_core::wrap_editor;
use crate::events::app_events::AppEvent;
use crate::events::editor_events::EditorEvent;
use crate::state::editor_state::EditorState;
use crate::theme::{ThemeMode, ThemePalette};

pub fn draw_top_bar(
    ui: &mut egui::Ui,
    editor: &EditorState,
    _is_running: bool,
    console_visible: bool,
    theme_mode: ThemeMode,
    _theme: &ThemePalette,
    events: &mut Vec<AppEvent>,
) {

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("MiniIDE")
                .strong()
                .color(Color32::LIGHT_BLUE),
        );

        ui.separator();

        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New File    Ctrl+N").clicked() {
                    events.push(wrap_editor(EditorEvent::NewTab));
                    ui.close();
                }

                if ui.button("Open File   Ctrl+O").clicked() {
                    if let Some(path) = FileDialog::new().pick_file() {
                        events.push(wrap_editor(EditorEvent::OpenFile(path)));
                    }
                    ui.close();
                }

                if ui.button("Save        Ctrl+S").clicked() {
                    let save_path = editor
                        .active_index()
                        .and_then(|index| editor.tabs[index].file_path().map(|p| p.to_path_buf()))
                        .or_else(|| FileDialog::new().save_file());
                    events.push(wrap_editor(EditorEvent::SaveActiveFile(save_path)));
                    ui.close();
                }

                if ui
                    .add_enabled(editor.active_index().is_some(), egui::Button::new("Close Tab   Ctrl+W"))
                    .clicked()
                {
                    if let Some(index) = editor.active_index() {
                        events.push(wrap_editor(EditorEvent::CloseTab(index)));
                    }
                    ui.close();
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui.button("Undo        Ctrl+Z").clicked() {
                    events.push(AppEvent::Undo);
                    ui.close();
                }
                if ui.button("Redo        Ctrl+Y").clicked() {
                    events.push(AppEvent::Redo);
                    ui.close();
                }
                if ui.button("Toggle Suggestions   Ctrl+Space").clicked() {
                    events.push(AppEvent::ToggleSuggestions);
                    ui.close();
                }
            });

            ui.menu_button("Theme", |ui| {
                for mode in [ThemeMode::Dark, ThemeMode::Light] {
                    if ui.selectable_label(theme_mode == mode, mode.label()).clicked() {
                        events.push(AppEvent::SetTheme(mode));
                        ui.close();
                    }
                }
            });

            ui.menu_button("Run", |ui| {
                if ui
                    .button("Run in Terminal   F5")
                    .clicked()
                {
                    events.push(AppEvent::RunInTerminal);
                    ui.close();
                }
            });

            ui.menu_button("Terminal", |ui| {
                if ui
                    .button(if console_visible {
                        "Hide Console"
                    } else {
                        "Show Console"
                    })
                    .clicked()
                {
                    events.push(AppEvent::ToggleConsole);
                    ui.close();
                }
            });
        });
    });
}
