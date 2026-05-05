use eframe::egui::{self, Button, Color32, Frame, Margin, RichText, Stroke};

use crate::core::suggestion_core::SuggestionContext;
use crate::theme::ThemePalette;

pub fn draw_suggestions_popup(
    ctx: &egui::Context,
    id: egui::Id,
    anchor: egui::Pos2,
    suggestions: &SuggestionContext,
    selected_index: usize,
    theme: &ThemePalette,
) -> Option<usize> {
    let mut clicked_index = None;

    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            Frame::new()
                .fill(theme.editor_bg)
                .stroke(Stroke::new(1.0, theme.surface_stroke))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_min_width(260.0);

                    if suggestions.items.is_empty() {
                        ui.label(RichText::new("No suggestions").color(theme.muted_text));
                        return;
                    }

                    for (index, item) in suggestions.items.iter().enumerate() {
                        let label = format!("{:<22} {}", item.label, item.detail);
                        let is_selected = index == selected_index;
                        let response = ui.add_sized(
                            [ui.available_width(), 24.0],
                            Button::new(
                                RichText::new(label)
                                    .monospace()
                                    .color(theme.syntax_identifier),
                            )
                            .fill(if is_selected {
                                Color32::from_rgb(50, 72, 110)
                            } else {
                                Color32::from_rgb(37, 41, 51)
                            })
                            .stroke(if is_selected {
                                Stroke::new(1.0, Color32::from_rgb(78, 136, 224))
                            } else {
                                Stroke::NONE
                            }),
                        );

                        if response.clicked() {
                            clicked_index = Some(index);
                        }
                    }
                });
        });

    clicked_index
}
