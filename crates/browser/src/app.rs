use std::sync::mpsc::{
    Sender, Receiver, channel
};
use std::thread;
use std::time::Instant;
use std::f32::consts::TAU;
use std::sync::Arc;
use crate::renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Window, WindowId, Fullscreen},
    keyboard::{Key, NamedKey, ModifiersState},
};

const OMEGA: f32 = 2.0;
const OMEGA_MIN: f32 = 0.05;
const OMEGA_MAX: f32 = 20.0;
const OMEGA_SCALE: f32 = 1.2;
const OMEGA_EASE: f32 = 8.0;

enum PageMessage {
    Loaded { url: String, body: String },
    Failed { url: String, reason: String },
}

pub enum Wake {
    NetReady,
}

pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    modifiers: ModifiersState,
    animate: bool,
    is_fullscreen: bool,
    omega: f32,
    target_omega: f32,
    color_running: bool,
    phase: f32,
    last_tick: Instant,
    proxy: EventLoopProxy<Wake>,
    fetch_tx: Sender<PageMessage>,
    fetch_rx: Receiver<PageMessage>,

}

impl ApplicationHandler<Wake> for App{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let raw_window = event_loop.create_window(Window::default_attributes().with_title("Borrowser"))
            .expect("Failed to create window");
        let window = Arc::new(raw_window);
        let renderer = Renderer::new(&window);
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

   fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
       if let Some(window) = self.window.as_ref() {
           if self.animate {
               window.request_redraw();
           }
       }
   }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
      match event {
           WindowEvent::CloseRequested => event_loop.exit(),
           WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
           }
           WindowEvent::RedrawRequested => {
               if let Some(renderer) = self.renderer.as_mut() {
                   let now = Instant::now();
                   let dt = (now - self.last_tick).as_secs_f32();
                   self.last_tick = now;

                   let delta = self.target_omega - self.omega;
                   // exponential smoothing factor based on dt
                   let a = 1.0 - (-OMEGA_EASE * dt).exp();  // in [0, 1)
                   self.omega += delta * a;

                   if self.color_running {
                       self.phase += self.omega * dt;
                       if self.phase > TAU { self.phase -= TAU; }

                       let r = 0.5 + 0.5 * (self.phase + 0.0).sin();
                       let g = 0.5 + 0.5 * (self.phase + TAU/3.0).sin();
                       let b = 0.5 + 0.5 * (self.phase + 2.0*TAU/3.0).sin();

                       renderer.set_clear_color(r as f64, g as f64, b as f64, 1.0);
                   }
                   renderer.render();
               }
           }
           WindowEvent::KeyboardInput{ event, .. } => {
             if event.state == ElementState::Pressed {
                match &event.logical_key {
                          Key::Named(NamedKey::Escape) => {
                              if self.is_fullscreen {
                                self.toggle_fullscreen();
                              }
                          },
                          Key::Named(NamedKey::Space) => {
                             self.animate = !self.animate;
                             if self.animate {
                                 self.last_tick = Instant::now();
                             }
                             if let Some(window) = self.window.as_ref() {
                                 window.set_title(&format!(
                                         "Borrowser - animate: {}", if self.animate { "on" } else { "off" }
                                 ))
                             }
                          }
                          Key::Character(ch) if ch == "c" || ch == "C" => {
                              self.color_running = !self.color_running;
                          }
                          Key::Named(NamedKey::F11) => {
                              self.toggle_fullscreen();
                          }
                          Key::Named(NamedKey::Enter) => {
                              let fetch_tx = self.fetch_tx.clone();
                              let proxy = self.proxy.clone();
                              let url = "https://example.org".to_string();

                              thread::spawn(move || {
                                  let message = match net::fetch_text(&url) {
                                      Ok(body) => PageMessage::Loaded { url, body },
                                      Err(err) => PageMessage::Failed { url, reason: err.to_string()},
                                  };
                                  let _ = fetch_tx.send(message);
                                  let _ = proxy.send_event(Wake::NetReady);
                              });
                              if let Some(window) = self.window.as_ref() {
                                  window.set_title("Borrowser - loading...");
                                  window.request_redraw();
                              }
                          }
                          Key::Character(ch) if (ch == "q" || ch == "Q") && self.modifiers.super_key() => {
                             event_loop.exit();
                          }
                          Key::Character(ch) if (ch == "=" && self.modifiers.super_key()) => {
                              self.target_omega = (self.target_omega * OMEGA_SCALE).min(OMEGA_MAX);
                          }
                          Key::Character(ch) if (ch == "-" && self.modifiers.super_key()) => {
                              self.target_omega = (self.target_omega / OMEGA_SCALE).max(OMEGA_MIN);
                          }
                          Key::Character(ch) => {
                            println!("Key pressed: {} with modifiers {:?}", ch, self.modifiers);
                            if (ch == "f" || ch == "F") && self.modifiers.super_key() && self.modifiers.control_key() {
                                self.toggle_fullscreen();
                            }
                          }
                          _ => { /* Unhandled key */}
                      }
               }
           }
           WindowEvent::Resized(size) => {
               if let Some(renderer) = self.renderer.as_mut() {
                   renderer.resize(size.width, size.height);
               }
               if let Some(window) = self.window.as_mut() {
                   window.set_title(&format!(
                       "Borrowser - size: {}x{}", size.width, size.height
                   ))
               }
           }
           _ => {}
       }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: Wake) {
        match event {
            Wake::NetReady => {
                for message in self.fetch_rx.try_iter() {
                    match message {
                        PageMessage::Loaded{ url: _, body} => {
                            let title = extract_title(&body).unwrap_or_else(|| "(no title)".to_string());
                            if let Some(window) = self.window.as_ref() {
                                let kb = body.len() as f64 / 1024.0;
                                window.set_title(&format!("Borrowser - {} ({} kb)", title, kb));
                                window.request_redraw();
                            }
                        }
                        PageMessage::Failed { url: _, reason } => {
                            if let Some(window) = self.window.as_ref() {
                                window.set_title(&format!("Borrowser - load failed: {}", reason));
                                window.request_redraw();
                            }
                        }
                    }
                }
            }
        }
    }
}

impl App {
    pub fn new(proxy: EventLoopProxy<Wake>) -> Self{
        let (fetch_tx, fetch_rx) = channel();

        Self {
            window: None,
            renderer: None,
            modifiers: ModifiersState::default(),
            animate: true,
            is_fullscreen: false,
            omega: OMEGA,
            target_omega: OMEGA,
            color_running: true,
            phase: 0.0,
            last_tick: Instant::now(),
            proxy: proxy,
            fetch_tx: fetch_tx,
            fetch_rx: fetch_rx,
        }
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(window) = self.window.as_ref() {
            self.is_fullscreen = !self.is_fullscreen;
            if self.is_fullscreen {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            } else {
                window.set_fullscreen(None);
            }
            window.set_title(&format!(
                "Borrowser - fullscreen: {}", if self.is_fullscreen { "on" } else { "off" }
            ))
        }
    }

}

fn extract_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start_tag = lower.find("<title")?;
    let after_start = &html[start_tag..];

    let start_gt = after_start.find('>')?;
    let after_gt = &html[start_tag + start_gt + 1..];

    let lower_after = after_gt.to_lowercase();
    let end_tag = lower_after.find("</title>")?;

    let raw_title = &after_gt[..end_tag];
    let title = raw_title.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(title)
}
