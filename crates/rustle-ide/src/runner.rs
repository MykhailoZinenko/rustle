use std::time::Instant;

use rustle_lang::{DrawCommand, Input, Runtime, compile};

use crate::services::preview_runtime_service::{ScriptHandle, WorkerResult};
use crate::state::editor_state::TabId;

#[derive(Clone)]
pub struct PreviewSnapshot {
    pub tab_id: TabId,
    pub source: String,
}

pub struct PreviewRunnerState {
    pub script: Option<ScriptHandle>,
    pub running_preview: Option<PreviewSnapshot>,
    pub last_tick: Instant,
}

impl Default for PreviewRunnerState {
    fn default() -> Self {
        Self {
            script: None,
            running_preview: None,
            last_tick: Instant::now(),
        }
    }
}

#[derive(Clone)]
pub struct ConsoleEntry {
    pub level: ConsoleLevel,
    pub message: String,
}

#[derive(Clone, Copy)]
pub enum ConsoleLevel {
    Log,
    Warn,
    Error,
}

pub struct PreviewFrame {
    pub draw_commands: Vec<DrawCommand>,
    pub console_entries: Vec<ConsoleEntry>,
}

pub enum TickStatus {
    Running,
    Stopped,
}

pub fn spawn_runtime(source: &str) -> Result<ScriptHandle, String> {
    let program = compile(source).map_err(join_compile_errors)?;
    Ok(ScriptHandle::spawn(program))
}

pub fn render_static_preview(source: &str) -> Result<PreviewFrame, String> {
    let program = compile(source).map_err(join_compile_errors)?;
    let mut runtime = Runtime::new(program).map_err(|error| error.message)?;
    let commands = runtime.tick(&Input { dt: 0.0 }).map_err(|error| error.message)?;
    Ok(split_draw_commands(commands))
}

pub fn stop_runtime(runner: &mut PreviewRunnerState) {
    if let Some(mut script) = runner.script.take() {
        script.stop();
    }
    runner.running_preview = None;
}

pub fn tick_runtime(
    runner: &mut PreviewRunnerState,
    on_frame: &mut dyn FnMut(PreviewFrame),
    on_error: &mut dyn FnMut(String),
) -> TickStatus {
    let Some(script) = runner.script.as_ref() else {
        return TickStatus::Stopped;
    };

    let now = Instant::now();
    let dt = now.duration_since(runner.last_tick).as_secs_f64();
    runner.last_tick = now;
    script.tick(dt);

    let mut should_stop = false;
    while let Ok(result) = script.rx.try_recv() {
        match result {
            WorkerResult::Frame(commands) => on_frame(split_draw_commands(commands)),
            WorkerResult::Error(message) => {
                if message != "script cancelled" {
                    on_error(message);
                }
                should_stop = true;
            }
            WorkerResult::InitError(message) => {
                on_error(message);
                should_stop = true;
            }
            WorkerResult::Exited => should_stop = true,
        }
    }

    if should_stop {
        stop_runtime(runner);
        TickStatus::Stopped
    } else {
        TickStatus::Running
    }
}

fn split_draw_commands(commands: Vec<DrawCommand>) -> PreviewFrame {
    let mut draw_commands = Vec::new();
    let mut console_entries = Vec::new();

    for command in commands {
        match command {
            DrawCommand::DrawShape(data) => draw_commands.push(DrawCommand::DrawShape(data)),
            DrawCommand::Print(message) => console_entries.push(ConsoleEntry {
                level: ConsoleLevel::Log,
                message,
            }),
            DrawCommand::Warn(message) => console_entries.push(ConsoleEntry {
                level: ConsoleLevel::Warn,
                message,
            }),
            DrawCommand::Error(message) => console_entries.push(ConsoleEntry {
                level: ConsoleLevel::Error,
                message,
            }),
            _ => {}
        }
    }

    PreviewFrame {
        draw_commands,
        console_entries,
    }
}

fn join_compile_errors(errors: Vec<impl ToString>) -> String {
    errors
        .into_iter()
        .map(|error| error.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
