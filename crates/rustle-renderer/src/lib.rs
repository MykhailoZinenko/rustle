#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

mod instance;
mod pipeline;
pub(crate) mod prepare;

use instance::{LineVertex, PolygonVertex, SdfInstance};
use rustle_lang::DrawCommand;
use wgpu::util::DeviceExt;

pub use prepare::PreparedFrame;

const INITIAL_SDF_CAPACITY: usize = 256;
const INITIAL_LINE_VERTEX_CAPACITY: usize = 1024;
const INITIAL_LINE_INDEX_CAPACITY: usize = 2048;
const INITIAL_POLY_VERTEX_CAPACITY: usize = 1024;
const INITIAL_POLY_INDEX_CAPACITY: usize = 2048;

pub struct Renderer {
    sdf_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    polygon_pipeline: wgpu::RenderPipeline,
    #[expect(dead_code, reason = "retained for potential future pipeline recreation")]
    viewport_bind_group_layout: wgpu::BindGroupLayout,
    viewport_buffer: wgpu::Buffer,
    viewport_bind_group: wgpu::BindGroup,

    sdf_instance_buffer: wgpu::Buffer,
    sdf_instance_capacity: usize,

    line_vertex_buffer: wgpu::Buffer,
    line_vertex_capacity: usize,
    line_index_buffer: wgpu::Buffer,
    line_index_capacity: usize,

    polygon_vertex_buffer: wgpu::Buffer,
    polygon_vertex_capacity: usize,
    polygon_index_buffer: wgpu::Buffer,
    polygon_index_capacity: usize,
}

