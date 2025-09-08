use winit::window::Window;

pub struct Renderer {
    size: (u32, u32),
    frame: u64,
}

impl Renderer {
   pub fn new(window: &Window) -> Self {
       let s = window.inner_size();
       Self {
           size: (s.width.max(1), s.height.max(1)),
           frame: 0,
       }
   }

   pub fn resize(&mut self, width: u32, height: u32) {
       self.size = (width.max(1), height.max(1));
       eprintln!("Renderer resizedf to {}x{}", width, height);
   }

   pub fn render(&mut self) {
       self.frame += 1;
       if self.frame % 60 == 0 {
           eprintln!("Rendered {} frames at {}x{}", self.frame, self.size.0, self.size.1);
       }
   }
}
