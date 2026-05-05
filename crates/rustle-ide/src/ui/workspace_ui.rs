use eframe::egui;

use crate::app_core::AppCore;
use crate::events::app_events::AppEvent;
use crate::renderer::egui_renderer::EguiPreviewRenderer;
use crate::theme::ThemePalette;
use crate::ui::console_ui::draw_console;
use crate::ui::preview_ui::PreviewPanelState;

pub fn draw_workspace(
    ui: &mut egui::Ui,
    core: &mut AppCore,
    renderer: &EguiPreviewRenderer,
    theme: &ThemePalette,
) {
    let mut preview_event = None;
    let available_rect = ui.available_rect_before_wrap();
    let console_visible = core.state.console_visible;

    let (top_rect, bottom_rect) = if console_visible {
        let horizontal_splitter_height = 6.0;
        let min_top_height = 220.0;
        let min_console_height = 120.0;
        let total_height = available_rect.height().max(1.0);
        let content_height =
            (total_height - horizontal_splitter_height).max(min_top_height + min_console_height);
        let min_top_ratio = min_top_height / content_height.max(1.0);
        let max_top_ratio = 1.0 - (min_console_height / content_height.max(1.0));
        core.layout.top_console_ratio =
            core.layout.top_console_ratio.clamp(min_top_ratio, max_top_ratio);

        let top_height = (content_height * core.layout.top_console_ratio)
            .clamp(min_top_height, content_height - min_console_height);
        let horizontal_splitter_top = available_rect.top() + top_height;

        let top_rect = egui::Rect::from_min_max(
            available_rect.min,
            egui::pos2(available_rect.right(), horizontal_splitter_top),
        );
        let horizontal_splitter_rect = egui::Rect::from_min_max(
            egui::pos2(available_rect.left(), horizontal_splitter_top),
            egui::pos2(available_rect.right(), horizontal_splitter_top + horizontal_splitter_height),
        );
        let bottom_rect = egui::Rect::from_min_max(
            egui::pos2(available_rect.left(), horizontal_splitter_rect.bottom()),
            available_rect.max,
        );

        let horizontal_splitter_response = ui.interact(
            horizontal_splitter_rect,
            egui::Id::new("console_splitter"),
            egui::Sense::click_and_drag(),
        );
        let horizontal_splitter_response =
            horizontal_splitter_response.on_hover_and_drag_cursor(egui::CursorIcon::ResizeVertical);
        if horizontal_splitter_response.dragged() {
            let delta_y = ui.ctx().input(|input| input.pointer.delta().y);
            let ratio = (core.layout.top_console_ratio + delta_y / content_height.max(1.0))
                .clamp(min_top_ratio, max_top_ratio);
            core.events.push(AppEvent::SetTopConsoleRatio(ratio));
        }
        paint_splitter(ui, horizontal_splitter_rect, &horizontal_splitter_response, theme);
        (top_rect, Some(bottom_rect))
    } else {
        (available_rect, None)
    };

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(top_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            let available_rect = ui.available_rect_before_wrap();
            let vertical_splitter_width = 6.0;
            let total_width = available_rect.width().max(1.0);
            let min_pane_width = 240.0;
            let content_width = (total_width - vertical_splitter_width).max(min_pane_width * 2.0);
            let min_ratio = min_pane_width / content_width.max(1.0);
            let max_ratio = 1.0 - min_ratio;
            core.layout.editor_preview_ratio =
                core.layout.editor_preview_ratio.clamp(min_ratio, max_ratio);

            let left_width = (content_width * core.layout.editor_preview_ratio)
                .clamp(min_pane_width, content_width - min_pane_width);
            let vertical_splitter_left = available_rect.left() + left_width;

            let left_rect = egui::Rect::from_min_max(
                available_rect.min,
                egui::pos2(vertical_splitter_left, available_rect.bottom()),
            );
            let vertical_splitter_rect = egui::Rect::from_min_max(
                egui::pos2(vertical_splitter_left, available_rect.top()),
                egui::pos2(
                    vertical_splitter_left + vertical_splitter_width,
                    available_rect.bottom(),
                ),
            );
            let right_rect = egui::Rect::from_min_max(
                egui::pos2(vertical_splitter_rect.right(), available_rect.top()),
                available_rect.max,
            );

            let vertical_splitter_response = ui.interact(
                vertical_splitter_rect,
                egui::Id::new("editor_preview_splitter"),
                egui::Sense::click_and_drag(),
            );
            let vertical_splitter_response =
                vertical_splitter_response.on_hover_and_drag_cursor(egui::CursorIcon::ResizeHorizontal);
            if vertical_splitter_response.dragged() {
                let delta_x = ui.ctx().input(|input| input.pointer.delta().x);
                let ratio = (core.layout.editor_preview_ratio + delta_x / content_width.max(1.0))
                    .clamp(min_ratio, max_ratio);
                core.events.push(AppEvent::SetEditorPreviewRatio(ratio));
            }
            paint_splitter(ui, vertical_splitter_rect, &vertical_splitter_response, theme);

            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    let state = &mut core.state;
                    let events = &mut core.events;
                    crate::ui::editor_ui::draw_editor(ui, state, theme, events);
                },
            );

            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    preview_event = crate::ui::preview_ui::draw_preview(
                        ui,
                        PreviewPanelState {
                            has_active_tab: core.editor().active_index().is_some(),
                            is_running: core.is_running(),
                            commands: core.preview_commands(),
                            error: core.preview_error(),
                            native_size: core.preview_native_size(),
                        },
                        renderer,
                        theme,
                    );
                },
            );

            ui.allocate_rect(available_rect, egui::Sense::hover());
        },
    );

    if let Some(bottom_rect) = bottom_rect {
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(bottom_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                draw_console(ui, &core.console, theme, &mut core.events);
            },
        );
    }

    ui.allocate_rect(available_rect, egui::Sense::hover());
    if let Some(event) = preview_event {
        core.events.push(event);
    }
}

fn paint_splitter(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
    theme: &ThemePalette,
) {
    ui.painter().rect_filled(
        rect,
        0.0,
        if response.dragged() || response.hovered() {
            theme.active_tab_stroke
        } else {
            theme.surface_stroke
        },
    );
}
