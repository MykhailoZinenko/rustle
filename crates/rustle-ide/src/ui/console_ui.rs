use std::collections::VecDeque;

use eframe::egui::{Button, Color32, Frame, Layout, Margin, RichText, ScrollArea, Stroke, Ui};

use crate::events::app_events::AppEvent;
use crate::runner::{ConsoleEntry, ConsoleLevel};
use crate::theme::ThemePalette;

pub fn draw_console(
    ui: &mut Ui,
    console: &VecDeque<ConsoleEntry>,
    theme: &ThemePalette,
    events: &mut Vec<AppEvent>,
) {
    Frame::new()
        .fill(theme.console_bg)
        .stroke(Stroke::new(1.0, theme.surface_stroke))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Console").strong());
                if !console.is_empty() {
                    ui.label(RichText::new(format!("({})", console.len())).color(theme.muted_text));
                }
                ui.with_layout(Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                    if ui.add(Button::new(RichText::new("X").strong())).clicked() {
                        events.push(AppEvent::ToggleConsole);
                    }
                });
            });
            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if console.is_empty() {
                        ui.label(RichText::new("No console output").color(theme.muted_text));
                        return;
                    }

                    for entry in console {
                        let (prefix, color) = match entry.level {
                            ConsoleLevel::Log => ("", ui.visuals().text_color()),
                            ConsoleLevel::Warn => ("[warn]  ", Color32::from_rgb(220, 180, 60)),
                            ConsoleLevel::Error => ("[error] ", theme.error_text),
                        };
                        ui.label(
                            RichText::new(format!("{}{}", prefix, entry.message))
                                .monospace()
                                .color(color),
                        );
                    }
                });
        });
}
