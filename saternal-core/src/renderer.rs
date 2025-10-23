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
use wgpu::util::DeviceExt;

/// GPU-accelerated renderer using wgpu/Metal
pub struct Renderer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'a>,
    config: wgpu::SurfaceConfiguration,
    font_manager: FontManager,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    cell_width: f32,
    cell_height: f32,
    baseline_offset: f32,
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

        // Get the preferred surface format and use it for both surface and texture
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Choose the best supported alpha mode
        let alpha_mode = if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            surface_caps.alpha_modes[0] // Use first available
        };

        info!("Using surface format: {:?}, alpha mode: {:?}", surface_format, alpha_mode);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, // VSync
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let font_manager = FontManager::new(font_family, font_size)?;

        // Calculate cell dimensions and baseline
        let (cell_width, cell_height, baseline_offset) = {
            let line_metrics = font_manager.font().horizontal_line_metrics(font_size).unwrap();
            let cell_width = font_manager.font().metrics('M', font_size).advance_width;
            let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
            // Baseline is positioned from the top of the cell by the ascent value
            let baseline_offset = line_metrics.ascent.ceil();
            (cell_width, cell_height, baseline_offset)
        };

        // Create texture for text rendering using the SAME format as surface
        // This is critical - texture and surface must have matching formats
        let texture_size = wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        };
        info!("Creating text texture: {}x{} with format {:?}", size.width, size.height, surface_format);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format, // FIXED: Use same format as surface!
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
                    array_stride: 16, // 2 floats (pos) + 2 floats (tex) = 16 bytes
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
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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

        // Create vertex buffer with fullscreen quad vertices
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

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: unsafe {
                std::slice::from_raw_parts(
                    vertices.as_ptr() as *const u8,
                    std::mem::size_of_val(&vertices),
                )
            },
            usage: wgpu::BufferUsages::VERTEX,
        });

        info!("Created fullscreen quad vertex buffer with 6 vertices");

        Ok(Self {
            device,
            queue,
            surface,
            config,
            font_manager,
            texture,
            bind_group,
            bind_group_layout,
            sampler,
            render_pipeline,
            vertex_buffer,
            cell_width,
            cell_height,
            baseline_offset,
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
                            // Black background for terminal
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
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
        let cursor = term.grid().cursor.point;
        log::info!("Rendering terminal: {}x{} cells, cursor at ({}, {})",
                   cols, rows, cursor.column.0, cursor.line.0);

        // Create buffer for the entire window/texture
        let width = self.config.width;
        let height = self.config.height;

        // Determine if we need BGRA or RGBA based on surface format
        let is_bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Create buffer for rendering terminal text
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Render each cell from the terminal grid
        let mut char_count = 0;
        for row_idx in 0..rows {
            let line = Line(row_idx as i32);
            for col_idx in 0..cols {
                let column = Column(col_idx);
                let cell = &term.grid()[line][column];

                // Get character
                let c = cell.c;

                if c == '\0' {
                    continue; // Skip null cells
                }

                // Skip spaces (only render visible text)
                if c == ' ' {
                    continue;
                }
                char_count += 1;

                // Get colors
                let (fg_r, fg_g, fg_b) = self.get_cell_color(&cell.fg);

                // Rasterize glyph
                let (metrics, bitmap) = self.font_manager.rasterize(c);

                // Calculate cell position in window coordinates
                let cell_x = col_idx as f32 * self.cell_width;
                let cell_y = row_idx as f32 * self.cell_height;

                // Calculate baseline position (from top of cell)
                let baseline_y = cell_y + self.baseline_offset;

                // Calculate glyph position using proper baseline alignment
                // In fontdue, ymin is the offset from baseline to top of glyph (negative means above baseline)
                // So glyph top Y = baseline - (height + ymin)
                let glyph_x = cell_x;
                let glyph_y = baseline_y - (metrics.height as f32 + metrics.ymin as f32);

                if row_idx == 0 && col_idx < 5 {
                    log::debug!("Char '{}' at cell ({}, {}) -> glyph ({:.1}, {:.1}), baseline {:.1}, metrics: h={} ymin={}",
                               c, cell_x, cell_y, glyph_x, glyph_y, baseline_y, metrics.height, metrics.ymin);
                }

                // Draw glyph to buffer with premultiplied alpha
                for gy in 0..metrics.height {
                    for gx in 0..metrics.width {
                        let px = (glyph_x as i32 + gx as i32);
                        let py = (glyph_y as i32 + gy as i32);

                        // Bounds check
                        if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                            let glyph_idx = gy * metrics.width + gx;
                            let coverage = bitmap[glyph_idx];

                            if coverage > 0 {
                                let buffer_idx = ((py as usize * width as usize) + px as usize) * 4;

                                // Premultiply the color channels by alpha for correct blending
                                let alpha = coverage as f32 / 255.0;
                                let fg_r_pre = (fg_r as f32 * alpha) as u8;
                                let fg_g_pre = (fg_g as f32 * alpha) as u8;
                                let fg_b_pre = (fg_b as f32 * alpha) as u8;

                                // Write in correct channel order (BGRA or RGBA)
                                if is_bgra {
                                    buffer[buffer_idx] = fg_b_pre;     // B (premultiplied)
                                    buffer[buffer_idx + 1] = fg_g_pre; // G (premultiplied)
                                    buffer[buffer_idx + 2] = fg_r_pre; // R (premultiplied)
                                    buffer[buffer_idx + 3] = coverage; // A
                                } else {
                                    buffer[buffer_idx] = fg_r_pre;     // R (premultiplied)
                                    buffer[buffer_idx + 1] = fg_g_pre; // G (premultiplied)
                                    buffer[buffer_idx + 2] = fg_b_pre; // B (premultiplied)
                                    buffer[buffer_idx + 3] = coverage; // A
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!("Rendered {} non-empty characters from terminal grid", char_count);

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
            info!("Resizing renderer to {}x{}", width, height);
            
            // Update surface configuration
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            
            // Recreate texture with new dimensions
            let texture_size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            
            self.texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Text Texture"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            
            // Recreate texture view
            let texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
            
            // Recreate bind group with new texture view
            self.bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Texture Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            
            info!("Renderer resized successfully");
        }
    }

    /// Get font manager
    pub fn font_manager(&mut self) -> &mut FontManager {
        &mut self.font_manager
    }

    /// Update font size and recalculate cell dimensions
    pub fn set_font_size(&mut self, font_size: f32) -> Result<()> {
        // Update font manager
        self.font_manager.set_font_size(font_size);
        
        // Recalculate cell dimensions using the SAME formula as initialization
        // This ensures proper line spacing and baseline alignment
        let line_metrics = self.font_manager.font().horizontal_line_metrics(font_size).unwrap();
        self.cell_width = self.font_manager.font().metrics('M', font_size).advance_width;
        self.cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        
        // Baseline is positioned from the top of the cell by the ascent value (NOT an approximation!)
        self.baseline_offset = line_metrics.ascent.ceil();
        
        info!("Font size updated to {}: cell={}x{}, baseline={}", 
              font_size, self.cell_width, self.cell_height, self.baseline_offset);
        
        // Recreate vertex buffer with new dimensions
        self.recreate_vertex_buffer()?;
        
        Ok(())
    }
    
    /// Recreate vertex buffer with current cell dimensions
    fn recreate_vertex_buffer(&mut self) -> Result<()> {
        // Use the same vertex structure as the original
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

        self.vertex_buffer = self.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: unsafe {
                    std::slice::from_raw_parts(
                        vertices.as_ptr() as *const u8,
                        std::mem::size_of_val(&vertices),
                    )
                },
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        );
        
        info!("Recreated vertex buffer with 6 vertices for new cell size: {}x{}", 
              self.cell_width, self.cell_height);
        
        Ok(())
    }
}
