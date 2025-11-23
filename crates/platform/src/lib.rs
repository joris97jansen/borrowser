use std::{thread, time::Duration};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use winit::{
    application::{ApplicationHandler},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId, Theme},
    event::{WindowEvent},
    dpi::PhysicalSize,
};
use egui::Visuals;
use std::sync::mpsc;
use gfx::Renderer;
use app_api::{
    UiApp,
    Repaint,
    RepaintHandle
};
use bus::{
    CoreEvent,
    CoreCommand,
};
use runtime_net::start_net_runtime;
use runtime_parse::start_parse_runtime;
use runtime_css::start_css_runtime;

pub enum UserEvent {
    Core(CoreEvent),
    Repaint,
}


pub fn run_with<A: UiApp + 'static>(app: A) {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();

    let mut platform = PlatformApp::new(proxy);
    platform.app = Some(Box::new(app));           // <- inject the app

    event_loop.run_app(&mut platform).expect("crashed");
}

fn start_bus_bridge(proxy: EventLoopProxy<UserEvent>, evt_rx: std::sync::mpsc::Receiver<bus::CoreEvent>) {
    thread::spawn(move || {
        while let Ok(evt) = evt_rx.recv() {
            let _ = proxy.send_event(UserEvent::Core(evt));
        }
    });
}

fn router_thread(cmd_rx_main: mpsc::Receiver<CoreCommand>,
                 net_tx: mpsc::Sender<CoreCommand>,
                 parse_tx: mpsc::Sender<CoreCommand>,
                 css_tx: mpsc::Sender<CoreCommand>) {
    thread::spawn(move || {
        while let Ok(cmd) = cmd_rx_main.recv() {
            match cmd {
                // Networking goes to net runtime
                CoreCommand::FetchStream { .. } |
                CoreCommand::CancelRequest { .. } => {
                    let _ = net_tx.send(cmd);
                }

                // HTML parsing commands go to parse runtime
                CoreCommand::ParseHtmlStart { .. } |
                CoreCommand::ParseHtmlChunk { .. } |
                CoreCommand::ParseHtmlDone  { .. } => {
                    let _ = parse_tx.send(cmd);
                }

                // CSS streaming→parsing goes to css runtime
                CoreCommand::CssChunk { .. } |
                CoreCommand::CssDone  { .. } => {
                    let _ = css_tx.send(cmd);
                }
            }
        }
    });
}

struct PlatformApp {
    window: Option<Arc<Window>>,
    proxy: EventLoopProxy<UserEvent>,
    renderer: Option<Renderer>,
    repaint: Option<Arc<PlatformRepaint>>,
    app: Option<Box<dyn UiApp>>,
}

impl PlatformApp {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            proxy: proxy,
            renderer: None,
            repaint: None,
            app: None,
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

    fn init_renderer(&mut self) {
        if self.renderer.is_some() {
            return;
        }
        let window = self.window.as_ref().unwrap();
        let renderer = Renderer::new(window.as_ref());
        self.renderer = Some(renderer);
    }

    fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(new_size);
        }
    }

    fn draw_frame(&mut self) {
        let window   = self.window.as_ref().unwrap();
        let renderer = self.renderer.as_mut().unwrap();
        let app      = self.app.as_mut().expect("UiApp not injected");

        // ---- 1) Sync egui visuals with OS theme ----
        if let Some(theme) = window.theme() {
            let ctx = renderer.context();
            let visuals = ctx.style().visuals.clone();

            let want_dark = matches!(theme, Theme::Dark);
            let is_current_dark = visuals.dark_mode;

            if want_dark != is_current_dark {
                if want_dark {
                    ctx.set_visuals(Visuals::dark());
                } else {
                    ctx.set_visuals(Visuals::light());
                }
            }
        }

        // ---- 2) Normal rendering ----
        renderer.render(window.as_ref(), |ctx| app.ui(ctx));
    }
}

impl ApplicationHandler<UserEvent> for PlatformApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // --- window & renderer boot ---
        self.init_window(event_loop);
        self.init_renderer();

        // --- create Bus channels (one cmd in, one evt out) ---
        let (cmd_tx_main, cmd_rx_main) = mpsc::channel::<CoreCommand>();
        let (evt_tx_main, evt_rx_main) = mpsc::channel::<CoreEvent>();

        // --- per-runtime command channels ---
        let (net_cmd_tx,  net_cmd_rx)  = mpsc::channel::<CoreCommand>();
        let (par_cmd_tx,  par_cmd_rx)  = mpsc::channel::<CoreCommand>();
        let (css_cmd_tx,  css_cmd_rx)  = mpsc::channel::<CoreCommand>();

        // --- start runtimes (each gets its cmd_rx + shared evt_tx) ---
        start_net_runtime(net_cmd_rx, evt_tx_main.clone());
        start_parse_runtime(par_cmd_rx, evt_tx_main.clone());
        start_css_runtime(css_cmd_rx, evt_tx_main.clone());

        // --- route CoreCommand → proper runtime ---
        router_thread(cmd_rx_main, net_cmd_tx.clone(), par_cmd_tx.clone(), css_cmd_tx.clone());

        // --- bridge CoreEvent → winit user events ---
        start_bus_bridge(self.proxy.clone(), evt_rx_main);

        // --- install repaint handle (unchanged) ---
        let repaint = Arc::new(PlatformRepaint::new(self.proxy.clone()));
        self.repaint = Some(repaint.clone());
        if let Some(app) = self.app.as_mut() {
            let handle: RepaintHandle = repaint;
            app.set_repaint_handle(handle);

            // give the BrowserApp the CoreCommand sender so it can drive the system
            app.set_bus_sender(cmd_tx_main.clone());
        }

        // --- first frame ---
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
           UserEvent::Core(core_evt) => {
                if let Some(app) = self.app.as_mut() {
                    // new single entry-point:
                    app.on_core_event(core_evt);
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            UserEvent::Repaint => {
                if let Some(repaint) = self.repaint.as_ref() {
                    repaint.clear_pending();
                }
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
        if let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) {
            renderer.on_window_event(window.as_ref(), &event);
        }
        match event {
            WindowEvent::ThemeChanged(_theme) => {
                // Don’t set visuals here anymore, just redraw.
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                self.on_resize(new_size);
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::Touch { .. } => {
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                self.draw_frame();
            }
            _ => {}
        }
    }
}

pub struct PlatformRepaint {
    pending: Arc<AtomicBool>,
    proxy: EventLoopProxy<UserEvent>,
}

impl PlatformRepaint {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            pending: Arc::new(AtomicBool::new(false)),
            proxy,
        }
    }

    pub fn clear_pending(&self) {
        self.pending.store(false, Ordering::Release);
    }
}

impl Repaint for PlatformRepaint {
    fn request_now(&self) {
        if !self.pending.swap(true, Ordering::AcqRel) {
            let _ = self.proxy.send_event(UserEvent::Repaint);
        }
    }

    fn request_after(&self, duration: Duration) {
        let pending = self.pending.clone();
        let proxy = self.proxy.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            if !pending.swap(true, Ordering::AcqRel) {
                let _ = proxy.send_event(UserEvent::Repaint);
            }
        });
    }
}
