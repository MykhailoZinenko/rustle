use super::editor_state::EditorState;
use super::suggestion_state::SuggestionState;

pub struct AppState {
    pub editor: EditorState,
    pub suggestions: SuggestionState,
    pub console_visible: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            editor: EditorState::default(),
            suggestions: SuggestionState::default(),
            console_visible: true,
        }
    }
}
