use crate::core::app_event_core::wrap_editor;
use crate::events::app_events::AppEvent;
use eframe::egui::{Button, Frame, Margin, RichText, ScrollArea, Stroke, Ui};

use crate::events::editor_events::EditorEvent;
use crate::state::editor_state::EditorState;
use crate::theme::{
    ACTIVE_TAB_STROKE, SURFACE_STROKE, TAB_ACTIVE_BG, TAB_INACTIVE_BG,
};

pub fn draw_editor_tabs(ui: &mut Ui, editor: &EditorState, events: &mut Vec<AppEvent>) {
    Frame::new()
        .fill(TAB_INACTIVE_BG)
        .stroke(Stroke::new(1.0, SURFACE_STROKE))
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
                                    TAB_ACTIVE_BG
                                } else {
                                    TAB_INACTIVE_BG
                                })
                                .stroke(Stroke::new(
                                    1.0,
                                    if is_active {
                                        ACTIVE_TAB_STROKE
                                    } else {
                                        SURFACE_STROKE
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
