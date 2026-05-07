/// A selection in the document. `anchor` and `head` are **char** offsets
/// (not byte offsets). All rope interactions use char indices directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub fn cursor(offset: usize) -> Self {
        Self { anchor: offset, head: offset }
    }

    pub fn is_cursor(&self) -> bool {
        self.anchor == self.head
    }

    pub fn min(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn max(&self) -> usize {
        self.anchor.max(self.head)
    }

    pub fn range(&self) -> std::ops::Range<usize> {
        self.min()..self.max()
    }

    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.min() && offset < self.max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_creates_zero_width_selection() {
        let sel = Selection::cursor(5);
        assert_eq!(sel.anchor, 5);
        assert_eq!(sel.head, 5);
        assert!(sel.is_cursor());
    }

    #[test]
    fn selection_with_different_anchor_head_is_not_cursor() {
        let sel = Selection { anchor: 2, head: 8 };
        assert!(!sel.is_cursor());
    }

    #[test]
    fn min_max_with_forward_selection() {
        let sel = Selection { anchor: 3, head: 10 };
        assert_eq!(sel.min(), 3);
        assert_eq!(sel.max(), 10);
    }

    #[test]
    fn min_max_with_backward_selection() {
        let sel = Selection { anchor: 10, head: 3 };
        assert_eq!(sel.min(), 3);
        assert_eq!(sel.max(), 10);
    }

    #[test]
    fn range_returns_min_to_max() {
        let sel = Selection { anchor: 10, head: 3 };
        assert_eq!(sel.range(), 3..10);
    }

    #[test]
    fn contains_checks_within_range() {
        let sel = Selection { anchor: 3, head: 10 };
        assert!(!sel.contains(2));
        assert!(sel.contains(3));
        assert!(sel.contains(5));
        assert!(sel.contains(9));
        assert!(!sel.contains(10));
    }

    #[test]
    fn cursor_contains_nothing() {
        let sel = Selection::cursor(5);
        assert!(!sel.contains(5));
    }
}
