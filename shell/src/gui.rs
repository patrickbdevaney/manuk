//! The interactive GPU window (feature `gui`): `winit` for the window/event loop,
//! `wgpu` for the GPU surface.
//!
//! CLAUDE.md's paint target is Vello (GPU-compute) on `wgpu`. Vello is alpha, so
//! this window presents the CPU raster tier's canvas as a GPU-sampled fullscreen
//! quad — a real `wgpu` present path into which a `VelloGpuPainter` slots later for
//! the focused tab. Scroll re-rasterizes the visible viewport; resize reflows.

use std::sync::Arc;

use anyhow::{Context, Result};
use manuk_compositor::Viewport;
use manuk_css::Rgba;
use manuk_paint::CpuPainter;
use manuk_text::FontContext;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::tab::Browser;
use manuk_compositor::TabId;
use manuk_page::{fetch_html, Page};

const WGSL: &str = r#"
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    // Fullscreen triangle.
    var pts = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let xy = pts[i];
    var out: VsOut;
    out.pos = vec4(xy, 0.0, 1.0);
    out.uv = vec2((xy.x + 1.0) * 0.5, (1.0 - xy.y) * 0.5);
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv);
}
"#;

/// Launch the browser window pointed at `url`, with an initial content width.
pub fn run(url: String, width: u32, measure_frames: Option<usize>) -> Result<()> {
    let event_loop = EventLoop::new().context("creating winit event loop")?;
    let mut app = App::new(url, width, measure_frames);
    event_loop.run_app(&mut app).context("running event loop")?;
    Ok(())
}

struct App {
    url: String,
    width: u32,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    fonts: FontContext,
    page: Option<Page>,
    viewport: Viewport,
    scroll_y: f32,
    browser: Browser,
    tab_id: TabId,
    /// Rolling GPU-present frame timer (§8 metric #4) — real on-screen frames.
    frame: manuk_compositor::FrameTimer,
    /// If set, render this many frames back-to-back, print GPU stats, then exit.
    measure_frames: Option<usize>,
    frames_done: usize,
}

impl App {
    fn new(url: String, width: u32, measure_frames: Option<usize>) -> Self {
        // One window == one tab for now, but the tab/tier model is wired so
        // multi-tab is an additive change (CLAUDE.md § per-tab memory).
        let mut browser = Browser::new(8);
        let tab_id = browser.open(url.clone());
        App {
            url,
            width,
            window: None,
            gpu: None,
            fonts: FontContext::new(),
            page: None,
            viewport: Viewport::new(width as f32, 768.0),
            scroll_y: 0.0,
            browser,
            tab_id,
            frame: manuk_compositor::FrameTimer::new(240),
            measure_frames,
            frames_done: 0,
        }
    }

