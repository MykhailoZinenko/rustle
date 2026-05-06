use eframe::egui;
use rfd::FileDialog;

use crate::app_core::AppCore;
use crate::core::app_event_core::wrap_editor;
use crate::core::shortcut_core::{ShortcutAction, consume_shortcut};
use crate::events::app_events::AppEvent;
use crate::events::editor_events::EditorEvent;
use crate::theme::visuals_for;

pub struct App {
    core: AppCore,
}

impl Default for App {
    fn default() -> Self {
        Self {
            core: AppCore::default(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().set_visuals(visuals_for(self.core.state.theme_mode));
        self.core.tick_preview(ui.ctx());
        let shortcut_action = consume_shortcut(ui.ctx());

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let is_running = self.core.is_running();
            let editor = &self.core.state.editor;
            let theme = self.core.state.theme;
            let events = &mut self.core.events;
            crate::ui::top_bar::draw_top_bar(
                ui,
                editor,
                is_running,
                self.core.state.console_visible,
                self.core.state.theme_mode,
                &theme,
                events,
            );

            ui.separator();
            crate::ui::workspace_ui::draw_workspace(
                ui,
                &mut self.core,
                &theme,
            );
        });

        self.handle_shortcut_action(shortcut_action);
        self.core.drain_events();
        self.core.sync_preview_state();
    }
}

impl App {
    fn handle_shortcut_action(&mut self, action: Option<ShortcutAction>) {
        match action {
            Some(ShortcutAction::Save) => {
                let save_path = self
                    .core
                    .editor()
                    .active_index()
                    .and_then(|index| self.core.editor().tabs[index].file_path().map(|p| p.to_path_buf()))
                    .or_else(|| FileDialog::new().save_file());
                self.core.queue_event(wrap_editor(EditorEvent::SaveActiveFile(save_path)));
            }
            Some(ShortcutAction::NewFile) => self.core.queue_event(wrap_editor(EditorEvent::NewTab)),
            Some(ShortcutAction::OpenFile) => {
                if let Some(path) = FileDialog::new().pick_file() {
                    self.core.queue_event(wrap_editor(EditorEvent::OpenFile(path)));
                }
            }
            Some(ShortcutAction::CloseTab) => {
                if let Some(index) = self.core.editor().active_index() {
                    self.core.queue_event(wrap_editor(EditorEvent::CloseTab(index)));
                }
            }
            Some(ShortcutAction::ToggleRun) => {
                if self.core.is_running() {
                    self.core.queue_event(AppEvent::StopPreview);
                } else {
                    self.core.queue_event(AppEvent::StartPreview);
                }
            }
            Some(ShortcutAction::ToggleSuggestions) => {
                self.core.queue_event(AppEvent::ToggleSuggestions);
            }
            None => {}
        }
    }
}
