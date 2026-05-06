use eframe::egui;
use crate::terminal::TerminalView;
use crate::app_core::AppCore;
use crate::theme::ThemePalette;

pub fn draw_terminal(
    ui: &mut egui::Ui,
    core: &mut AppCore,
    _theme: &ThemePalette,
) {
    if let Some(terminal) = core.terminal.as_mut() {
        let view = TerminalView::new(ui, terminal);
        ui.add(view);
    } else {
        ui.centered_and_justified(|ui| {
            ui.label("Terminal not available");
        });
    }
}
