use eframe::egui::{Context, Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy)]
pub enum ShortcutAction {
    Save,
    NewFile,
    OpenFile,
    CloseTab,
    ToggleRun,
    ToggleSuggestions,
}

pub fn consume_shortcut(ctx: &Context) -> Option<ShortcutAction> {
    ctx.input_mut(|input| {
        if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::S)) {
            return Some(ShortcutAction::Save);
        }
        if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::N)) {
            return Some(ShortcutAction::NewFile);
        }
        if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::O)) {
            return Some(ShortcutAction::OpenFile);
        }
        if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::W)) {
            return Some(ShortcutAction::CloseTab);
        }
        if input.consume_key(Modifiers::NONE, Key::F5) {
            return Some(ShortcutAction::ToggleRun);
        }
        if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Space)) {
            return Some(ShortcutAction::ToggleSuggestions);
        }
        None
    })
}