impl Renderer {
    #[must_use]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let viewport_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("viewport_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("viewport_uniform"),
            contents: bytemuck::cast_slice(&[1.0_f32, 1.0_f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let viewport_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport_bind_group"),
            layout: &viewport_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: viewport_buffer.as_entire_binding(),
            }],
        });

        let sdf_pipeline =
            pipeline::sdf::create_pipeline(device, format, &viewport_bind_group_layout);
        let line_pipeline = pipeline::line::create_pipeline(device, format);
        let polygon_pipeline = pipeline::polygon::create_pipeline(device, format);

        let sdf_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sdf_instance_buffer"),
            size: (INITIAL_SDF_CAPACITY * std::mem::size_of::<SdfInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let line_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_vertex_buffer"),
            size: (INITIAL_LINE_VERTEX_CAPACITY * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let line_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("line_index_buffer"),
            size: (INITIAL_LINE_INDEX_CAPACITY * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let polygon_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("polygon_vertex_buffer"),
            size: (INITIAL_POLY_VERTEX_CAPACITY * std::mem::size_of::<PolygonVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let polygon_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("polygon_index_buffer"),
            size: (INITIAL_POLY_INDEX_CAPACITY * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            sdf_pipeline,
            line_pipeline,
            polygon_pipeline,
            viewport_bind_group_layout,
            viewport_buffer,
            viewport_bind_group,
            sdf_instance_buffer,
            sdf_instance_capacity: INITIAL_SDF_CAPACITY,
            line_vertex_buffer,
            line_vertex_capacity: INITIAL_LINE_VERTEX_CAPACITY,
            line_index_buffer,
            line_index_capacity: INITIAL_LINE_INDEX_CAPACITY,
            polygon_vertex_buffer,
            polygon_vertex_capacity: INITIAL_POLY_VERTEX_CAPACITY,
            polygon_index_buffer,
            polygon_index_capacity: INITIAL_POLY_INDEX_CAPACITY,
        }
    }

    /// Full render: creates encoder, begins render pass, draws, submits.
    pub fn render(
        &mut self,
        commands: &[DrawCommand],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let frame = self.prepare(commands, device, queue, width, height);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rustle_render_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rustle_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            self.render_to_pass(&mut pass, &frame);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Prepare GPU buffers from draw commands. Call before `render_to_pass`.
    pub fn prepare(
        &mut self,
        commands: &[DrawCommand],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> PreparedFrame {
        queue.write_buffer(
            &self.viewport_buffer,
            0,
            bytemuck::cast_slice(&[width as f32, height as f32]),
        );

        let frame = prepare::prepare(commands);

        if !frame.sdf_instances.is_empty() {
            grow_buffer(device, &mut self.sdf_instance_buffer, &mut self.sdf_instance_capacity,
                frame.sdf_instances.len(), std::mem::size_of::<SdfInstance>(),
                "sdf_instance_buffer", wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
            queue.write_buffer(&self.sdf_instance_buffer, 0,
                bytemuck::cast_slice(&frame.sdf_instances));
        }

        if !frame.line_vertices.is_empty() {
            grow_buffer(device, &mut self.line_vertex_buffer, &mut self.line_vertex_capacity,
                frame.line_vertices.len(), std::mem::size_of::<LineVertex>(),
                "line_vertex_buffer", wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
            queue.write_buffer(&self.line_vertex_buffer, 0,
                bytemuck::cast_slice(&frame.line_vertices));
        }
        if !frame.line_indices.is_empty() {
            grow_buffer(device, &mut self.line_index_buffer, &mut self.line_index_capacity,
                frame.line_indices.len(), std::mem::size_of::<u32>(),
                "line_index_buffer", wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
            queue.write_buffer(&self.line_index_buffer, 0,
                bytemuck::cast_slice(&frame.line_indices));
        }

        if !frame.polygon_vertices.is_empty() {
            grow_buffer(device, &mut self.polygon_vertex_buffer, &mut self.polygon_vertex_capacity,
                frame.polygon_vertices.len(), std::mem::size_of::<PolygonVertex>(),
                "polygon_vertex_buffer", wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
            queue.write_buffer(&self.polygon_vertex_buffer, 0,
                bytemuck::cast_slice(&frame.polygon_vertices));
        }
        if !frame.polygon_indices.is_empty() {
            grow_buffer(device, &mut self.polygon_index_buffer, &mut self.polygon_index_capacity,
                frame.polygon_indices.len(), std::mem::size_of::<u32>(),
                "polygon_index_buffer", wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
            queue.write_buffer(&self.polygon_index_buffer, 0,
                bytemuck::cast_slice(&frame.polygon_indices));
        }

        frame
    }

    /// Record draw calls into an existing render pass (for egui integration).
    pub fn render_to_pass<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, frame: &PreparedFrame) {
        // 1. Filled polygons (behind everything)
        if !frame.polygon_indices.is_empty() {
            pass.set_pipeline(&self.polygon_pipeline);
            pass.set_vertex_buffer(0, self.polygon_vertex_buffer.slice(..));
            pass.set_index_buffer(self.polygon_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..frame.polygon_indices.len() as u32, 0, 0..1);
        }

        // 2. SDF shapes (circles, rects)
        if !frame.sdf_instances.is_empty() {
            pass.set_pipeline(&self.sdf_pipeline);
            pass.set_bind_group(0, &self.viewport_bind_group, &[]);
            pass.set_vertex_buffer(0, self.sdf_instance_buffer.slice(..));
            pass.draw(0..6, 0..frame.sdf_instances.len() as u32);
        }

        // 3. Lines and polygon outlines
        if !frame.line_indices.is_empty() {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_vertex_buffer(0, self.line_vertex_buffer.slice(..));
            pass.set_index_buffer(self.line_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..frame.line_indices.len() as u32, 0, 0..1);
        }
    }

}

fn grow_buffer(
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    capacity: &mut usize,
    needed: usize,
    element_size: usize,
    label: &str,
    usage: wgpu::BufferUsages,
) {
    if needed <= *capacity {
        return;
    }
    let mut cap = *capacity;
    while cap < needed {
        cap *= 2;
    }
    *buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (cap * element_size) as u64,
        usage,
        mapped_at_creation: false,
    });
    *capacity = cap;
}
