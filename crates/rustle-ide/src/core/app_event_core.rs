use crate::app_core::AppCore;
use crate::events::app_events::AppEvent;
use crate::events::editor_events::EditorEvent;

pub fn handle_app_event(core: &mut AppCore, event: AppEvent) {
    match event {
        AppEvent::Editor(editor_event) => {
            super::editor_core::handle_editor_event(&mut core.state.editor, editor_event);
        }
        AppEvent::StartPreview => core.start_preview(),
        AppEvent::StopPreview => core.stop_preview(),
        AppEvent::ToggleSuggestions => {
            core.state.suggestions.is_open = !core.state.suggestions.is_open;
        }
        AppEvent::ToggleConsole => {
            core.state.console_visible = !core.state.console_visible;
        }
        AppEvent::SetEditorPreviewRatio(ratio) => core.layout.editor_preview_ratio = ratio,
        AppEvent::SetTopConsoleRatio(ratio) => core.layout.top_console_ratio = ratio,
    }
}

pub fn wrap_editor(event: EditorEvent) -> AppEvent {
    AppEvent::Editor(event)
}
