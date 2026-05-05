use super::editor_state::EditorState;
use super::suggestion_state::SuggestionState;
use crate::theme::{ThemeMode, ThemePalette, palette_for};

#[derive(Clone, Copy)]
pub enum PendingEditorCommand {
    Undo,
    Redo,
}

pub struct AppState {
    pub editor: EditorState,
    pub suggestions: SuggestionState,
    pub console_visible: bool,
    pub pending_editor_command: Option<PendingEditorCommand>,
    pub theme_mode: ThemeMode,
    pub theme: ThemePalette,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            editor: EditorState::default(),
            suggestions: SuggestionState::default(),
            console_visible: true,
            pending_editor_command: None,
            theme_mode: ThemeMode::Dark,
            theme: palette_for(ThemeMode::Dark),
        }
    }
}
