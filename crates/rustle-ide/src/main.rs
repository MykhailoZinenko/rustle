#[macro_use]
mod terminal;
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
use renderer::wgpu_renderer::RustleRenderResources;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Rustle IDE",
        options,
        Box::new(|cc| {
            if let Some(render_state) = cc.wgpu_render_state.as_ref() {
                let renderer = rustle_renderer::Renderer::new(
                    &render_state.device,
                    &render_state.queue,
                    render_state.target_format,
                );
                render_state
                    .renderer
                    .write()
                    .callback_resources
                    .insert(RustleRenderResources {
                        renderer,
                        prepared_frame: None,
                    });
            }
            Ok(Box::new(App::default()))
        }),
    )
}
