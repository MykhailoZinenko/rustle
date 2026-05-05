use crate::events::editor_events::EditorEvent;
use crate::theme::ThemeMode;

pub enum AppEvent {
    Editor(EditorEvent),
    Undo,
    Redo,
    StartPreview,
    StopPreview,
    ToggleSuggestions,
    ToggleConsole,
    SetTheme(ThemeMode),
    SetEditorPreviewRatio(f32),
    SetTopConsoleRatio(f32),
}
