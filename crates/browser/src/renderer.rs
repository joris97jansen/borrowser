use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{
    Device, Queue, Surface, SurfaceConfiguration, InstanceDescriptor, Instance,
    TextureUsages, DeviceDescriptor, Features, Limits, MemoryHints, Trace, SurfaceError,
    TextureViewDescriptor, CommandEncoderDescriptor, RenderPassDescriptor, RenderPassColorAttachment, Operations, LoadOp, Color, StoreOp,
    ShaderSource, PipelineLayoutDescriptor, RenderPipeline, RenderPipelineDescriptor, PipelineCompilationOptions, VertexState, FragmentState,
};
use winit::window::Window;
use pollster::block_on;

const TRIANGLE_WGSL: &str = include_str!("shaders/triangle.wgsl");

pub struct Renderer {
    surface: Surface<'static>,
    pub device: Device,
    queue: Queue,
    pub config: SurfaceConfiguration,
    size: (u32, u32),
    clear_color: Color,
    render_pipeline: RenderPipeline,
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

      let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor{
            label: Some("triangle shader"),
            source: ShaderSource::Wgsl(Cow::Borrowed(TRIANGLE_WGSL)),
        });

      let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor{
          label: Some("triangle layout"),
          bind_group_layouts: &[],
          push_constant_ranges: &[],
        });

      let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("triangle pipeline"),
        cache: None,
        vertex: VertexState {
          compilation_options: PipelineCompilationOptions::default(),
          module: &shader,
          entry_point: Some("vs_main"),
          buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
          compilation_options: PipelineCompilationOptions::default(),
          module: &shader,
          entry_point: Some("fs_main"),
          targets: &[Some(wgpu::ColorTargetState {
            format: config.format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
          })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        layout: Some(&pipeline_layout),
      });

      return Self {
          surface,
          device,
          queue,
          config,
          size: (w, h),
          clear_color: Color::default(),
          render_pipeline: render_pipeline,
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
    let (Some(eg), Some(w), Some(r)) = (self.egui.as_mut(), self.window.as_ref(), self.renderer.as_mut()) else { return; };
    eg.begin(w);

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
                let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: Some("clean pass"),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: Operations {
                            load: LoadOp::Clear(self.clear_color),
                            store: StoreOp::Store,
                        },
                        // depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                pass.set_pipeline(&self.render_pipeline);
                pass.draw(0..3, 0..1);
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

   pub fn set_clear_color(&mut self, r: f64, g: f64, b: f64, a: f64) {
     self.clear_color = Color { r, g, b, a }
   }
}
