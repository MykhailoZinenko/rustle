use eframe::egui::{self, Frame, Margin, RichText, Stroke, Ui};
use rustle_lang::DrawCommand;

use crate::events::app_events::AppEvent;
use crate::renderer::egui_renderer::EguiPreviewRenderer;
use crate::theme::ThemePalette;

pub struct PreviewPanelState<'a> {
    pub has_active_tab: bool,
    pub is_running: bool,
    pub commands: &'a [DrawCommand],
    pub error: Option<&'a str>,
    pub native_size: Option<egui::Vec2>,
}

pub fn draw_preview(
    ui: &mut Ui,
    state: PreviewPanelState<'_>,
    renderer: &EguiPreviewRenderer,
    theme: &ThemePalette,
 ) -> Option<AppEvent> {
    let mut event = None;

    Frame::new()
        .fill(theme.editor_bg)
        .stroke(Stroke::new(1.0, theme.surface_stroke))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            let header_body_reserved_height = 36.0;
            let fitted_size = state.native_size.map(|native_size| {
                EguiPreviewRenderer::fit_size(
                    native_size,
                    egui::vec2(
                        ui.available_width(),
                        (ui.available_height() - header_body_reserved_height).max(0.0),
                    ),
                )
            });

            ui.horizontal(|ui| {
                ui.label(RichText::new("Preview").strong());
                if let (Some(native_size), Some(fitted_size)) = (state.native_size, fitted_size) {
                    ui.label(
                        RichText::new(format!(
                            "{}x{} -> {}x{}",
                            native_size.x.round() as i32,
                            native_size.y.round() as i32,
                            fitted_size.x.round() as i32,
                            fitted_size.y.round() as i32
                        ))
                        .color(theme.muted_text),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(state.is_running, egui::Button::new("Stop"))
                        .clicked()
                    {
                        event = Some(AppEvent::StopPreview);
                    }
                    if ui
                        .add_enabled(!state.is_running, egui::Button::new("Run"))
                        .clicked()
                    {
                        event = Some(AppEvent::StartPreview);
                    }
                });
            });
            ui.separator();

            if !state.has_active_tab {
                ui.label(RichText::new("No file opened").color(theme.muted_text));
                return;
            }

            if let Some(error) = state.error {
                ui.label(RichText::new(error).color(theme.error_text));
                return;
            }

            if state.commands.is_empty() {
                let message = if state.is_running {
                    "No draw commands this frame"
                } else {
                    "No draw commands"
                };
                ui.label(RichText::new(message).color(theme.muted_text));
                return;
            }

            renderer.draw(ui, state.commands, fitted_size, theme);
        });

    event
}
