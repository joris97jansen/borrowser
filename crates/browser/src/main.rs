use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

struct App {
    window: Option<Window>,
    frame: u64,
}

impl Default for App {
    fn default() -> Self {
        Self { window: None, frame: 0 }
    }
}

// This method is implemented to override the default behavior for the struct App
impl ApplicationHandler for App{
    // The resumed method expect a borrowed & mutable reference to self and a borrowed reference to an ActiveEventLoop
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop.create_window(Window::default_attributes().with_title("Borrowser"))
            .expect("Failed to create window");
        // Move ownership of the Window into your field.
        // Some(...) means "Option has a value"; the opposite is None.
        self.window = Some(window);
    }

    // The about_to_wait method expexts a borrowed & mutable reference to self and a borrowed
    // reference to an ActiveEventLoop
   fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
       // if the the borrowed self.window is Some(window) (there is an actual value) then call the
       // request_redraw method on the window
       // Check if self.window is Some and if so, borrow the inner value as window
       if let Some(window) = &self.window {
           window.request_redraw();
       }
   }

   // The window_event method expect a borrowed & mutable reference to self, a borrowed reference
   // to an ActiveEventLoop, a WindowId (Can't guess/see the type) and a WindowEvent (Enum from
   // Winit crate0)
    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
      match event {
           WindowEvent::CloseRequested => event_loop.exit(),
           WindowEvent::RedrawRequested => {
               self.frame += 1;
               println!("redraw #{}", self.frame);
           }
           _ => {}
       }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app);
}

