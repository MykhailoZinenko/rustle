use std::path::PathBuf;

pub enum EditorEvent {
    NewTab,
    OpenFile(PathBuf),
    SaveActiveFile(Option<PathBuf>),
    SetActiveTab(usize),
    CloseTab(usize),
}
