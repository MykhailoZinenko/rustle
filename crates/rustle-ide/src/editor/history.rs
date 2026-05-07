use crate::editor::selection::Selection;

#[derive(Debug, Clone)]
pub enum EditOperation {
    Insert { offset: usize, text: String },
    Delete { offset: usize, deleted: String },
}

impl EditOperation {
    pub fn inverse(&self) -> EditOperation {
        match self {
            EditOperation::Insert { offset, text } => EditOperation::Delete {
                offset: *offset,
                deleted: text.clone(),
            },
            EditOperation::Delete { offset, deleted } => EditOperation::Insert {
                offset: *offset,
                text: deleted.clone(),
            },
        }
    }
}

pub struct EditGroup {
    pub operations: Vec<EditOperation>,
    pub selections_before: Vec<Selection>,
    pub selections_after: Vec<Selection>,
}

pub struct EditHistory {
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
    saved_at_depth: usize,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            saved_at_depth: 0,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.undo_stack.len() != self.saved_at_depth
    }

    pub fn mark_saved(&mut self) {
        self.saved_at_depth = self.undo_stack.len();
    }

    pub fn push(&mut self, group: EditGroup) {
        self.undo_stack.push(group);
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> Option<&EditGroup> {
        if let Some(group) = self.undo_stack.pop() {
            self.redo_stack.push(group);
            self.redo_stack.last()
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&EditGroup> {
        if let Some(group) = self.redo_stack.pop() {
            self.undo_stack.push(group);
            self.undo_stack.last()
        } else {
            None
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_of_insert_is_delete() {
        let op = EditOperation::Insert { offset: 5, text: "hello".into() };
        let inv = op.inverse();
        match inv {
            EditOperation::Delete { offset, deleted } => {
                assert_eq!(offset, 5);
                assert_eq!(deleted, "hello");
            }
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn inverse_of_delete_is_insert() {
        let op = EditOperation::Delete { offset: 3, deleted: "world".into() };
        let inv = op.inverse();
        match inv {
            EditOperation::Insert { offset, text } => {
                assert_eq!(offset, 3);
                assert_eq!(text, "world");
            }
            _ => panic!("expected Insert"),
        }
    }

    #[test]
    fn new_history_cannot_undo_or_redo() {
        let history = EditHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn push_then_undo() {
        let mut history = EditHistory::new();
        history.push(EditGroup {
            operations: vec![EditOperation::Insert { offset: 0, text: "a".into() }],
            selections_before: vec![Selection::cursor(0)],
            selections_after: vec![Selection::cursor(1)],
        });
        assert!(history.can_undo());
        assert!(!history.can_redo());

        let group = history.undo().unwrap();
        assert_eq!(group.selections_before, vec![Selection::cursor(0)]);
        assert!(!history.can_undo());
        assert!(history.can_redo());
    }

    #[test]
    fn undo_then_redo() {
        let mut history = EditHistory::new();
        history.push(EditGroup {
            operations: vec![EditOperation::Insert { offset: 0, text: "x".into() }],
            selections_before: vec![Selection::cursor(0)],
            selections_after: vec![Selection::cursor(1)],
        });

        history.undo();
        let group = history.redo().unwrap();
        assert_eq!(group.selections_after, vec![Selection::cursor(1)]);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn push_after_undo_clears_redo_stack() {
        let mut history = EditHistory::new();
        history.push(EditGroup {
            operations: vec![EditOperation::Insert { offset: 0, text: "a".into() }],
            selections_before: vec![Selection::cursor(0)],
            selections_after: vec![Selection::cursor(1)],
        });
        history.undo();
        assert!(history.can_redo());

        history.push(EditGroup {
            operations: vec![EditOperation::Insert { offset: 0, text: "b".into() }],
            selections_before: vec![Selection::cursor(0)],
            selections_after: vec![Selection::cursor(1)],
        });
        assert!(!history.can_redo());
        assert!(history.can_undo());
    }

    #[test]
    fn undo_on_empty_returns_none() {
        let mut history = EditHistory::new();
        assert!(history.undo().is_none());
    }

    #[test]
    fn redo_on_empty_returns_none() {
        let mut history = EditHistory::new();
        assert!(history.redo().is_none());
    }
}
