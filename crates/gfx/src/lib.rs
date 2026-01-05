use egui::{Context as EguiContext, viewport::ViewportId};
use egui_wgpu::{
    Renderer as EguiWgpuRenderer, ScreenDescriptor,
    wgpu::{
        Color, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, Instance,
        InstanceDescriptor, Limits, LoadOp, MemoryHints, Operations, PowerPreference, PresentMode,
        Queue, RenderPassColorAttachment, RenderPassDescriptor, RequestAdapterOptions, StoreOp,
        Surface, SurfaceConfiguration, SurfaceError, TextureUsages, TextureViewDescriptor, Trace,
    },
};
use egui_winit::State as EguiWinitState;
use std::mem;
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

pub mod text_measurer;
pub use text_measurer::EguiTextMeasurer;
pub mod dom;
pub mod input;
pub mod paint;
pub mod textarea;
pub(crate) mod text_control;
pub mod ui;
pub mod viewport;
pub struct Renderer {
    egui_context: EguiContext,
    egui_state: EguiWinitState,
    egui_renderer: EguiWgpuRenderer,
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    surface_config: SurfaceConfiguration,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let egui_context = EguiContext::default();

        let egui_state = EguiWinitState::new(
            egui_context.clone(),
            ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let instance = Instance::new(&InstanceDescriptor::default());

        let surface = instance.create_surface(window).expect("surface");
        let surface: Surface<'static> =
            unsafe { mem::transmute::<Surface<'_>, Surface<'static>>(surface) };

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .expect("no suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("device"),
            required_features: Features::empty(),
            required_limits: Limits::default(),
            memory_hints: MemoryHints::Performance,
            trace: Trace::default(),
        }))
        .expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 0,
        };
        surface.configure(&device, &config);

        let egui_renderer = EguiWgpuRenderer::new(&device, format, None, 1, true);

        Self {
            egui_context,
            egui_state,
            egui_renderer,
            surface,
            device,
            queue,
            surface_config: config,
        }
    }

    pub fn context(&self) -> &EguiContext {
        &self.egui_context
    }

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.egui_state.on_window_event(window, event);
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn render<F: FnOnce(&EguiContext)>(&mut self, window: &Window, build_ui: F) {
        let surface_texture = match self.surface.get_current_texture() {
            Ok(x) => x,
            Err(SurfaceError::Lost) => {
                // Reconfigure (common after display changes)
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(SurfaceError::Outdated) => return, // minimized / moved
            Err(e) => {
                eprintln!("surface error: {e:?}");
                return;
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        let raw_input = self.egui_state.take_egui_input(window);
        self.egui_context.begin_pass(raw_input);

        build_ui(&self.egui_context);

        let full_output = self.egui_context.end_pass();
        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        // Tessellate
        let clipped = self
            .egui_context
            .tessellate(full_output.shapes, self.egui_context.pixels_per_point());

        // Upload textures
        for (id, delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        // 4) Encode draw
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("gfx encoder"),
            });

        // Screen descriptor (pixels + scale)
        let screen = ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.egui_context.pixels_per_point(),
        };

        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &clipped,
            &screen,
        );

        {
            let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.egui_renderer
                .render(&mut render_pass.forget_lifetime(), &clipped, &screen);
        }

        // Free textures requested by egui
        for id in full_output.textures_delta.free {
            self.egui_renderer.free_texture(&id);
        }

        // 5) Submit & present
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }
}