    fn load_page(&mut self, width: u32, height: u32) {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("tokio runtime: {e}");
                return;
            }
        };
        match rt.block_on(fetch_html(&self.url)) {
            Ok((html, final_url)) => {
                let page = Page::load(&html, &final_url, &self.fonts, width as f32);
                if let Some(w) = &self.window {
                    w.set_title(&format!("{} — manuk", page.title));
                }
                self.viewport = Viewport::new(width as f32, height as f32);
                self.viewport.content_height = page.content_height;
                self.browser.set_loaded(
                    self.tab_id,
                    page.final_url.clone(),
                    page.title.clone(),
                    page.content_height,
                );
                self.page = Some(page);
            }
            Err(e) => tracing::error!("load {}: {e:#}", self.url),
        }
    }

    fn rerender(&mut self) {
        let (Some(gpu), Some(page)) = (&mut self.gpu, &self.page) else {
            return;
        };
        let canvas = CpuPainter::new(&self.fonts).render_scrolled(
            &page.root_box,
            gpu.config.width,
            gpu.config.height,
            Rgba::WHITE,
            self.scroll_y,
        );
        gpu.upload(&canvas);
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn clamp_scroll(&mut self) {
        self.scroll_y = self.scroll_y.clamp(0.0, self.viewport.max_scroll());
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("manuk")
            .with_inner_size(LogicalSize::new(self.width as f64, 768.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                tracing::error!("create_window: {e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        match pollster::block_on(Gpu::new(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
        )) {
            Ok(gpu) => {
                self.window = Some(window);
                self.gpu = Some(gpu);
            }
            Err(e) => {
                tracing::error!("wgpu init: {e:#}");
                event_loop.exit();
                return;
            }
        }
        self.load_page(size.width.max(1), size.height.max(1));
        self.rerender();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                let (w, h) = (size.width.max(1), size.height.max(1));
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(w, h);
                }
                if let Some(page) = &mut self.page {
                    page.relayout(&self.fonts, w as f32);
                    self.viewport.width = w as f32;
                    self.viewport.height = h as f32;
                    self.viewport.content_height = page.content_height;
                }
                self.clamp_scroll();
                self.rerender();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 48.0,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                self.scroll_y -= dy;
                self.clamp_scroll();
                self.rerender();
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    self.frame.begin();
                    if let Err(e) = gpu.draw() {
                        tracing::warn!("draw: {e:?}");
                    }
                    self.frame.end();
                    // Log frame stats once per window of frames.
                    if self.frame.len() == 120 {
                        if let (Some(avg), Some(fps)) = (self.frame.average(), self.frame.fps()) {
                            tracing::debug!(
                                frame_ms = avg.as_secs_f64() * 1000.0,
                                fps,
                                janky = self.frame.janky(manuk_compositor::FRAME_BUDGET_60FPS),
                                "gpu present frame stats (120-frame window)"
                            );
                        }
                    }
                    // §8 metric #4: `browse --frames N` renders N frames back-to-back,
                    // reports GPU-present stats, then exits — a headful measurement.
                    if let Some(n) = self.measure_frames {
                        self.frames_done += 1;
                        if self.frames_done >= n {
                            let avg = self.frame.average().unwrap_or_default();
                            let p95 = self.frame.p95().unwrap_or_default();
                            println!(
                                "gpu-present over {} frames: avg {:.2} ms ({:.0} fps), p95 {:.2} ms, jank {}/{}",
                                self.frame.len(),
                                avg.as_secs_f64() * 1000.0,
                                self.frame.fps().unwrap_or(0.0),
                                p95.as_secs_f64() * 1000.0,
                                self.frame.janky(manuk_compositor::FRAME_BUDGET_60FPS),
                                self.frame.len(),
                            );
                            event_loop.exit();
                        } else if let Some(w) = &self.window {
                            w.request_redraw(); // keep the render loop running
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// GPU state: surface, device/queue, the present pipeline, and the current frame
/// texture + bind group uploaded from the CPU canvas.
struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
}

impl Gpu {
    async fn new(window: Arc<Window>, width: u32, height: u32) -> Result<Gpu> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window).context("create_surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("no suitable GPU adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("manuk device"),
                ..Default::default()
            })
            .await
            .context("request_device")?;

        let config = surface
            .get_default_config(&adapter, width, height)
            .context("surface unsupported by adapter")?;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present shader"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("present bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("present pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        Ok(Gpu {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group_layout,
            sampler,
            bind_group: None,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Upload a CPU canvas as the texture to present next frame.
    fn upload(&mut self, canvas: &manuk_paint::Canvas) {
        let size = wgpu::Extent3d {
            width: canvas.width(),
            height: canvas.height(),
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("page texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            canvas.rgba_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * canvas.width()),
                rows_per_image: Some(canvas.height()),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("page bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        }));
    }

    fn draw(&mut self) -> Result<(), wgpu::SurfaceError> {
        let Some(bind_group) = &self.bind_group else {
            return Ok(()); // nothing uploaded yet
        };
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                self.surface.get_current_texture()?
            }
            Err(e) => return Err(e),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("present pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
