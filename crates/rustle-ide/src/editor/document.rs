use std::path::PathBuf;

use ropey::Rope;

use crate::editor::history::{EditGroup, EditHistory, EditOperation};
use crate::editor::markers::{Marker, MarkerCategory, MarkerSet};
use crate::editor::selection::Selection;

pub struct FileMeta {
    pub path: PathBuf,
}

pub struct Document {
    rope: Rope,
    history: EditHistory,
    selections: Vec<Selection>,
    pub markers: MarkerSet,
    dirty: bool,
    pub file: Option<FileMeta>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            history: EditHistory::new(),
            selections: vec![Selection::cursor(0)],
            markers: MarkerSet::new(),
            dirty: false,
            file: None,
        }
    }

    pub fn from_file(path: PathBuf, content: &str) -> Self {
        Self {
            rope: Rope::from_str(content),
            history: EditHistory::new(),
            selections: vec![Selection::cursor(0)],
            markers: MarkerSet::new(),
            dirty: false,
            file: Some(FileMeta { path }),
        }
    }

    pub fn text(&self) -> &Rope {
        &self.rope
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn selections(&self) -> &[Selection] {
        &self.selections
    }

    pub fn set_selections(&mut self, selections: Vec<Selection>) {
        self.selections = selections;
    }

    pub fn primary_selection(&self) -> Selection {
        self.selections.last().copied().unwrap_or(Selection::cursor(0))
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line_text(&self, line: usize) -> ropey::RopeSlice<'_> {
        self.rope.line(line)
    }

    pub fn byte_to_line_col(&self, byte_offset: usize) -> (usize, usize) {
        let clamped = byte_offset.min(self.rope.len_bytes());
        let line = self.rope.byte_to_line(clamped);
        let line_start = self.rope.line_to_byte(line);
        let col = clamped - line_start;
        (line, col)
    }

    pub fn line_col_to_byte(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.rope.len_lines().saturating_sub(1));
        let line_start = self.rope.line_to_byte(line);
        let line_len = self.rope.line(line).len_bytes();
        line_start + col.min(line_len)
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn insert_text(&mut self, text: &str) {
        let text = text.to_string();
        let selections_before = self.selections.clone();
        let mut operations = Vec::new();
        let mut new_selections = Vec::new();
        let mut cumulative_delta: isize = 0;

        let mut indexed: Vec<(usize, Selection)> = self.selections
            .iter()
            .copied()
            .enumerate()
            .collect();
        indexed.sort_by_key(|(_, sel)| sel.min());

        for (orig_index, sel) in indexed {
            let adjusted_min = (sel.min() as isize + cumulative_delta) as usize;
            let adjusted_max = (sel.max() as isize + cumulative_delta) as usize;

            let old_len = adjusted_max - adjusted_min;
            if old_len > 0 {
                let old_text: String = self.rope.slice(adjusted_min..adjusted_max).chars().collect();
                self.rope.remove(adjusted_min..adjusted_max);
                operations.push(EditOperation::Delete {
                    offset: adjusted_min,
                    deleted: old_text,
                });
                self.markers.adjust(adjusted_min, old_len, 0);
            }

            let new_len = text.len();
            if !text.is_empty() {
                self.rope.insert(adjusted_min, &text);
                operations.push(EditOperation::Insert {
                    offset: adjusted_min,
                    text: text.clone(),
                });
                self.markers.adjust(adjusted_min, 0, new_len);
            }

            let new_cursor = adjusted_min + new_len;
            new_selections.push((orig_index, Selection::cursor(new_cursor)));
            cumulative_delta += new_len as isize - old_len as isize;
        }

        new_selections.sort_by_key(|(idx, _)| *idx);
        self.selections = new_selections.into_iter().map(|(_, sel)| sel).collect();

        if !operations.is_empty() {
            self.history.push(EditGroup {
                operations,
                selections_before,
                selections_after: self.selections.clone(),
            });
            self.dirty = true;
        }
    }

    pub fn backspace(&mut self) {
        let selections_before = self.selections.clone();
        let mut operations = Vec::new();
        let mut new_selections = Vec::new();
        let mut cumulative_delta: isize = 0;

        let mut indexed: Vec<(usize, Selection)> = self.selections
            .iter()
            .copied()
            .enumerate()
            .collect();
        indexed.sort_by_key(|(_, sel)| sel.min());

        for (orig_index, sel) in indexed {
            let adjusted_anchor = (sel.anchor as isize + cumulative_delta) as usize;
            let adjusted_head = (sel.head as isize + cumulative_delta) as usize;
            let adjusted = Selection { anchor: adjusted_anchor, head: adjusted_head };

            if !adjusted.is_cursor() {
                let min = adjusted.min();
                let max = adjusted.max();
                let old_text: String = self.rope.slice(min..max).chars().collect();
                let old_len = max - min;
                self.rope.remove(min..max);
                operations.push(EditOperation::Delete { offset: min, deleted: old_text });
                self.markers.adjust(min, old_len, 0);
                new_selections.push((orig_index, Selection::cursor(min)));
                cumulative_delta -= old_len as isize;
            } else if adjusted.head > 0 {
                let char_idx = self.rope.byte_to_char(adjusted.head);
                let prev_char_byte = self.rope.char_to_byte(char_idx.saturating_sub(1));
                let del_start = prev_char_byte;
                let del_end = adjusted.head;
                let old_text: String = self.rope.slice(del_start..del_end).chars().collect();
                let old_len = del_end - del_start;
                self.rope.remove(del_start..del_end);
                operations.push(EditOperation::Delete { offset: del_start, deleted: old_text });
                self.markers.adjust(del_start, old_len, 0);
                new_selections.push((orig_index, Selection::cursor(del_start)));
                cumulative_delta -= old_len as isize;
            } else {
                new_selections.push((orig_index, Selection::cursor(0)));
            }
        }

        new_selections.sort_by_key(|(idx, _)| *idx);
        self.selections = new_selections.into_iter().map(|(_, sel)| sel).collect();

        if !operations.is_empty() {
            self.history.push(EditGroup {
                operations,
                selections_before,
                selections_after: self.selections.clone(),
            });
            self.dirty = true;
        }
    }

    pub fn undo(&mut self) {
        if let Some(group) = self.history.undo() {
            let selections = group.selections_before.clone();
            let ops: Vec<EditOperation> = group.operations.iter().rev().map(|op| op.inverse()).collect();
            for op in ops {
                match &op {
                    EditOperation::Insert { offset, text } => {
                        self.rope.insert(*offset, text);
                    }
                    EditOperation::Delete { offset, deleted } => {
                        self.rope.remove(*offset..*offset + deleted.len());
                    }
                }
            }
            self.selections = selections;
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(group) = self.history.redo() {
            let selections = group.selections_after.clone();
            let ops: Vec<EditOperation> = group.operations.clone();
            for op in &ops {
                match op {
                    EditOperation::Insert { offset, text } => {
                        self.rope.insert(*offset, text);
                    }
                    EditOperation::Delete { offset, deleted } => {
                        self.rope.remove(*offset..*offset + deleted.len());
                    }
                }
            }
            self.selections = selections;
            self.dirty = true;
        }
    }

    pub fn delete_forward(&mut self) {
        let selections_before = self.selections.clone();
        let mut operations = Vec::new();
        let mut new_selections = Vec::new();
        let mut cumulative_delta: isize = 0;

        let mut indexed: Vec<(usize, Selection)> = self.selections
            .iter()
            .copied()
            .enumerate()
            .collect();
        indexed.sort_by_key(|(_, sel)| sel.min());

        for (orig_index, sel) in indexed {
            let adjusted_anchor = (sel.anchor as isize + cumulative_delta) as usize;
            let adjusted_head = (sel.head as isize + cumulative_delta) as usize;
            let adjusted = Selection { anchor: adjusted_anchor, head: adjusted_head };

            if !adjusted.is_cursor() {
                let min = adjusted.min();
                let max = adjusted.max();
                let old_text: String = self.rope.slice(min..max).chars().collect();
                let old_len = max - min;
                self.rope.remove(min..max);
                operations.push(EditOperation::Delete { offset: min, deleted: old_text });
                self.markers.adjust(min, old_len, 0);
                new_selections.push((orig_index, Selection::cursor(min)));
                cumulative_delta -= old_len as isize;
            } else if adjusted.head < self.rope.len_bytes() {
                let char_idx = self.rope.byte_to_char(adjusted.head);
                let next_char_byte = self.rope.char_to_byte((char_idx + 1).min(self.rope.len_chars()));
                let del_start = adjusted.head;
                let del_end = next_char_byte;
                let old_text: String = self.rope.slice(del_start..del_end).chars().collect();
                let old_len = del_end - del_start;
                self.rope.remove(del_start..del_end);
                operations.push(EditOperation::Delete { offset: del_start, deleted: old_text });
                self.markers.adjust(del_start, old_len, 0);
                new_selections.push((orig_index, Selection::cursor(del_start)));
                cumulative_delta -= old_len as isize;
            } else {
                new_selections.push((orig_index, adjusted));
            }
        }

        new_selections.sort_by_key(|(idx, _)| *idx);
        self.selections = new_selections.into_iter().map(|(_, sel)| sel).collect();

        if !operations.is_empty() {
            self.history.push(EditGroup {
                operations,
                selections_before,
                selections_after: self.selections.clone(),
            });
            self.dirty = true;
        }
    }

    pub fn find_all(&self, query: &str, case_sensitive: bool) -> Vec<std::ops::Range<usize>> {
        let text = self.rope.to_string();
        let (search_text, search_query) = if case_sensitive {
            (text.clone(), query.to_string())
        } else {
            (text.to_lowercase(), query.to_lowercase())
        };
        let mut results = Vec::new();
        let mut start = 0;
        while let Some(pos) = search_text[start..].find(&search_query) {
            let abs_pos = start + pos;
            results.push(abs_pos..abs_pos + query.len());
            start = abs_pos + 1;
        }
        results
    }

    pub fn replace_range(&mut self, range: std::ops::Range<usize>, replacement: &str) {
        let selections_before = self.selections.clone();
        let old_text: String = self.rope.slice(range.start..range.end).chars().collect();
        let old_len = range.end - range.start;
        let new_len = replacement.len();

        self.rope.remove(range.start..range.end);
        if !replacement.is_empty() {
            self.rope.insert(range.start, replacement);
        }
        self.markers.adjust(range.start, old_len, new_len);

        let mut ops = Vec::new();
        if !old_text.is_empty() {
            ops.push(EditOperation::Delete { offset: range.start, deleted: old_text });
        }
        if !replacement.is_empty() {
            ops.push(EditOperation::Insert { offset: range.start, text: replacement.to_string() });
        }
        if !ops.is_empty() {
            self.history.push(EditGroup {
                operations: ops,
                selections_before,
                selections_after: self.selections.clone(),
            });
            self.dirty = true;
        }
    }

    pub fn find_matching_bracket(&self, byte_offset: usize) -> Option<usize> {
        let char_idx = self.rope.byte_to_char(byte_offset);
        if char_idx >= self.rope.len_chars() {
            return None;
        }
        let c = self.rope.char(char_idx);
        let (open, close, forward) = match c {
            '(' => ('(', ')', true),
            ')' => ('(', ')', false),
            '[' => ('[', ']', true),
            ']' => ('[', ']', false),
            '{' => ('{', '}', true),
            '}' => ('{', '}', false),
            _ => return None,
        };

        let mut depth = 0i32;
        if forward {
            for i in char_idx..self.rope.len_chars() {
                let current = self.rope.char(i);
                if current == open { depth += 1; }
                if current == close { depth -= 1; }
                if depth == 0 {
                    return Some(self.rope.char_to_byte(i));
                }
            }
        } else {
            for i in (0..=char_idx).rev() {
                let current = self.rope.char(i);
                if current == close { depth += 1; }
                if current == open { depth -= 1; }
                if depth == 0 {
                    return Some(self.rope.char_to_byte(i));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_is_empty() {
        let doc = Document::new();
        assert_eq!(doc.to_string(), "");
        assert_eq!(doc.line_count(), 1);
        assert!(!doc.is_dirty());
    }

    #[test]
    fn from_file_preserves_content() {
        let doc = Document::from_file(
            PathBuf::from("/tmp/test.rustle"),
            "hello\nworld",
        );
        assert_eq!(doc.to_string(), "hello\nworld");
        assert_eq!(doc.line_count(), 2);
        assert!(!doc.is_dirty());
        assert!(doc.file.is_some());
    }

    #[test]
    fn default_selection_is_cursor_at_zero() {
        let doc = Document::new();
        assert_eq!(doc.selections(), &[Selection::cursor(0)]);
    }

    #[test]
    fn primary_selection_is_last() {
        let mut doc = Document::new();
        doc.set_selections(vec![
            Selection::cursor(0),
            Selection::cursor(5),
        ]);
        assert_eq!(doc.primary_selection(), Selection::cursor(5));
    }

    #[test]
    fn byte_to_line_col_first_line() {
        let doc = Document::from_file(PathBuf::from("t"), "hello\nworld");
        assert_eq!(doc.byte_to_line_col(0), (0, 0));
        assert_eq!(doc.byte_to_line_col(4), (0, 4));
    }

    #[test]
    fn byte_to_line_col_second_line() {
        let doc = Document::from_file(PathBuf::from("t"), "hello\nworld");
        assert_eq!(doc.byte_to_line_col(6), (1, 0));
        assert_eq!(doc.byte_to_line_col(9), (1, 3));
    }

    #[test]
    fn line_col_to_byte_roundtrips() {
        let doc = Document::from_file(PathBuf::from("t"), "hello\nworld\nfoo");
        for offset in [0, 3, 5, 6, 10, 12, 14] {
            let (line, col) = doc.byte_to_line_col(offset);
            assert_eq!(doc.line_col_to_byte(line, col), offset);
        }
    }

    #[test]
    fn line_col_to_byte_clamps_column() {
        let doc = Document::from_file(PathBuf::from("t"), "hi\nworld");
        let result = doc.line_col_to_byte(0, 100);
        assert!(result <= 3);
    }

    #[test]
    fn line_text_returns_line_content() {
        let doc = Document::from_file(PathBuf::from("t"), "aaa\nbbb\nccc");
        let line1: String = doc.line_text(1).chars().collect();
        assert_eq!(line1, "bbb\n");
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello");
        doc.dirty = true;
        doc.mark_saved();
        assert!(!doc.is_dirty());
    }

    #[test]
    fn text_returns_rope_ref() {
        let doc = Document::from_file(PathBuf::from("t"), "hello");
        assert_eq!(doc.text().len_bytes(), 5);
    }

    #[test]
    fn insert_text_at_cursor() {
        let mut doc = Document::new();
        doc.insert_text("hello");
        assert_eq!(doc.to_string(), "hello");
        assert_eq!(doc.primary_selection(), Selection::cursor(5));
        assert!(doc.is_dirty());
    }

    #[test]
    fn insert_text_replaces_selection() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello world");
        doc.set_selections(vec![Selection { anchor: 5, head: 11 }]);
        doc.insert_text("!");
        assert_eq!(doc.to_string(), "hello!");
        assert_eq!(doc.primary_selection(), Selection::cursor(6));
    }

    #[test]
    fn insert_text_multiple_cursors() {
        let mut doc = Document::from_file(PathBuf::from("t"), "aa bb cc");
        doc.set_selections(vec![
            Selection::cursor(2),
            Selection::cursor(5),
            Selection::cursor(8),
        ]);
        doc.insert_text("X");
        assert_eq!(doc.to_string(), "aaX bbX ccX");
    }

    #[test]
    fn backspace_deletes_char_before_cursor() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello");
        doc.set_selections(vec![Selection::cursor(5)]);
        doc.backspace();
        assert_eq!(doc.to_string(), "hell");
        assert_eq!(doc.primary_selection(), Selection::cursor(4));
    }

    #[test]
    fn backspace_deletes_selection() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello world");
        doc.set_selections(vec![Selection { anchor: 2, head: 8 }]);
        doc.backspace();
        assert_eq!(doc.to_string(), "herld");
        assert_eq!(doc.primary_selection(), Selection::cursor(2));
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello");
        doc.set_selections(vec![Selection::cursor(0)]);
        doc.backspace();
        assert_eq!(doc.to_string(), "hello");
    }

    #[test]
    fn delete_forward_removes_char_after_cursor() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello");
        doc.set_selections(vec![Selection::cursor(0)]);
        doc.delete_forward();
        assert_eq!(doc.to_string(), "ello");
        assert_eq!(doc.primary_selection(), Selection::cursor(0));
    }

    #[test]
    fn delete_forward_at_end_does_nothing() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hi");
        doc.set_selections(vec![Selection::cursor(2)]);
        doc.delete_forward();
        assert_eq!(doc.to_string(), "hi");
    }

    #[test]
    fn delete_forward_with_selection_removes_selection() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello world");
        doc.set_selections(vec![Selection { anchor: 0, head: 6 }]);
        doc.delete_forward();
        assert_eq!(doc.to_string(), "world");
    }

    #[test]
    fn undo_reverses_insert() {
        let mut doc = Document::new();
        doc.insert_text("hello");
        assert_eq!(doc.to_string(), "hello");

        doc.undo();
        assert_eq!(doc.to_string(), "");
        assert_eq!(doc.primary_selection(), Selection::cursor(0));
    }

    #[test]
    fn redo_reapplies_insert() {
        let mut doc = Document::new();
        doc.insert_text("hello");
        doc.undo();
        assert_eq!(doc.to_string(), "");

        doc.redo();
        assert_eq!(doc.to_string(), "hello");
        assert_eq!(doc.primary_selection(), Selection::cursor(5));
    }

    #[test]
    fn undo_reverses_backspace() {
        let mut doc = Document::from_file(PathBuf::from("t"), "abc");
        doc.set_selections(vec![Selection::cursor(3)]);
        doc.backspace();
        assert_eq!(doc.to_string(), "ab");

        doc.undo();
        assert_eq!(doc.to_string(), "abc");
    }

    #[test]
    fn multiple_undo_redo_roundtrip() {
        let mut doc = Document::new();
        doc.insert_text("a");
        doc.insert_text("b");
        doc.insert_text("c");
        assert_eq!(doc.to_string(), "abc");

        doc.undo();
        assert_eq!(doc.to_string(), "ab");
        doc.undo();
        assert_eq!(doc.to_string(), "a");
        doc.undo();
        assert_eq!(doc.to_string(), "");

        doc.redo();
        assert_eq!(doc.to_string(), "a");
        doc.redo();
        assert_eq!(doc.to_string(), "ab");
        doc.redo();
        assert_eq!(doc.to_string(), "abc");
    }

    #[test]
    fn undo_on_clean_document_does_nothing() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello");
        doc.undo();
        assert_eq!(doc.to_string(), "hello");
    }

    #[test]
    fn new_edit_after_undo_clears_redo() {
        let mut doc = Document::new();
        doc.insert_text("a");
        doc.insert_text("b");
        doc.undo();
        doc.insert_text("c");
        assert_eq!(doc.to_string(), "ac");

        doc.redo();
        assert_eq!(doc.to_string(), "ac");
    }

    #[test]
    fn find_all_returns_byte_ranges() {
        let doc = Document::from_file(PathBuf::from("t"), "abcabc");
        let matches = doc.find_all("abc", true);
        assert_eq!(matches, vec![0..3, 3..6]);
    }

    #[test]
    fn find_all_case_insensitive() {
        let doc = Document::from_file(PathBuf::from("t"), "Hello hello HELLO");
        let matches = doc.find_all("hello", false);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_all_no_matches() {
        let doc = Document::from_file(PathBuf::from("t"), "hello");
        let matches = doc.find_all("xyz", true);
        assert!(matches.is_empty());
    }

    #[test]
    fn find_all_overlapping_positions() {
        let doc = Document::from_file(PathBuf::from("t"), "aaa");
        let matches = doc.find_all("aa", true);
        assert_eq!(matches, vec![0..2, 1..3]);
    }

    #[test]
    fn replace_range_basic() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello world");
        doc.replace_range(5..11, "!");
        assert_eq!(doc.to_string(), "hello!");
        assert!(doc.is_dirty());
    }

    #[test]
    fn replace_range_undo() {
        let mut doc = Document::from_file(PathBuf::from("t"), "hello world");
        doc.replace_range(5..11, "!");
        doc.undo();
        assert_eq!(doc.to_string(), "hello world");
    }

    #[test]
    fn find_matching_bracket_forward() {
        let doc = Document::from_file(PathBuf::from("t"), "fn(a, b)");
        let matching = doc.find_matching_bracket(2);
        assert_eq!(matching, Some(7));
    }

    #[test]
    fn find_matching_bracket_backward() {
        let doc = Document::from_file(PathBuf::from("t"), "fn(a, b)");
        let matching = doc.find_matching_bracket(7);
        assert_eq!(matching, Some(2));
    }

    #[test]
    fn find_matching_bracket_nested() {
        let doc = Document::from_file(PathBuf::from("t"), "((a))");
        let matching = doc.find_matching_bracket(0);
        assert_eq!(matching, Some(4));
    }

    #[test]
    fn find_matching_bracket_curly() {
        let doc = Document::from_file(PathBuf::from("t"), "if { x }");
        let matching = doc.find_matching_bracket(3);
        assert_eq!(matching, Some(7));
    }

    #[test]
    fn find_matching_bracket_square() {
        let doc = Document::from_file(PathBuf::from("t"), "a[0]");
        let matching = doc.find_matching_bracket(1);
        assert_eq!(matching, Some(3));
    }

    #[test]
    fn find_matching_bracket_no_match() {
        let doc = Document::from_file(PathBuf::from("t"), "(unclosed");
        let matching = doc.find_matching_bracket(0);
        assert_eq!(matching, None);
    }

    #[test]
    fn find_matching_bracket_not_a_bracket() {
        let doc = Document::from_file(PathBuf::from("t"), "hello");
        let matching = doc.find_matching_bracket(0);
        assert_eq!(matching, None);
    }
}
