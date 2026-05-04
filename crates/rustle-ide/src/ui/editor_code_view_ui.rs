use eframe::egui::{
    self, Align2, FontId, Frame, Id, Margin, Pos2, Rect, ScrollArea, Stroke, TextBuffer,
    TextEdit, Ui, vec2,
};
use egui::scroll_area::ScrollBarVisibility;
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::text::{CCursor, CCursorRange};

use crate::core::suggestion_core::{apply_suggestion, suggestion_context};
use crate::state::editor_state::{Cursor, EditorState, EditorTab};
use crate::state::suggestion_state::SuggestionState;
use crate::ui::editor_highlight_ui::highlight_layout;
use crate::ui::editor_suggestions_ui::draw_suggestions_popup;
use crate::ui::editor_status_bar_ui::EditorStatusSnapshot;
use crate::theme::{
    EDITOR_BG, EDITOR_CONTENT_PADDING, EDITOR_FONT_SIZE, GUTTER_BG, GUTTER_TEXT, SURFACE_STROKE,
};

pub fn draw_editor_code_view(
    ui: &mut Ui,
    editor: &mut EditorState,
    suggestions: &mut SuggestionState,
    active_index: usize,
    row_height: f32,
    line_count: usize,
) {
    let gutter_width = gutter_width(ui, line_count);
    let viewport_height = ui.available_height().max(row_height);
    let tab_id = editor.tabs[active_index].id;
    let text_edit_id = Id::new(("editor_text", tab_id));

    handle_cut_full_line(ui, suggestions, &mut editor.tabs[active_index], text_edit_id);
    handle_auto_indent(ui, suggestions, &mut editor.tabs[active_index], text_edit_id);
    handle_suggestion_keys(ui, suggestions, &mut editor.tabs[active_index], text_edit_id);

    Frame::new()
        .fill(EDITOR_BG)
        .stroke(Stroke::new(1.0, SURFACE_STROKE))
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt(Id::new(("editor_vertical_scroll", tab_id)))
                .auto_shrink([false, false])
                .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let gutter_origin = ui.cursor().min;
                        ui.add_space(gutter_width);

                        let horizontal_output = ScrollArea::horizontal()
                            .id_salt(Id::new(("editor_horizontal_scroll", tab_id)))
                            .auto_shrink([false, true])
                            .scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
                            .show(ui, |ui| {
                                Frame::new()
                                    .fill(EDITOR_BG)
                                    .inner_margin(Margin::same(EDITOR_CONTENT_PADDING))
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            let mut layouter =
                                                |ui: &Ui, text: &dyn TextBuffer, _wrap_width: f32| {
                                                    highlight_layout(ui, text.as_str())
                                                };

                                            let output =
                                                TextEdit::multiline(&mut editor.tabs[active_index].buffer)
                                                    .id(text_edit_id)
                                                    .code_editor()
                                                    .frame(Frame::NONE)
                                                    .desired_rows(line_count.max(
                                                        (viewport_height / row_height).ceil() as usize,
                                                    ))
                                                    .min_size(vec2(
                                                        ui.available_width(),
                                                        viewport_height,
                                                    ))
                                                    .layouter(&mut layouter)
                                                    .show(ui);

                                            if output.response.changed() {
                                                editor.tabs[active_index].mark_dirty();
                                            }

                                            if let Some(cursor_range) = output.cursor_range {
                                                let cursor = cursor_from_char_index(
                                                    &editor.tabs[active_index].buffer,
                                                    cursor_range.primary.index,
                                                );
                                                let current = editor.tabs[active_index].cursor;
                                                if current.line != cursor.line
                                                    || current.column != cursor.column
                                                {
                                                    editor.tabs[active_index]
                                                        .set_cursor(cursor.line, cursor.column);
                                                }
                                            }

                                            handle_suggestions(
                                                ui,
                                                suggestions,
                                                &mut editor.tabs[active_index],
                                                tab_id,
                                                text_edit_id,
                                                &output,
                                            );

                                            focus_end_on_empty_click(
                                                ui,
                                                &mut editor.tabs[active_index],
                                                text_edit_id,
                                                &output,
                                            );

                                            output
                                        })
                                        .inner
                                    })
                                    .inner
                            });

                        let gutter_rect = Rect::from_min_max(
                            gutter_origin,
                            Pos2::new(
                                gutter_origin.x + gutter_width,
                                horizontal_output.inner.response.rect.bottom(),
                            ),
                        );
                        ui.painter().rect_filled(gutter_rect, 0, GUTTER_BG);
                        paint_gutter(ui, gutter_rect, &horizontal_output.inner, line_count);
                    });
                });
        });
}

pub fn line_count(buffer: &str) -> usize {
    buffer.chars().filter(|ch| *ch == '\n').count() + 1
}

fn gutter_width(ui: &Ui, line_count: usize) -> f32 {
    let digits = line_count.max(1).to_string().len();
    let sample = "9".repeat(digits);
    let font_id = FontId::monospace(EDITOR_FONT_SIZE);
    let width = ui.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(sample, font_id, GUTTER_TEXT)
            .size()
            .x
    });

    width + 8.0
}

