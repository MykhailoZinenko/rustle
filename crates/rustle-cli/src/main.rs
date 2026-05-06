use std::sync::Arc;
use std::time::Instant;

use rustle_lang::{compile, DrawCommand, Input, Runtime};
use rustle_renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct RunState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    runtime: Runtime,
    last_frame: Instant,
}

struct App {
    source: String,
    state: Option<RunState>,
}

impl App {
    fn new(source: String) -> Self {
        Self {
            source,
            state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        // Compile script
        let program = match compile(&self.source) {
            Ok(p) => p,
            Err(errors) => {
                for e in &errors {
                    eprintln!("{e}");
                }
                event_loop.exit();
                return;
            }
        };

        let runtime = match Runtime::new(program) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Runtime error: {e}");
                event_loop.exit();
                return;
            }
        };

        // Create window
        let attrs = Window::default_attributes()
            .with_title("Rustle")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());

        // Init wgpu
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor::default()),
        )
        .expect("Failed to create device");

        let size = window.inner_size();
        let format = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(|f| !f.is_srgb())
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let renderer = Renderer::new(&device, format);

        self.state = Some(RunState {
            window,
            surface,
            device,
            queue,
            config,
            renderer,
            runtime,
            last_frame: Instant::now(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                let _ = state.runtime.exit();
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.config.width = new_size.width;
                    state.config.height = new_size.height;
                    state.surface.configure(&state.device, &state.config);
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame).as_secs_f64();
                state.last_frame = now;

                let input = Input {
                    dt,
                    ..Default::default()
                };

                let commands: Vec<DrawCommand> = match state.runtime.tick(&input) {
                    Ok(cmds) => cmds,
                    Err(e) => {
                        eprintln!("Runtime error: {e}");
                        event_loop.exit();
                        return;
                    }
                };

                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated
                    | wgpu::CurrentSurfaceTexture::Lost => {
                        state.surface.configure(&state.device, &state.config);
                        state.window.request_redraw();
                        return;
                    }
                    other => {
                        eprintln!("Surface error: {other:?}");
                        event_loop.exit();
                        return;
                    }
                };

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                state.renderer.render(
                    &commands,
                    &state.device,
                    &state.queue,
                    &view,
                    state.config.width,
                    state.config.height,
                );

                frame.present();
                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: rustle-cli <script.rustle>");
        std::process::exit(1);
    }

    let source = match std::fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {e}", args[1]);
            std::process::exit(1);
        }
    };

    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(source);
    event_loop.run_app(&mut app).unwrap();
}
