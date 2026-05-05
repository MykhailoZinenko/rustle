use crate::core::app_event_core::wrap_editor;
use crate::events::app_events::AppEvent;
use eframe::egui::{Button, Frame, Margin, RichText, ScrollArea, Stroke, Ui};

use crate::events::editor_events::EditorEvent;
use crate::state::editor_state::EditorState;
use crate::theme::ThemePalette;

pub fn draw_editor_tabs(
    ui: &mut Ui,
    editor: &EditorState,
    theme: &ThemePalette,
    events: &mut Vec<AppEvent>,
) {
    Frame::new()
        .fill(theme.tab_inactive_bg)
        .stroke(Stroke::new(1.0, theme.surface_stroke))
        .inner_margin(Margin::same(4))
        .show(ui, |ui| {
            ScrollArea::horizontal()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (index, tab) in editor.tabs.iter().enumerate() {
                            let is_active = editor.active_index() == Some(index);

                            Frame::new()
                                .fill(if is_active {
                                    theme.tab_active_bg
                                } else {
                                    theme.tab_inactive_bg
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    if is_active {
                                        theme.active_tab_stroke
                                    } else {
                                        theme.surface_stroke
                                    },
                                ))
                                .inner_margin(Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let title_button =
                                            Button::new(RichText::new(tab.title()).monospace())
                                                .frame(false)
                                                .selected(is_active);
                                        if ui.add(title_button).clicked() {
                                            events.push(wrap_editor(EditorEvent::SetActiveTab(index)));
                                        }

                                        let close_button = Button::new(RichText::new("x").monospace())
                                            .frame(false)
                                            .small();
                                        if ui.add(close_button).clicked() {
                                            events.push(wrap_editor(EditorEvent::CloseTab(index)));
                                        }
                                    });
                                });
                        }
                    });
                });
        });
}
