use crate::font::FontManager;
use alacritty_terminal::term::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
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
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    cell_width: f32,
    cell_height: f32,
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
        
        // Calculate cell dimensions
        let (cell_width, cell_height) = {
            let metrics = font_manager.font().horizontal_line_metrics(font_size).unwrap();
            let cell_width = font_manager.font().metrics('M', font_size).advance_width;
            let cell_height = (metrics.ascent - metrics.descent + metrics.line_gap).ceil();
            (cell_width, cell_height)
        };

        // Create texture for text rendering (RGBA)
        // Size it to match the window/surface size so it fills the screen
        let texture_size = wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        };
        log::info!("Creating text texture: {}x{} (window size)", size.width, size.height);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Create texture view and sampler
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Text Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout and bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });

        // Create render pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 20, // 2 floats (pos) + 2 floats (tex) + 4 bytes (color) = 20 bytes
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create vertex buffer (will be updated per frame)
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (6 * 20 * 5000) as u64, // 5000 characters max
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            surface,
            config,
            font_manager,
            texture,
            bind_group,
            render_pipeline,
            vertex_buffer,
            cell_width,
            cell_height,
        })
    }

    /// Render a frame with terminal content
    pub fn render<T>(&mut self, term: Option<Arc<Mutex<Term<T>>>>) -> Result<()> {
        // Render terminal text to texture
        if let Some(term_arc) = &term {
            log::debug!("Attempting to lock terminal for rendering");
            if let Some(term_lock) = term_arc.try_lock() {
                log::debug!("Terminal locked, rendering to texture");
                self.render_terminal_to_texture(&term_lock)?;
            } else {
                log::warn!("Could not lock terminal for rendering");
            }
        } else {
            log::warn!("No terminal provided to render");
        }

        log::trace!("Getting surface texture for rendering...");
        let frame = self.surface.get_current_texture()?;
        log::trace!("Got surface texture, creating view...");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            // Render the fullscreen quad with terminal texture
            log::trace!("Setting pipeline and drawing quad...");
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            log::trace!("Drawing 6 vertices for fullscreen quad");
            render_pass.draw(0..6, 0..1); // 6 vertices for fullscreen quad
        }
        
        log::trace!("Submitting command buffer and presenting frame...");

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// Render terminal content to texture (CPU-based for simplicity)
    fn render_terminal_to_texture<T>(&mut self, term: &Term<T>) -> Result<()> {
        let rows = term.screen_lines();
        let cols = term.columns();
        log::info!("Rendering terminal: {}x{} cells", cols, rows);

        // Create RGBA buffer for the entire window/texture
        let width = self.config.width;
        let height = self.config.height;
        log::debug!("Texture dimensions: {}x{} pixels (cell: {}x{})", width, height, self.cell_width, self.cell_height);
        
        // TEMP: Fill entire screen with bright green to test pipeline
        let mut buffer = vec![0u8; (width * height * 4) as usize];
        for i in (0..buffer.len()).step_by(4) {
            buffer[i] = 0;       // R
            buffer[i + 1] = 255; // G - bright green
            buffer[i + 2] = 0;   // B  
            buffer[i + 3] = 255; // A - full opacity
        }
        log::info!("Filled entire texture with bright green ({} pixels)", width * height);

        // TEMP: Skip terminal rendering, just test solid color
        /*
        // Render each cell
        let mut char_count = 0;
        for row_idx in 0..rows {
            let line = Line(row_idx as i32);
            for col_idx in 0..cols {
                let column = Column(col_idx);
                let cell = &term.grid()[line][column];

                // Get character
                let c = cell.c;
                if c == ' ' || c == '\0' {
                    continue; // Skip empty cells
                }
                char_count += 1;

                // Get colors
                let (fg_r, fg_g, fg_b) = self.get_cell_color(&cell.fg);

                // Rasterize glyph
                let (metrics, bitmap) = self.font_manager.rasterize(c);

                // Calculate position in window coordinates
                let x = col_idx as f32 * self.cell_width;
                let y = row_idx as f32 * self.cell_height;
                
                if row_idx == 0 && col_idx < 5 {
                    log::debug!("Char '{}' at ({}, {}) with color ({},{},{})", c, x, y, fg_r, fg_g, fg_b);
                }

                // Draw glyph to buffer
                for glyph_y in 0..metrics.height {
                    for glyph_x in 0..metrics.width {
                        let px = x as usize + glyph_x;
                        let py = y as usize + glyph_y + metrics.ymin.max(0) as usize;

                        if px < width as usize && py < height as usize {
                            let glyph_idx = glyph_y * metrics.width + glyph_x;
                            let coverage = bitmap[glyph_idx];

                            if coverage > 0 {
                                let buffer_idx = (py * width as usize + px) * 4;
                                buffer[buffer_idx] = fg_r;
                                buffer[buffer_idx + 1] = fg_g;
                                buffer[buffer_idx + 2] = fg_b;
                                buffer[buffer_idx + 3] = coverage;
                                
                                if row_idx == 0 && col_idx == 0 && glyph_x < 3 && glyph_y < 3 {
                                    log::trace!("Writing pixel at ({},{}) = RGBA({},{},{},{})", px, py, fg_r, fg_g, fg_b, coverage);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        log::info!("Rendered {} non-empty characters to texture (buffer size: {} bytes)", char_count, buffer.len());
        */

        // Upload to GPU texture
        log::debug!("Uploading {}x{} texture to GPU", width, height);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &buffer,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        
        // ALWAYS update vertex buffer for fullscreen quad (even when testing)
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct Vertex {
            position: [f32; 2],
            tex_coords: [f32; 2],
        }

        let vertices = [
            // Top-left triangle
            Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
            Vertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
            // Bottom-right triangle
            Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
            Vertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
            Vertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
        ];

        let vertex_data = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                std::mem::size_of_val(&vertices),
            )
        };

        log::debug!("Writing {} vertices ({} bytes) to vertex buffer", vertices.len(), vertex_data.len());
        self.queue.write_buffer(&self.vertex_buffer, 0, vertex_data);

        Ok(())
    }

    /// Convert terminal color to RGB
    fn get_cell_color(&self, color: &AnsiColor) -> (u8, u8, u8) {
        match color {
            AnsiColor::Named(named) => match named {
                NamedColor::Black => (0, 0, 0),
                NamedColor::Red => (205, 49, 49),
                NamedColor::Green => (13, 188, 121),
                NamedColor::Yellow => (229, 229, 16),
                NamedColor::Blue => (36, 114, 200),
                NamedColor::Magenta => (188, 63, 188),
                NamedColor::Cyan => (17, 168, 205),
                NamedColor::White => (229, 229, 229),
                NamedColor::BrightBlack => (102, 102, 102),
                NamedColor::BrightRed => (241, 76, 76),
                NamedColor::BrightGreen => (35, 209, 139),
                NamedColor::BrightYellow => (245, 245, 67),
                NamedColor::BrightBlue => (59, 142, 234),
                NamedColor::BrightMagenta => (214, 112, 214),
                NamedColor::BrightCyan => (41, 184, 219),
                NamedColor::BrightWhite => (255, 255, 255),
                NamedColor::Foreground => (229, 229, 229),
                _ => (229, 229, 229),
            },
            AnsiColor::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
            AnsiColor::Indexed(idx) => {
                // Basic 256-color palette approximation
                match idx {
                    0..=7 => self.get_cell_color(&AnsiColor::Named(match idx {
                        0 => NamedColor::Black,
                        1 => NamedColor::Red,
                        2 => NamedColor::Green,
                        3 => NamedColor::Yellow,
                        4 => NamedColor::Blue,
                        5 => NamedColor::Magenta,
                        6 => NamedColor::Cyan,
                        7 => NamedColor::White,
                        _ => NamedColor::White,
                    })),
                    8..=15 => self.get_cell_color(&AnsiColor::Named(match idx - 8 {
                        0 => NamedColor::BrightBlack,
                        1 => NamedColor::BrightRed,
                        2 => NamedColor::BrightGreen,
                        3 => NamedColor::BrightYellow,
                        4 => NamedColor::BrightBlue,
                        5 => NamedColor::BrightMagenta,
                        6 => NamedColor::BrightCyan,
                        7 => NamedColor::BrightWhite,
                        _ => NamedColor::White,
                    })),
                    _ => (229, 229, 229), // Default to white
                }
            },
        }
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
