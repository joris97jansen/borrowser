use std::{thread, time::Duration};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use winit::{
    application::{ApplicationHandler},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
    event::{WindowEvent},
    dpi::PhysicalSize,
};
use gfx::Renderer;
use app_api::{
    UiApp,
    Repaint,
    RepaintHandle,
    NetStreamCallback,
};
use net::{
    FetchResult,
    NetEvent,
};

enum UserEvent {
    NetResult(FetchResult),
    NetStream(NetEvent),
    Repaint,
}


pub fn run_with<A: UiApp + 'static>(mut app: A) {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let proxy = event_loop.create_proxy();

    let net_callback = Arc::new({
        let proxy = proxy.clone();
        move |result: FetchResult| {
            let _ = proxy.send_event(UserEvent::NetResult(result));
        }
    });
    app.set_net_callback(net_callback);

    let mut platform = PlatformApp::new(proxy);
    platform.app = Some(Box::new(app));           // <- inject the app

    event_loop.run_app(&mut platform).expect("crashed");
}

fn make_stream_callback(proxy: EventLoopProxy<UserEvent>) -> NetStreamCallback {
    Arc::new(move |net_event: NetEvent| {
        let _ = proxy.send_event(UserEvent::NetStream(net_event));
    })
}

struct PlatformApp {
    window: Option<Arc<Window>>,
    proxy: EventLoopProxy<UserEvent>,
    // ticker_started: bool,
    renderer: Option<Renderer>,
    repaint: Option<Arc<PlatformRepaint>>,
    app: Option<Box<dyn UiApp>>,
}

impl PlatformApp {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            proxy: proxy,
            // ticker_started: false,
            renderer: None,
            repaint: None,
            app: None,
        }
    }

    fn some_start_nav(&mut self, url: String) {
        // Build a stream callback:
        let proxy = self.proxy.clone();
        let callback_stream = Arc::new(move |e: NetEvent| {
            let _ = proxy.send_event(UserEvent::NetStream(e));
        });
        // call net::fetch_text_stream(url, callback_stream)
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

        renderer.render(window.as_ref(), |ctx| app.ui(ctx));
    }
}

impl ApplicationHandler<UserEvent> for PlatformApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.init_window(event_loop);
        self.init_renderer();

        let repaint = Arc::new(PlatformRepaint::new(self.proxy.clone()));
        if let Some(app) = self.app.as_mut() {
            let handle: RepaintHandle = repaint.clone();
            app.set_repaint_handle(handle);
            app.set_net_stream_callback(
                make_stream_callback(self.proxy.clone())
            );
        }
        self.repaint = Some(repaint);

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::NetResult(result) => {
                if let Some(app) = self.app.as_mut() {
                    app.on_net_result(result);
                }
            }
            UserEvent::NetStream(event) => {
                if let Some(app) = self.app.as_mut() {
                    app.on_net_stream(event);
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
