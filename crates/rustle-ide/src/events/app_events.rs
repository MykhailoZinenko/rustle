use crate::events::editor_events::EditorEvent;
use crate::theme::ThemeMode;

#[allow(dead_code)]
pub enum AppEvent {
    Editor(EditorEvent),
    Undo,
    Redo,
    StartPreview,
    StopPreview,
    RunInTerminal,
    ToggleSuggestions,
    ToggleConsole,
    SetTheme(ThemeMode),
    SetEditorPreviewRatio(f32),
    SetTopConsoleRatio(f32),
    Notify(String),
}
