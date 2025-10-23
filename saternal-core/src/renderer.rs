use crate::font::FontManager;
use alacritty_terminal::term::Term;
use alacritty_terminal::grid::Dimensions;
use anyhow::Result;
use log::info;
use parking_lot::Mutex;
use std::sync::Arc;
use wgpu;

/// GPU-accelerated renderer using wgpu/Metal
pub struct Renderer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'a>,
    config: wgpu::SurfaceConfiguration,
    font_manager: FontManager,
}

impl<'a> Renderer<'a> {
    /// Create a new renderer
    pub async fn new(
        window: &'a winit::window::Window,
        font_family: &str,
        font_size: f32,
    ) -> Result<Self> {
        info!("Initializing GPU renderer with Metal backend");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL, // Force Metal on macOS
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        info!("Using GPU adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Saternal Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, // VSync
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let font_manager = FontManager::new(font_family, font_size)?;

        Ok(Self {
            device,
            queue,
            surface,
            config,
            font_manager,
        })
    }

    /// Render a frame with terminal content
    pub fn render<T>(&mut self, term: Option<Arc<Mutex<Term<T>>>>) -> Result<()> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.95,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // TODO: Implement actual glyph rendering with wgpu
            // For now, just clear the background
            // In a complete implementation:
            // 1. Iterate through terminal grid cells
            // 2. Rasterize glyphs using font_manager
            // 3. Upload glyph textures to GPU
            // 4. Render quads with text textures
            
            if let Some(term_arc) = term {
                if let Some(term_lock) = term_arc.try_lock() {
                    // Access the terminal grid
                    let _rows = term_lock.screen_lines();
                    let _cols = term_lock.columns();
                    
                    // TODO: Iterate through grid and render each cell
                    // let grid = &term_lock.grid();
                    // for row in grid.display_iter() {
                    //     for cell in row {
                    //         // Render cell character at position
                    //     }
                    // }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// Resize the renderer
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Get font manager
    pub fn font_manager(&mut self) -> &mut FontManager {
        &mut self.font_manager
    }
}
