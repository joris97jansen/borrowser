use std::sync::Arc;
use wgpu::{
    Device, Queue, Surface, SurfaceConfiguration, InstanceDescriptor, Instance,
    TextureUsages, DeviceDescriptor, Features, Limits, MemoryHints, Trace, SurfaceError,
    TextureViewDescriptor, CommandEncoderDescriptor, RenderPassDescriptor, RenderPassColorAttachment, Operations, LoadOp, Color, StoreOp,
};
use winit::window::Window;
use pollster::block_on;

pub struct Renderer {
    surface: Surface<'static>,
    device: Device,
    queue: Queue,
    config: SurfaceConfiguration,
    size: (u32, u32),
}

impl Renderer {
   pub fn new(window: &Arc<Window>) -> Self {
       let instance = Instance::new(&InstanceDescriptor::default());
       let surface = unsafe { instance.create_surface(Arc::clone(window)) }.expect("surface");

       let adapter = block_on(instance.request_adapter(
           &wgpu::RequestAdapterOptions {
               compatible_surface: Some(&surface),
               power_preference: wgpu::PowerPreference::HighPerformance,
               force_fallback_adapter: false,
           }
       )).expect("adapter");

      let (device, queue) = block_on(adapter.request_device(
              &DeviceDescriptor{
                  label: Some("Device"),
                  required_features: Features::empty(),
                  required_limits: Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                  memory_hints: MemoryHints::default(),
                  trace: Trace::default(),
              }
            )
      ).expect("device");

      let caps = surface.get_capabilities(&adapter);

      let format = caps.formats.iter().copied()
        .find(|f| f.is_srgb())
        .unwrap_or(caps.formats[0]);

      let present_mode = caps.present_modes.iter().copied()
        .find(|m| matches!(m, wgpu::PresentMode::AutoVsync | wgpu::PresentMode::Fifo))
        .unwrap_or(caps.present_modes[0]);


       let size = window.inner_size();
       let (w, h) = (size.width.max(1), size.height.max(1));

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

      surface.configure(&device, &config);

      return Self {
          surface,
          device,
          queue,
          config,
          size: (w, h),
      }
   }

   pub fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        if (w, h) == self.size { return; }
        self.size = (w, h);
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }

   pub fn render(&mut self) {
    if self.config.width == 0 || self.config.height == 0 {
        return;
    }

    match self.surface.get_current_texture() {
        Ok(frame) => {
            let view = frame.texture.create_view(&TextureViewDescriptor::default());
            let mut encoder = self.device.create_command_encoder(
                &CommandEncoderDescriptor {
                    label: Some("clean coder"),
                }
            );
            {
                let _pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("clean pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(Color { r: 0.12, g: 0.14, b: 0.22, a: 1.0 }),
                            store: StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
            }
            self.queue.submit([encoder.finish()]);
            frame.present();
        }
        Err(SurfaceError::Lost | SurfaceError::Outdated) => {
            self.surface.configure(&self.device, &self.config);
            return;
        }
        Err(SurfaceError::OutOfMemory) => {
            panic!("Out of memory");
        }
        Err(SurfaceError::Timeout) => {
            return;
        }
        Err(SurfaceError::Other) => {
            eprintln!("SurfaceError::Other");
            return;
        }
    }
   }
}
