use std::{thread, time::Duration};
use std::sync::Arc;
use winit::{
    application::{ApplicationHandler},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
    event::{WindowEvent},
};
use egui::{
    Context as EguiContext,
    viewport::ViewportId,
    Window as EguiWindow,
};
use egui_winit::{State as EguiWinitState};
use egui_wgpu::{
    Renderer as EguiWgpuRenderer,
    ScreenDescriptor,
    wgpu,
};

enum UserEvent {
    Tick,
}

pub fn run() {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("failed to create event loop");

    let proxy = event_loop.create_proxy();

    let mut app = PlatformApp {
        window: None,
        proxy: Some(proxy),
        ticker_started: false,
        egui_ctx: None,
        egui_state: None,
        instance: None,
        surface: None,
        device: None,
        queue: None,
        surface_config: None,
        egui_renderer: None,
    };
    event_loop.run_app(&mut app).expect("Event loop crashed");
}

struct PlatformApp {
    window: Option<Arc<Window>>,
    proxy: Option<EventLoopProxy<UserEvent>>,
    ticker_started: bool,
    egui_ctx: Option<EguiContext>,
    egui_state: Option<EguiWinitState>,
    instance: Option<wgpu::Instance>,
    surface: Option<wgpu::Surface<'static>>,
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    egui_renderer: Option<EguiWgpuRenderer>,
}

impl ApplicationHandler<UserEvent> for PlatformApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let raw_window = event_loop.create_window(
                Window::default_attributes().with_title("Borrowser")
            ).expect("create window");
            let window = Arc::new(raw_window);
            self.window = Some(window);
        }

        if !self.ticker_started {
            self.ticker_started = true;

            if let Some(proxy) = self.proxy.clone() {
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
        }
        if self.egui_ctx.is_none() || self.egui_state.is_none() {
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

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let window = self.window.as_ref().unwrap();
        let surface = unsafe {
            instance.create_surface(Arc::clone(window))
        }.expect("surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })).expect("no suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::default(),
            },
        )).expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
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
                if let (Some(surface), Some(device), Some(cfg)) =
                    (self.surface.as_ref(), self.device.as_ref(), self.surface_config.as_mut())
                {
                    cfg.width = new_size.width.max(1);
                    cfg.height = new_size.height.max(1);
                    surface.configure(device, cfg);
                }
            }
            WindowEvent::RedrawRequested => {
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
                    Err(wgpu::SurfaceError::Lost) => {
                        // Reconfigure (common after display changes)
                        surface.configure(device, cfg);
                        return;
                    }
                    Err(wgpu::SurfaceError::Outdated) => return, // minimized / moved
                    Err(e) => {
                        eprintln!("surface error: {e:?}");
                        return;
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

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
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("encoder"),
                });

                // Screen descriptor (pixels + scale)
                let screen = ScreenDescriptor {
                    size_in_pixels: [cfg.width, cfg.height],
                    pixels_per_point: ctx.pixels_per_point(),
                };

                egui_renderer.update_buffers(device, queue, &mut encoder, &clipped, &screen);

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui rpass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    egui_renderer.render(&mut rpass.forget_lifetime(), &clipped, &screen);
                }

                // Free textures requested by egui
                for id in full_output.textures_delta.free {
                    egui_renderer.free_texture(&id);
                }

                // 5) Submit & present
                queue.submit(Some(encoder.finish()));
                frame.present();
            }
                      _ => {}
        }
    }
}
