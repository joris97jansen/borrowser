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
    TopBottomPanel,
    Window as EguiWindow,
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
    renderer: Option<gfx::Renderer>,
}

impl PlatformApp {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        Self {
            window: None,
            proxy: proxy,
            ticker_started: false,
            renderer: None,
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

    fn init_renderer(&mut self) {
        if self.renderer.is_some() {
            return;
        }
        let window = self.window.as_ref().unwrap();
        let renderer = gfx::Renderer::new(window);
        self.renderer = Some(renderer);
    }

    fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(new_size);
        }
    }

    fn draw_frame(&mut self) {
        let window = self.window.as_ref().unwrap();
        let renderer = self.renderer.as_mut().unwrap();

        renderer.render(window, |context| {
            TopBottomPanel::top("top").show(context, |ui| {
                ui.label("Borrowser - gfc::Renderer");
            });
            EguiWindow::new("Demo").show(context, |ui| {
                ui.label("Rendering moved to gfx crate");
                if ui.button("Click me").clicked() {
                    println!("Button clicked!");
                }
            });
        })
    }
}

impl ApplicationHandler<UserEvent> for PlatformApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.init_window(event_loop);
        self.init_ticker();
        self.init_renderer();
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
        if let (Some(window), Some(renderer)) = (self.window.as_ref(), self.renderer.as_mut()) {
            renderer.on_window_event(&window, &event);
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
