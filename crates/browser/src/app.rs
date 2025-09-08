use crate::renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId, Fullscreen},
    keyboard::{Key, NamedKey, ModifiersState},
};

pub struct App {
    window: Option<Window>,
    renderer: Option<Renderer>,
    modifiers: ModifiersState,
    animate: bool,
    is_fullscreen: bool,
}

// zero-arg constructor
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            modifiers: ModifiersState::default(),
            animate: true,
            is_fullscreen: false,
        }
    }
}

impl ApplicationHandler for App{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop.create_window(Window::default_attributes().with_title("Borrowser"))
            .expect("Failed to create window");
        let renderer = Renderer::new(&window);
        self.window = Some(window);
        self.renderer = Some(renderer);
    }

   fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
       if let Some(window) = &self.window.as_ref() {
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
           WindowEvent::KeyboardInput{ event, .. } => {
             if event.state == ElementState::Pressed {
                match &event.logical_key {
                          Key::Named(NamedKey::Escape) => {
                              if (self.is_fullscreen) {
                                self.toggle_fullscreen();
                              }
                          },
                          Key::Named(NamedKey::Space) => {
                             self.animate = !self.animate;
                             if let Some(window) = &self.window.as_ref() {
                                 window.set_title(&format!(
                                         "Borrowser - animate: {}", if self.animate { "on" } else { "off" }
                                 ))
                             }
                          }
                          Key::Named(NamedKey::F11) => {
                              self.toggle_fullscreen();
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
           }
           _ => {}
       }
    }
}

impl App {
    fn toggle_fullscreen(&mut self) {
        if let Some(window) = &self.window.as_ref() {
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
