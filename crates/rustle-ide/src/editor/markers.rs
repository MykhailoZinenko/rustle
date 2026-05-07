use ropey::Rope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkerCategory {
    SearchMatch,
    CurrentSearchMatch,
    BracketPair,
    SelectionOccurrence,
}

#[derive(Debug, Clone)]
pub struct Marker {
    pub category: MarkerCategory,
    pub start: usize,
    pub end: usize,
}

pub struct MarkerSet {
    markers: Vec<Marker>,
}

impl MarkerSet {
    pub fn new() -> Self {
        Self { markers: Vec::new() }
    }

    pub fn add(&mut self, marker: Marker) {
        self.markers.push(marker);
    }

    pub fn clear_category(&mut self, category: MarkerCategory) {
        self.markers.retain(|m| m.category != category);
    }

    pub fn markers_on_line(&self, rope: &Rope, line: usize) -> Vec<&Marker> {
        let line_start = rope.line_to_byte(line);
        let line_end = if line + 1 < rope.len_lines() {
            rope.line_to_byte(line + 1)
        } else {
            rope.len_bytes()
        };
        self.markers
            .iter()
            .filter(|m| m.start < line_end && m.end > line_start)
            .collect()
    }

    pub fn adjust(&mut self, offset: usize, old_len: usize, new_len: usize) {
        let delta = new_len as isize - old_len as isize;
        self.markers.retain_mut(|m| {
            if m.end <= offset {
                true
            } else if m.start >= offset + old_len {
                m.start = (m.start as isize + delta) as usize;
                m.end = (m.end as isize + delta) as usize;
                true
            } else {
                false
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope(s: &str) -> Rope {
        Rope::from_str(s)
    }

    #[test]
    fn new_marker_set_is_empty() {
        let ms = MarkerSet::new();
        let r = rope("hello\nworld\n");
        assert!(ms.markers_on_line(&r, 0).is_empty());
    }

    #[test]
    fn add_and_retrieve_marker_on_line() {
        let mut ms = MarkerSet::new();
        let r = rope("hello\nworld\n");
        ms.add(Marker {
            category: MarkerCategory::SearchMatch,
            start: 6,
            end: 11,
        });
        assert!(ms.markers_on_line(&r, 0).is_empty());
        assert_eq!(ms.markers_on_line(&r, 1).len(), 1);
    }

    #[test]
    fn clear_category_removes_only_that_category() {
        let mut ms = MarkerSet::new();
        ms.add(Marker { category: MarkerCategory::SearchMatch, start: 0, end: 3 });
        ms.add(Marker { category: MarkerCategory::BracketPair, start: 5, end: 6 });
        ms.clear_category(MarkerCategory::SearchMatch);

        let r = rope("hello world");
        let line0 = ms.markers_on_line(&r, 0);
        assert_eq!(line0.len(), 1);
        assert_eq!(line0[0].category, MarkerCategory::BracketPair);
    }

    #[test]
    fn adjust_shifts_markers_after_insert() {
        let mut ms = MarkerSet::new();
        ms.add(Marker { category: MarkerCategory::SearchMatch, start: 5, end: 8 });
        ms.adjust(2, 0, 3);

        let r = rope("heXXXllo world");
        let markers = ms.markers_on_line(&r, 0);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].start, 8);
        assert_eq!(markers[0].end, 11);
    }

    #[test]
    fn adjust_removes_markers_overlapping_edit() {
        let mut ms = MarkerSet::new();
        ms.add(Marker { category: MarkerCategory::SearchMatch, start: 3, end: 7 });
        ms.adjust(4, 2, 0);

        let r = rope("helo world");
        assert!(ms.markers_on_line(&r, 0).is_empty());
    }

    #[test]
    fn adjust_leaves_markers_before_edit_unchanged() {
        let mut ms = MarkerSet::new();
        ms.add(Marker { category: MarkerCategory::SearchMatch, start: 0, end: 3 });
        ms.adjust(5, 0, 3);

        let r = rope("helXXXlo world");
        let markers = ms.markers_on_line(&r, 0);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].start, 0);
        assert_eq!(markers[0].end, 3);
    }

    #[test]
    fn marker_spanning_multiple_lines_appears_on_both() {
        let mut ms = MarkerSet::new();
        let r = rope("aaa\nbbb\nccc\n");
        ms.add(Marker { category: MarkerCategory::SearchMatch, start: 2, end: 6 });

        assert_eq!(ms.markers_on_line(&r, 0).len(), 1);
        assert_eq!(ms.markers_on_line(&r, 1).len(), 1);
        assert!(ms.markers_on_line(&r, 2).is_empty());
    }
}
