use crate::app_core::AppCore;
use crate::events::app_events::AppEvent;
use crate::events::editor_events::EditorEvent;
use crate::state::app_state::PendingEditorCommand;
use crate::theme::palette_for;

pub fn handle_app_event(core: &mut AppCore, event: AppEvent) {
    match event {
        AppEvent::Editor(editor_event) => {
            super::editor_core::handle_editor_event(&mut core.state.editor, editor_event);
        }
        AppEvent::Undo => {
            core.state.pending_editor_command = Some(PendingEditorCommand::Undo);
        }
        AppEvent::Redo => {
            core.state.pending_editor_command = Some(PendingEditorCommand::Redo);
        }
        AppEvent::StartPreview => core.start_preview(),
        AppEvent::StopPreview => core.stop_preview(),
        AppEvent::RunInTerminal => core.run_in_terminal(),
        AppEvent::Notify(message) => core.notify(message),
        AppEvent::ToggleSuggestions => {
            core.state.suggestions.is_open = !core.state.suggestions.is_open;
        }
        AppEvent::ToggleConsole => {
            core.state.console_visible = !core.state.console_visible;
        }
        AppEvent::SetTheme(mode) => {
            core.state.theme_mode = mode;
            core.state.theme = palette_for(mode);
        }
        AppEvent::SetEditorPreviewRatio(ratio) => core.layout.editor_preview_ratio = ratio,
        AppEvent::SetTopConsoleRatio(ratio) => core.layout.top_console_ratio = ratio,
    }
}

pub fn wrap_editor(event: EditorEvent) -> AppEvent {
    AppEvent::Editor(event)
}
