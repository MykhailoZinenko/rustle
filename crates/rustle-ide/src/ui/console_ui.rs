use std::collections::VecDeque;

use eframe::egui::{Button, Color32, Frame, Margin, RichText, ScrollArea, Stroke, Ui};

use crate::runner::{ConsoleEntry, ConsoleLevel};
use crate::theme::{CONSOLE_BG, SURFACE_STROKE};

pub fn draw_console(ui: &mut Ui, console: &VecDeque<ConsoleEntry>) -> bool {
    let mut close_requested = false;

    Frame::new()
        .fill(CONSOLE_BG)
        .stroke(Stroke::new(1.0, SURFACE_STROKE))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(Button::new(RichText::new("x").strong())).clicked() {
                    close_requested = true;
                }
                ui.label(RichText::new("Console").strong());
                if !console.is_empty() {
                    ui.label(RichText::new(format!("({})", console.len())).color(Color32::GRAY));
                }
            });
            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if console.is_empty() {
                        ui.label(RichText::new("No console output").color(Color32::GRAY));
                        return;
                    }

                    for entry in console {
                        let (prefix, color) = match entry.level {
                            ConsoleLevel::Log => ("", Color32::from_rgb(210, 210, 210)),
                            ConsoleLevel::Warn => ("[warn]  ", Color32::from_rgb(220, 180, 60)),
                            ConsoleLevel::Error => ("[error] ", Color32::from_rgb(220, 80, 80)),
                        };
                        ui.label(
                            RichText::new(format!("{}{}", prefix, entry.message))
                                .monospace()
                                .color(color),
                        );
                    }
                });
        });

    close_requested
}
