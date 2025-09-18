use egui::Context;
use egui_winit::State;
use egui_wgpu::{
    Renderer
};
use egui::viewport::ViewportId;
use wgpu::{
    Queue, CommandEncoder, TextureView, RenderPassColorAttachment, RenderPassDescriptor, StoreOp, LoadOp, Operations, TextureFormat, Device
};
use winit::window::Window;
use winit::event::WindowEvent;

pub struct EguiHost {
    pub state: State,
    pub painter: egui_wgpu::Renderer,
    pub frame_started: bool,
}

impl EguiHost {
    pub fn new(device: &Device, format: TextureFormat, window: Window) -> Self {
        let context = Context::default();
        let state = State::new(
            context,
            ViewportId::Root,
            window,
            Some(window.scale_factor() as f32),
            None,
            Some(2048),
        );

        let painter = egui_wgpu::Renderer::new(
            device,
            format,
            None,
            1,
            true,
        );

        Self{
            state,
            painter,
            frame_started: false,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.state.on_window_event(window, event);
    }

    pub fn begin(&mut self, window: &Window) {
        let raw = self.state.take_egui_input(window);
        self.state.egui_ctx().begin_pass(raw);
        self.frame_started = true;
    }

    pub fn end_and_paint(
        &mut self,
        device: &wgpu::Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        view: &TextureView,
        screen: egui_wgpu::Renderer::ScreenDescriptor,
    ) {
        assert!(self.frame_started, "begin() first");
        self.state.egui_ctx().set_pixels_per_point(screen.pixels_per_point);

        let output = self.state.egui_ctx().end_pass();
        self.state.handle_platform_output(
            output.shapes,
            self.state.egui_ctx().pixels_per_point(),
        );

        let jobs = self.state.egui_ctx().tessellate(output.shapes, self.state.egui_ctx().pixels_per_point());

        for (id, delta) in &output.textures_delta.set {
            self.painter.update_texture(device, queue, *id, delta);
        }
        self.painter.update_buffers(
            device,
            queue,
            encoder,
            &jobs,
            &screen,
        );

        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("egui pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.painter.render(&mut rpass, &jobs, &screen);
        drop(rpass);

        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }
        self.frame_started = false;
    }
}
