use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use rustle_lang::{DrawCommand, Input, Program, Runtime};

pub enum WorkerCmd {
    Tick(Input),
    Exit,
}

pub enum WorkerResult {
    Frame(Vec<DrawCommand>),
    Error(String),
    InitError(String),
    Exited,
}

pub struct ScriptHandle {
    cancel: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerCmd>,
    pub rx: mpsc::Receiver<WorkerResult>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl ScriptHandle {
    pub fn spawn(program: Program) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx_cmd, rx_cmd) = mpsc::channel::<WorkerCmd>();
        let (tx_res, rx_res) = mpsc::channel::<WorkerResult>();
        let cancel_clone = cancel.clone();

        let thread = std::thread::spawn(move || {
            let mut runtime = match Runtime::new_cancellable(program, cancel_clone) {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = tx_res.send(WorkerResult::InitError(error.message));
                    return;
                }
            };

            while let Ok(command) = rx_cmd.recv() {
                match command {
                    WorkerCmd::Tick(input) => match runtime.tick(&input) {
                        Ok(commands) => {
                            let _ = tx_res.send(WorkerResult::Frame(commands));
                        }
                        Err(error) => {
                            let _ = tx_res.send(WorkerResult::Error(error.message));
                            return;
                        }
                    },
                    WorkerCmd::Exit => {
                        let _ = runtime.exit();
                        let _ = tx_res.send(WorkerResult::Exited);
                        return;
                    }
                }
            }
        });

        Self {
            cancel,
            tx: tx_cmd,
            rx: rx_res,
            thread: Some(thread),
        }
    }

    pub fn tick(&self, dt: f64) {
        let _ = self.tx.send(WorkerCmd::Tick(Input { dt }));
    }

    pub fn stop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.tx.send(WorkerCmd::Exit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for ScriptHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
