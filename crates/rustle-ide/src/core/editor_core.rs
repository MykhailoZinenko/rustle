use crate::events::editor_events::EditorEvent;
use crate::services::file_service::FileService;
use crate::state::editor_state::EditorState;

pub fn handle_editor_event(state: &mut EditorState, event: EditorEvent) {
    match event {
        EditorEvent::NewTab => {
            state.new_empty();
        }

        EditorEvent::OpenFile(path) => {
            let content = FileService::read(&path);
            state.open_file(path, content);
        }
        EditorEvent::SetActiveTab(index) => {
            state.set_active(index);
        }
        EditorEvent::SaveActiveFile(path) => {
            if let Some(active_index) = state.active_index() {
                let tab = &mut state.tabs[active_index];
                let target_path = path.or_else(|| tab.file_path().map(|p| p.to_path_buf()));
                if let Some(target_path) = target_path {
                    if FileService::write(&target_path, &tab.buffer) {
                        tab.set_file_path(target_path);
                    }
                }
            }
        }
        EditorEvent::CloseTab(index) => {
            state.close_tab(index);
        }
    }
}
