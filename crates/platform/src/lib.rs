use std::{thread, time::Duration};
use std::sync::Arc;
use winit::{
    application::{ApplicationHandler},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
    event::{WindowEvent},
    dpi::PhysicalSize,
};
use egui::{
    Context as EguiContext,
    viewport::ViewportId,
};
use egui_winit::{State as EguiWinitState};
use egui_wgpu::{
    Renderer as EguiWgpuRenderer,
    ScreenDescriptor,
    wgpu::{
        Instance, Surface, Device, Queue, SurfaceConfiguration,
        InstanceDescriptor, RequestAdapterOptions, PowerPreference,
        Features, Limits, MemoryHints, Trace,
        DeviceDescriptor, TextureUsages,
        PresentMode, SurfaceError, TextureViewDescriptor,
        CommandEncoderDescriptor, RenderPassDescriptor, RenderPassColorAttachment,
        Operations, LoadOp, StoreOp, Color,
    },
};

enum UserEvent {
    Tick,
}

pub fn run() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("failed to create event loop");

    let proxy = event_loop.create_proxy();
    let mut app = PlatformApp::new(proxy);

    event_loop.run_app(&mut app).expect("Event loop crashed");
}

struct PlatformApp {
    window: Option<Arc<Window>>,
    proxy: EventLoopProxy<UserEvent>,
    ticker_started: bool,
    egui_ctx: Option<EguiContext>,
    egui_state: Option<EguiWinitState>,
    instance: Option<Instance>,
    surface: Option<Surface<'static>>,
    device: Option<Device>,
    queue: Option<Queue>,
    surface_config: Option<SurfaceConfiguration>,
    egui_renderer: Option<EguiWgpuRenderer>,
}

impl PlatformApp {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            proxy: proxy,
            ticker_started: false,
            egui_ctx: None,
            egui_state: None,
            instance: None,
            surface: None,
            device: None,
            queue: None,
            surface_config: None,
            egui_renderer: None,
        }
    }

    fn init_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let raw_window = event_loop.create_window(
            Window::default_attributes().with_title("Borrowser")
        ).expect("create window");
        let window = Arc::new(raw_window);
        self.window = Some(window);
    }

    fn init_ticker(&mut self) {
        if self.ticker_started {
            return;
        }
        self.ticker_started = true;

        let proxy = self.proxy.clone();
        thread::spawn(move || {
            let frame = Duration::from_millis(16); // ~60Hz
            loop {
                if proxy.send_event(UserEvent::Tick).is_err() {
                    break;
                }
                thread::sleep(frame);
            }
        });
    }

    fn init_egui(&mut self) {
        if self.egui_ctx.is_some() && self.egui_state.is_some() {
            return;
        }
        let window = self.window.as_ref().unwrap();
        let ctx = EguiContext::default();

        let state = EguiWinitState::new(
            ctx.clone(),
            ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        self.egui_ctx = Some(ctx);
        self.egui_state = Some(state);
    }

    fn init_gpu(&mut self) {
        if self.instance.is_some()
            && self.surface.is_some()
            && self.device.is_some()
            && self.queue.is_some()
            && self.surface_config.is_some()
            && self.egui_renderer.is_some()
        {
            return;
        }
        let instance = Instance::new(&InstanceDescriptor::default());
        let window = self.window.as_ref().unwrap();
        let surface = instance.create_surface(Arc::clone(window)).expect("surface");

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })).expect("no suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
                &DeviceDescriptor {
                    label: Some("device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::Performance,
                    trace: Trace::default(),
            },
        )).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
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

        self.instance = Some(instance);
        self.surface = Some(surface);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface_config = Some(config);
        self.egui_renderer = Some(egui_renderer);
    }

    fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        if let (Some(surface), Some(device), Some(cfg)) =
            (self.surface.as_ref(), self.device.as_ref(), self.surface_config.as_mut())
        {
            cfg.width = new_size.width.max(1);
            cfg.height = new_size.height.max(1);
            surface.configure(device, cfg);
        }
    }

    fn draw_frame(&mut self) {
        let (ctx, state) = (self.egui_ctx.as_ref().unwrap(), self.egui_state.as_mut().unwrap());
        let window = self.window.as_ref().unwrap();
        let surface = self.surface.as_ref().unwrap();
        let device = self.device.as_ref().unwrap();
        let queue = self.queue.as_ref().unwrap();
        let cfg = self.surface_config.as_ref().unwrap();
        let egui_renderer = self.egui_renderer.as_mut().unwrap();

        // 1) Acquire frame
        let frame = match surface.get_current_texture() {
            Ok(x) => x,
            Err(SurfaceError::Lost) => {
                // Reconfigure (common after display changes)
                surface.configure(device, cfg);
                return;
            }
            Err(SurfaceError::Outdated) => return, // minimized / moved
            Err(e) => {
                eprintln!("surface error: {e:?}");
                return;
            }
        };
        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        // 2) Egui frame (build UI)
        let raw_input = state.take_egui_input(window);
        ctx.begin_pass(raw_input);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.label("Borrowser — egui + wgpu ✅");
        });

        egui::Window::new("Demo").show(ctx, |ui| {
            ui.label("We are actually rendering now.");
            if ui.button("Click me").clicked() {
                println!("clicked!");
            }
        });

        let full_output = ctx.end_pass();
        state.handle_platform_output(window, full_output.platform_output);

        // 3) Tessellate
        let shapes = full_output.shapes;
        let clipped = ctx.tessellate(shapes, ctx.pixels_per_point());

        // Upload textures
        for (id, delta) in &full_output.textures_delta.set {
            egui_renderer.update_texture(device, queue, *id, delta);
        }

        // 4) Encode draw
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("encoder"),
        });

        // Screen descriptor (pixels + scale)
        let screen = ScreenDescriptor {
            size_in_pixels: [cfg.width, cfg.height],
            pixels_per_point: ctx.pixels_per_point(),
        };

        egui_renderer.update_buffers(device, queue, &mut encoder, &clipped, &screen);

        {
            let render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("egui render_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
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

            egui_renderer.render(&mut render_pass.forget_lifetime(), &clipped, &screen);
        }

        // Free textures requested by egui
        for id in full_output.textures_delta.free {
            egui_renderer.free_texture(&id);
        }

        // 5) Submit & present
        queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

impl ApplicationHandler<UserEvent> for PlatformApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.init_window(event_loop);
        self.init_ticker();
        self.init_egui();
        self.init_gpu();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tick => {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        if let (Some(window), Some(state)) = (self.window.as_ref(), self.egui_state.as_mut()) {
            let _response = state.on_window_event(window, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.on_resize(new_size);
            }
            WindowEvent::RedrawRequested => {
                self.draw_frame();
            }
                      _ => {}
        }
    }
}
