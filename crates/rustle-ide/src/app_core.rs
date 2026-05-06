use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use rustle_lang::{DrawCommand, Input};

use crate::terminal::{BackendCommand, BackendSettings, PtyEvent, TerminalBackend};

use crate::core::app_event_core::handle_app_event;
use crate::events::app_events::AppEvent;
use crate::runner::{
    ConsoleEntry, ConsoleLevel, PreviewRunnerState, PreviewSnapshot, TickStatus,
    render_static_preview, spawn_runtime, stop_runtime, tick_runtime,
};
use crate::state::app_state::AppState;

const MAX_CONSOLE_ENTRIES: usize = 1000;
const NOTIFICATION_DURATION: Duration = Duration::from_secs(3);

pub struct LayoutState {
    pub editor_preview_ratio: f32,
    pub top_console_ratio: f32,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            editor_preview_ratio: 0.5,
            top_console_ratio: 0.72,
        }
    }
}

pub struct Notification {
    pub message: String,
    pub created_at: Instant,
}

pub struct AppCore {
    pub state: AppState,
    pub events: Vec<AppEvent>,
    pub runner: PreviewRunnerState,
    pub static_preview_commands: Vec<DrawCommand>,
    pub static_preview_error: Option<String>,
    pub runtime_preview_commands: Vec<DrawCommand>,
    pub runtime_preview_error: Option<String>,
    pub console: VecDeque<ConsoleEntry>,
    pub last_static_preview: Option<PreviewSnapshot>,
    pub layout: LayoutState,
    pub terminal: Option<TerminalBackend>,
    pub pty_event_rx: Option<mpsc::Receiver<(u64, PtyEvent)>>,
    pub notifications: Vec<Notification>,
    terminal_initialized: bool,
}

impl Default for AppCore {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            events: Vec::new(),
            runner: PreviewRunnerState::default(),
            static_preview_commands: Vec::new(),
            static_preview_error: None,
            runtime_preview_commands: Vec::new(),
            runtime_preview_error: None,
            console: VecDeque::new(),
            last_static_preview: None,
            layout: LayoutState::default(),
            terminal: None,
            pty_event_rx: None,
            notifications: Vec::new(),
            terminal_initialized: false,
        }
    }
}

impl AppCore {
    pub fn editor(&self) -> &crate::state::editor_state::EditorState {
        &self.state.editor
    }

    pub fn is_running(&self) -> bool {
        self.runner.script.is_some()
    }

    pub fn queue_event(&mut self, event: AppEvent) {
        self.events.push(event);
    }

    pub fn notify(&mut self, message: String) {
        self.notifications.push(Notification {
            message,
            created_at: Instant::now(),
        });
    }

    pub fn ensure_terminal(&mut self, ctx: &egui::Context) {
        if self.terminal_initialized {
            return;
        }
        self.terminal_initialized = true;

        let (tx, rx) = mpsc::channel();
        let settings = BackendSettings {
            shell: if cfg!(target_os = "macos") {
                "/bin/zsh".to_string()
            } else {
                "/bin/bash".to_string()
            },
            args: vec![],
            working_directory: std::env::current_dir().ok(),
        };

        match TerminalBackend::new(1, ctx.clone(), tx, settings) {
            Ok(backend) => {
                self.terminal = Some(backend);
                self.pty_event_rx = Some(rx);
            }
            Err(e) => {
                eprintln!("Failed to create terminal: {e}");
            }
        }
    }

    pub fn run_in_terminal(&mut self) {
        let Some(active_index) = self.state.editor.active_index() else {
            self.notify("No active file to run".to_string());
            return;
        };

        let tab = &self.state.editor.tabs[active_index];
        let path = match tab.file_path() {
            Some(p) => p.to_path_buf(),
            None => {
                self.notify("Please save the file before running".to_string());
                return;
            }
        };
        let buffer = tab.buffer.clone();
        let file_name = tab.file_name().to_string();

        if let Err(e) = std::fs::write(&path, &buffer) {
            self.notify(format!("Failed to save: {e}"));
            return;
        }
        self.state.editor.tabs[active_index].is_dirty = false;
        self.notify(format!("Running: {file_name}"));

        if let Some(terminal) = self.terminal.as_mut() {
            let cmd = format!("cargo run -q -p rustle-cli -- \"{}\"\n", path.display());
            terminal.process_command(BackendCommand::Write(cmd.into_bytes()));
            self.state.console_visible = true;
        } else {
            self.notify("Terminal not initialized".to_string());
        }
    }

    pub fn drain_events(&mut self) {
        let events = std::mem::take(&mut self.events);
        for event in events {
            handle_app_event(self, event);
        }
    }

    pub fn start_preview(&mut self) {
        self.stop_preview();
        self.runtime_preview_commands = self.static_preview_commands.clone();
        self.runtime_preview_error = None;
        self.console.clear();

        let Some(snapshot) = self.active_preview_snapshot() else {
            self.runtime_preview_error = Some("No file opened".to_string());
            return;
        };

        match spawn_runtime(&snapshot.source) {
            Ok(script) => {
                self.runner.script = Some(script);
                self.runner.running_preview = Some(snapshot);
                self.runner.last_tick = Instant::now();
            }
            Err(error) => {
                self.runtime_preview_error = Some(error);
            }
        }
    }

