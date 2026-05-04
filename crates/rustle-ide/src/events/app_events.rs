use crate::events::editor_events::EditorEvent;

pub enum AppEvent {
    Editor(EditorEvent),
    StartPreview,
    StopPreview,
    ToggleSuggestions,
    ToggleConsole,
    SetEditorPreviewRatio(f32),
    SetTopConsoleRatio(f32),
}
