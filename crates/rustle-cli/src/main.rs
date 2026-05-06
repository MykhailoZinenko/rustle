use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use rustle_lang::{compile, DrawCommand, Input, Runtime};
use rustle_renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::Key;
use winit::window::{Window, WindowId};

struct InputState {
    mouse_x: f64,
    mouse_y: f64,
    mouse_down: bool,
    mouse_pressed: bool,
    mouse_released: bool,
    keys_down: HashSet<String>,
    key_pressed: String,
    key_released: String,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            mouse_pressed: false,
            mouse_released: false,
            keys_down: HashSet::new(),
            key_pressed: String::new(),
            key_released: String::new(),
        }
    }
}

struct RunState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    runtime: Runtime,
    last_frame: Instant,
    input_state: InputState,
}

struct App {
    source: String,
    script_path: String,
    state: Option<RunState>,
    exit_code: i32,
}

impl App {
    fn new(source: String, script_path: String) -> Self {
        Self {
            source,
            script_path,
            state: None,
            exit_code: 0,
        }
    }
}

fn key_name(key: &Key) -> String {
    match key {
        Key::Character(c) => c.to_string(),
        Key::Named(named) => format!("{named:?}"),
        _ => String::new(),
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
                self.exit_code = 1;
                event_loop.exit();
                return;
            }
        };

        let runtime = match Runtime::new(program) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Runtime error: {e}");
                self.exit_code = 1;
                event_loop.exit();
                return;
            }
        };

        // Extract filename for window title
        let filename = std::path::Path::new(&self.script_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| self.script_path.clone());

        // Create window
        let attrs = Window::default_attributes()
            .with_title(format!("Rustle - {filename}"))
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );

        // Init wgpu
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Failed to find a suitable GPU adapter");

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("Failed to create device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first())
            .copied()
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

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
            input_state: InputState::default(),
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
            WindowEvent::ScaleFactorChanged { .. } => {
                let new_size = state.window.inner_size();
                if new_size.width > 0 && new_size.height > 0 {
                    state.config.width = new_size.width;
                    state.config.height = new_size.height;
                    state.surface.configure(&state.device, &state.config);
                    state.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.input_state.mouse_x = position.x;
                state.input_state.mouse_y = position.y;
            }
            WindowEvent::MouseInput { state: btn_state, button: winit::event::MouseButton::Left, .. } => {
                match btn_state {
                    ElementState::Pressed => {
                        state.input_state.mouse_down = true;
                        state.input_state.mouse_pressed = true;
                    }
                    ElementState::Released => {
                        state.input_state.mouse_down = false;
                        state.input_state.mouse_released = true;
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let name = key_name(&event.logical_key);
                if !name.is_empty() {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat {
                                state.input_state.key_pressed = name.clone();
                            }
                            state.input_state.keys_down.insert(name);
                        }
                        ElementState::Released => {
                            state.input_state.key_released = name.clone();
                            state.input_state.keys_down.remove(&name);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(state.last_frame).as_secs_f64();
                let dt = dt.min(1.0 / 30.0);
                state.last_frame = now;

                let key_down = state
                    .input_state
                    .keys_down
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_default();

                let input = Input {
                    dt,
                    mouse_x: state.input_state.mouse_x,
                    mouse_y: state.input_state.mouse_y,
                    mouse_down: state.input_state.mouse_down,
                    mouse_pressed: state.input_state.mouse_pressed,
                    mouse_released: state.input_state.mouse_released,
                    key_pressed: state.input_state.key_pressed.clone(),
                    key_down,
                    key_released: state.input_state.key_released.clone(),
                };

                // Clear per-frame flags after building Input
                state.input_state.mouse_pressed = false;
                state.input_state.mouse_released = false;
                state.input_state.key_pressed.clear();
                state.input_state.key_released.clear();

                let commands: Vec<DrawCommand> = match state.runtime.tick(&input) {
                    Ok(cmds) => cmds,
                    Err(e) => {
                        eprintln!("Runtime error: {e}");
                        let _ = state.runtime.exit();
                        self.exit_code = 1;
                        event_loop.exit();
                        return;
                    }
                };

                let frame = match state.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        state.surface.configure(&state.device, &state.config);
                        state.window.request_redraw();
                        return;
                    }
                    other => {
                        eprintln!("Surface error: {other:?}");
                        self.exit_code = 1;
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

    // Handle --help / -h
    if args.len() > 1 && (args[1] == "--help" || args[1] == "-h") {
        println!("Usage: rustle-cli <script.rustle>");
        println!();
        println!("Run a Rustle script in a window.");
        println!();
        println!("Options:");
        println!("  -h, --help       Print this help message");
        println!("  -V, --version    Print version");
        return;
    }

    // Handle --version / -V
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        println!("rustle-cli {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.len() < 2 {
        eprintln!("Usage: rustle-cli <script.rustle>");
        std::process::exit(1);
    }

    let script_path = args[1].clone();
    let source = match std::fs::read_to_string(&script_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {script_path}: {e}");
            std::process::exit(1);
        }
    };

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new(source, script_path);
    event_loop
        .run_app(&mut app)
        .expect("Failed to run event loop");
    std::process::exit(app.exit_code);
}
