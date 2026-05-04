mod app;
mod app_core;
mod core;
mod events;
mod renderer;
mod runner;
mod services;
mod state;
mod theme;
mod ui;

use app::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();

    eframe::run_native(
        "Mini IDE",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}
