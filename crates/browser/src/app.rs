use crate::renderer::Renderer;
use winit::{
    application::ApplicationHandler,
    event::{WindowEvent, ElementState},
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
    keyboard::{Key, NamedKey},
};

pub struct App {
    window: Option<Window>,
    renderer: Option<Renderer>,
    animate: bool,
}

// zero-arg constructor
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            animate: true,
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
           WindowEvent::KeyboardInput{ event, .. } => {
             if event.state == ElementState::Pressed {
                match &event.logical_key {
                          Key::Named(NamedKey::Escape) => event_loop.exit(),
                          Key::Named(NamedKey::Space) => {
                             self.animate = !self.animate;
                             if let Some(window) = &self.window.as_ref() {
                                 window.set_title(&format!(
                                         "Borrowser - animate: {}", if self.animate { "on" } else { "off" }
                                 ))
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