fn handle_suggestions(
    ui: &Ui,
    suggestions: &mut SuggestionState,
    tab: &mut EditorTab,
    tab_id: crate::state::editor_state::TabId,
    text_edit_id: Id,
    output: &TextEditOutput,
) {
    if !suggestions.is_open || !output.response.has_focus() {
        return;
    }

    let Some(cursor_range) = output.cursor_range else {
        return;
    };

    let suggestion_context = suggestion_context(&tab.buffer, cursor_range.primary.index);
    let cursor_rect = output
        .galley
        .pos_from_cursor(cursor_range.primary)
        .translate(output.galley_pos.to_vec2());
    let anchor = egui::pos2(
        cursor_rect.left() + EDITOR_CONTENT_PADDING as f32,
        cursor_rect.bottom() + 4.0,
    );

    if let Some(index) = draw_suggestions_popup(
        ui.ctx(),
        Id::new(("editor_suggestions", tab_id)),
        anchor,
        &suggestion_context,
        suggestions
            .selected_index
            .min(suggestion_context.items.len().saturating_sub(1)),
    ) {
        if let Some(item) = suggestion_context.items.get(index) {
            let new_cursor_index =
                apply_suggestion(&mut tab.buffer, suggestion_context.replace_range, &item.label);
            apply_suggestion_cursor(
                ui,
                tab,
                suggestions,
                text_edit_id,
                output.state.clone(),
                new_cursor_index,
            );
        }
    }
}

fn handle_suggestion_keys(
    ui: &Ui,
    suggestions: &mut SuggestionState,
    tab: &mut EditorTab,
    text_edit_id: Id,
) {
    if !suggestions.is_open || !ui.memory(|memory| memory.has_focus(text_edit_id)) {
        return;
    }

    let cursor_index = char_index_from_cursor(&tab.buffer, tab.cursor);
    let suggestion_context = suggestion_context(&tab.buffer, cursor_index);

    if suggestion_context.items.is_empty() {
        suggestions.selected_index = 0;
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            suggestions.is_open = false;
        }
        return;
    }

    suggestions.selected_index = suggestions
        .selected_index
        .min(suggestion_context.items.len().saturating_sub(1));

    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
        suggestions.selected_index = if suggestions.selected_index == 0 {
            suggestion_context.items.len() - 1
        } else {
            suggestions.selected_index - 1
        };
    }

    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
        suggestions.selected_index = (suggestions.selected_index + 1) % suggestion_context.items.len();
    }

    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        suggestions.is_open = false;
        suggestions.selected_index = 0;
        return;
    }

    if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
        if let Some(item) = suggestion_context.items.get(suggestions.selected_index) {
            let new_cursor_index =
                apply_suggestion(&mut tab.buffer, suggestion_context.replace_range, &item.label);
            let state = TextEditState::load(ui.ctx(), text_edit_id).unwrap_or_default();
            apply_suggestion_cursor(ui, tab, suggestions, text_edit_id, state, new_cursor_index);
        }
    }
}

fn handle_auto_indent(
    ui: &Ui,
    suggestions: &SuggestionState,
    tab: &mut EditorTab,
    text_edit_id: Id,
) {
    if suggestions.is_open || !ui.memory(|memory| memory.has_focus(text_edit_id)) {
        return;
    }

    if !ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
        return;
    }

    let state = TextEditState::load(ui.ctx(), text_edit_id).unwrap_or_default();
    let cursor_range = state
        .cursor
        .char_range()
        .unwrap_or_else(|| CCursorRange::one(CCursor::new(tab.buffer.chars().count())));
    let char_range = cursor_range.as_sorted_char_range();
    let insert_at = char_range.start;
    let indent = leading_indent_of_current_line(&tab.buffer, insert_at);
    let replacement = format!("\n{indent}");

    replace_char_range(&mut tab.buffer, char_range, &replacement);
    let new_cursor_index = insert_at + replacement.chars().count();
    apply_suggestion_cursor(ui, tab, &mut SuggestionState::default(), text_edit_id, state, new_cursor_index);
}

fn handle_cut_full_line(
    ui: &Ui,
    suggestions: &SuggestionState,
    tab: &mut EditorTab,
    text_edit_id: Id,
) {
    if suggestions.is_open || !ui.memory(|memory| memory.has_focus(text_edit_id)) {
        return;
    }

    let state = TextEditState::load(ui.ctx(), text_edit_id).unwrap_or_default();
    let Some(cursor_range) = state.cursor.char_range() else {
        return;
    };

    if !cursor_range.is_empty() {
        return;
    }

    if !ui.input_mut(|input| {
        input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::X))
    }) {
        return;
    }

    let line_range = current_line_char_range(&tab.buffer, cursor_range.primary.index);
    let line_text = slice_char_range(&tab.buffer, line_range.clone());
    ui.ctx().copy_text(line_text.to_string());

    replace_char_range(&mut tab.buffer, line_range.clone(), "");
    let new_cursor_index = line_range.start.min(tab.buffer.chars().count());
    apply_suggestion_cursor(ui, tab, &mut SuggestionState::default(), text_edit_id, state, new_cursor_index);
}

