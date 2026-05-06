use egui_wgpu::CallbackResources;
use eframe::epaint::PaintCallbackInfo;
use rustle_lang::DrawCommand;
use rustle_renderer::{PreparedFrame, Renderer};

/// Stored in `egui_wgpu::CallbackResources` (a type-map).
/// Holds the persistent GPU renderer and the per-frame prepared data.
pub struct RustleRenderResources {
    pub renderer: Renderer,
    pub prepared_frame: Option<PreparedFrame>,
}

/// Paint callback added to the egui paint list each frame.
/// Carries the draw commands and viewport dimensions for one frame.
pub struct RustlePaintCallback {
    pub commands: Vec<DrawCommand>,
    pub width: u32,
    pub height: u32,
}

impl egui_wgpu::CallbackTrait for RustlePaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = callback_resources.get_mut::<RustleRenderResources>().unwrap();
        let frame = resources
            .renderer
            .prepare(&self.commands, device, queue, self.width, self.height);
        resources.prepared_frame = Some(frame);
        Vec::new()
    }

    fn paint(
        &self,
        _info: PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &CallbackResources,
    ) {
        let resources = callback_resources.get::<RustleRenderResources>().unwrap();
        if let Some(frame) = &resources.prepared_frame {
            // SAFETY: The renderer lives in CallbackResources which is owned by the egui
            // Renderer and persists for the entire application lifetime. The render pass
            // borrows GPU buffers from the renderer only for the duration of this paint
            // call. The resources reference is guaranteed valid for the duration of paint().
            let renderer: &'static Renderer =
                unsafe { &*(&resources.renderer as *const Renderer) };
            renderer.render_to_pass(render_pass, frame);
        }
    }
}
