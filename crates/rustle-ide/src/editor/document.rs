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
}