    pub fn stop_preview(&mut self) {
        stop_runtime(&mut self.runner);
    }

    pub fn tick_preview(&mut self, ctx: &egui::Context) {
        // Cleanup old notifications
        self.notifications.retain(|n| n.created_at.elapsed() < NOTIFICATION_DURATION);

        let mut new_draw = None;
        let mut new_console = Vec::new();
        let mut runtime_error = None;

        let input = build_input_from_egui(ctx);

        let status = tick_runtime(
            &mut self.runner,
            input,
            &mut |frame| {
                new_draw = Some(frame.draw_commands);
                new_console.extend(frame.console_entries);
            },
            &mut |message| {
                runtime_error = Some(message);
            },
        );

        if let Some(draw_commands) = new_draw {
            self.runtime_preview_commands = draw_commands;
            self.runtime_preview_error = None;
        }

        for entry in new_console {
            push_console(&mut self.console, entry);
        }

        if let Some(message) = runtime_error {
            self.runtime_preview_error = Some(message.clone());
            push_console(
                &mut self.console,
                ConsoleEntry {
                    level: ConsoleLevel::Error,
                    message,
                },
            );
        }

        if matches!(status, TickStatus::Running) {
            ctx.request_repaint();
        }
    }

    pub fn sync_preview_state(&mut self) {
        if self.is_running() && self.running_snapshot_invalid() {
            self.stop_preview();
        }

        if !self.is_running() {
            self.refresh_static_preview_if_needed();
        }
    }

    pub fn preview_commands(&self) -> &[DrawCommand] {
        if self.is_running() {
            &self.runtime_preview_commands
        } else {
            &self.static_preview_commands
        }
    }

    pub fn preview_error(&self) -> Option<&str> {
        if self.is_running() {
            self.runtime_preview_error.as_deref()
        } else {
            self.static_preview_error.as_deref()
        }
    }

    pub fn preview_native_size(&self) -> Option<egui::Vec2> {
        self.preview_commands().iter().find_map(|command| match command {
            DrawCommand::DrawShape(data) => Some(egui::vec2(
                data.coord_meta.px_width.max(400.0) as f32,
                data.coord_meta.px_height.max(400.0) as f32,
            )),
            _ => None,
        })
    }

    fn running_snapshot_invalid(&self) -> bool {
        let Some(running) = self.runner.running_preview.as_ref() else {
            return true;
        };
        let Some(active_index) = self.state.editor.active_index() else {
            return true;
        };
        let tab = &self.state.editor.tabs[active_index];
        tab.id != running.tab_id || tab.buffer != running.source
    }

    fn refresh_static_preview_if_needed(&mut self) {
        let Some(active_index) = self.state.editor.active_index() else {
            self.static_preview_commands.clear();
            self.static_preview_error = None;
            self.console.clear();
            self.last_static_preview = None;
            return;
        };

        let tab = &self.state.editor.tabs[active_index];
        if let Some(last_preview) = self.last_static_preview.as_ref() {
            if last_preview.tab_id == tab.id && last_preview.source == tab.buffer {
                return;
            }
        }

        let snapshot = PreviewSnapshot {
            tab_id: tab.id,
            source: tab.buffer.clone(),
        };

        match render_static_preview(&snapshot.source) {
            Ok(frame) => {
                self.static_preview_commands = frame.draw_commands;
                self.static_preview_error = None;
                self.console = frame.console_entries.into_iter().collect();
            }
            Err(error) => {
                self.static_preview_commands.clear();
                self.static_preview_error = Some(error);
                self.console.clear();
            }
        }

        self.last_static_preview = Some(snapshot);
    }

    fn active_preview_snapshot(&self) -> Option<PreviewSnapshot> {
        let active_index = self.state.editor.active_index()?;
        let tab = &self.state.editor.tabs[active_index];
        Some(PreviewSnapshot {
            tab_id: tab.id,
            source: tab.buffer.clone(),
        })
    }
}

fn push_console(console: &mut VecDeque<ConsoleEntry>, entry: ConsoleEntry) {
    if console.len() >= MAX_CONSOLE_ENTRIES {
        console.pop_front();
    }
    console.push_back(entry);
}

fn build_input_from_egui(ctx: &egui::Context) -> Input {
    ctx.input(|i| {
        let (mouse_x, mouse_y) = i.pointer.interact_pos()
            .map(|p| (p.x as f64, p.y as f64))
            .unwrap_or((0.0, 0.0));

        let mouse_down = i.pointer.primary_down();
        let mouse_pressed = i.pointer.primary_pressed();
        let mouse_released = i.pointer.primary_released();

        let mut key_pressed = String::new();
        let mut key_released = String::new();
        for event in &i.events {
            if let egui::Event::Key { key, pressed, .. } = event {
                let name = format!("{key:?}").to_lowercase();
                if *pressed {
                    key_pressed = name;
                } else {
                    key_released = name;
                }
            }
        }

        let key_down = i.keys_down.iter().next()
            .map(|k| format!("{k:?}").to_lowercase())
            .unwrap_or_default();

        Input {
            dt: 0.0, // dt is set by the runner from frame timing
            mouse_x,
            mouse_y,
            mouse_down,
            mouse_pressed,
            mouse_released,
            key_pressed,
            key_down,
            key_released,
        }
    })
}