fn apply_suggestion_cursor(
    ui: &Ui,
    tab: &mut EditorTab,
    suggestions: &mut SuggestionState,
    text_edit_id: Id,
    mut state: TextEditState,
    new_cursor_index: usize,
) {
    tab.mark_dirty();
    let cursor = cursor_from_char_index(&tab.buffer, new_cursor_index);
    tab.set_cursor(cursor.line, cursor.column);

    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(new_cursor_index),
        CCursor::new(new_cursor_index),
    )));
    state.store(ui.ctx(), text_edit_id);
    suggestions.is_open = false;
    suggestions.selected_index = 0;
}

fn focus_end_on_empty_click(
    ui: &Ui,
    tab: &mut EditorTab,
    text_edit_id: Id,
    output: &TextEditOutput,
) {
    let remaining_rect = ui.available_rect_before_wrap();
    if remaining_rect.is_positive()
        && ui
            .interact(
                remaining_rect,
                Id::new(("editor_empty_click", tab.id)),
                egui::Sense::click(),
            )
            .clicked()
    {
        ui.memory_mut(|memory| memory.request_focus(text_edit_id));
        let end_index = tab.buffer.chars().count();
        let mut state = output.state.clone();
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(end_index))));
        state.store(ui.ctx(), text_edit_id);

        let cursor = cursor_from_char_index(&tab.buffer, end_index);
        tab.set_cursor(cursor.line, cursor.column);
    }
}

fn paint_gutter(ui: &Ui, gutter_rect: Rect, output: &TextEditOutput, line_count: usize) {
    let painter = ui.painter().with_clip_rect(gutter_rect);
    let digits = line_count.max(1).to_string().len();
    let font_id = FontId::monospace(EDITOR_FONT_SIZE);
    let x = gutter_rect.right() - 8.0;
    let visible_y = output.text_clip_rect.y_range();

    for (line_number, row_top) in galley_line_numbers(output, line_count) {
        let y = output.galley_pos.y + row_top;
        if y + output.galley.rows[line_number - 1].rect().height() < visible_y.min
            || y > visible_y.max
        {
            continue;
        }
        let text = format!("{line_number:>digits$}");
        painter.text(
            Pos2::new(x, y),
            Align2::RIGHT_TOP,
            text,
            font_id.clone(),
            GUTTER_TEXT,
        );
    }
}

fn galley_line_numbers(output: &TextEditOutput, line_count: usize) -> Vec<(usize, f32)> {
    output
        .galley
        .rows
        .iter()
        .take(line_count)
        .enumerate()
        .map(|(index, row)| (index + 1, row.pos.y))
        .collect()
}

fn cursor_from_char_index(buffer: &str, index: usize) -> Cursor {
    let mut line = 0;
    let mut column = 0;

    for (offset, ch) in buffer.chars().enumerate() {
        if offset >= index {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Cursor { line, column }
}

fn char_index_from_cursor(buffer: &str, cursor: Cursor) -> usize {
    let mut line = 0usize;
    let mut column = 0usize;
    let mut index = 0usize;

    for ch in buffer.chars() {
        if line == cursor.line && column == cursor.column {
            break;
        }

        index += 1;
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    index
}

fn leading_indent_of_current_line(buffer: &str, char_index: usize) -> String {
    let line_start = buffer[..char_to_byte_index(buffer, char_index)]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line = &buffer[line_start..char_to_byte_index(buffer, char_index)];

    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

fn replace_char_range(buffer: &mut String, char_range: std::ops::Range<usize>, replacement: &str) {
    let start = char_to_byte_index(buffer, char_range.start);
    let end = char_to_byte_index(buffer, char_range.end);
    buffer.replace_range(start..end, replacement);
}

fn char_to_byte_index(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(text.len())
}

fn current_line_char_range(buffer: &str, cursor_index: usize) -> std::ops::Range<usize> {
    let chars: Vec<char> = buffer.chars().collect();
    let cursor_index = cursor_index.min(chars.len());

    let mut start = cursor_index;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }

    let mut end = cursor_index;
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    if end < chars.len() && chars[end] == '\n' {
        end += 1;
    }

    start..end
}

fn slice_char_range(buffer: &str, char_range: std::ops::Range<usize>) -> &str {
    let start = char_to_byte_index(buffer, char_range.start);
    let end = char_to_byte_index(buffer, char_range.end);
    &buffer[start..end]
}

impl EditorStatusSnapshot {
    pub fn from_tab(tab: &EditorTab) -> Self {
        Self {
            file_path: tab.file_path().map(|path| path.to_path_buf()),
            file_name: tab.file_name().to_string(),
            cursor: tab.cursor,
            is_dirty: tab.is_dirty,
        }
    }
}
