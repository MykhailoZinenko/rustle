use std::collections::VecDeque;
use std::time::Instant;

use eframe::egui;
use rustle_lang::DrawCommand;

use crate::core::app_event_core::handle_app_event;
use crate::events::app_events::AppEvent;
use crate::runner::{
    ConsoleEntry, ConsoleLevel, PreviewRunnerState, PreviewSnapshot, TickStatus,
    render_static_preview, spawn_runtime, stop_runtime, tick_runtime,
};
use crate::state::app_state::AppState;

const MAX_CONSOLE_ENTRIES: usize = 1000;

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
        let mut new_draw = None;
        let mut new_console = Vec::new();
        let mut runtime_error = None;

        let status = tick_runtime(
            &mut self.runner,
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
