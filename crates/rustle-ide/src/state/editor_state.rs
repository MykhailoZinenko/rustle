use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct EditorState {
    pub tabs: Vec<EditorTab>,
    pub active: Option<usize>,
}

pub struct EditorTab {
    pub id: TabId,
    pub file: Option<FileMeta>, // None = unsaved buffer
    pub buffer: String,
    pub cursor: Cursor,
    pub is_dirty: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TabId(u64);

pub struct FileMeta {
    pub path: PathBuf,
}

#[derive(Default, Clone, Copy)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
}

impl EditorState {
    pub fn open_file(&mut self, path: PathBuf, content: String) {
        let tab = EditorTab {
            id: TabId(self.next_id()),
            file: Some(FileMeta { path }),
            buffer: content,
            cursor: Cursor::default(),
            is_dirty: false,
        };

        self.tabs.push(tab);
        self.active = Some(self.tabs.len() - 1);
    }

    pub fn new_empty(&mut self) {
        let tab = EditorTab {
            id: TabId(self.next_id()),
            file: None,
            buffer: String::new(),
            cursor: Cursor::default(),
            is_dirty: false,
        };

        self.tabs.push(tab);
        self.active = Some(self.tabs.len() - 1);
    }

    pub fn set_active(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active = Some(index);
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(index);

        match self.active {
            Some(_) if self.tabs.is_empty() => self.active = None,
            Some(i) if i == index => {
                self.active = Some(index.saturating_sub(1).min(self.tabs.len().saturating_sub(1)));
            }
            Some(i) if i > index => self.active = Some(i - 1),
            _ => {}
        }
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active
    }

    fn next_id(&self) -> u64 {
        self.tabs.len() as u64 + 1
    }
}

impl EditorTab {
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn set_cursor(&mut self, line: usize, column: usize) {
        self.cursor = Cursor { line, column };
    }

    pub fn title(&self) -> String {
        let name = self
            .file
            .as_ref()
            .and_then(|file| file.path.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| "Untitled".to_string());

        if self.is_dirty {
            format!("{name} *")
        } else {
            name
        }
    }

    pub fn file_name(&self) -> &str {
        self.file
            .as_ref()
            .and_then(|file| file.path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled")
    }

    pub fn file_path(&self) -> Option<&Path> {
        self.file.as_ref().map(|file| file.path.as_path())
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file = Some(FileMeta { path });
        self.is_dirty = false;
    }
}
