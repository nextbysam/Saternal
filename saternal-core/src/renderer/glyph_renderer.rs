use crate::constants::{PADDING_LEFT, PADDING_TOP};
use crate::font::FontManager;
use crate::renderer::color::ansi_to_rgb_with_palette;
use crate::renderer::theme::ColorPalette;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use anyhow::Result;
use wgpu;

use super::glyph_atlas::GlyphAtlas;

// Maximum instance buffer capacity to prevent unbounded memory growth
const MAX_INSTANCE_CAPACITY: usize = 100_000;
const INITIAL_INSTANCE_CAPACITY: usize = 10_000;

/// Instance data for a single glyph (sent to GPU)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphInstance {
    /// Position in NDC (Normalized Device Coordinates)
    position: [f32; 2],
    /// Size in NDC
    size: [f32; 2],
    /// UV coordinates in atlas
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    /// Foreground color (RGBA)
    color: [f32; 4],
    /// Background color (RGBA)
    bg_color: [f32; 4],
    /// Style flags: bit 0 = bold, bit 1 = underline, bit 2 = reverse
    flags: u32,
    /// Padding to align to 16 bytes
    _padding: [u32; 3],
}

/// Uniform data for screen dimensions
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ScreenUniforms {
    width: f32,
    height: f32,
    _padding: [f32; 2],
}

/// GPU-based glyph renderer using instanced rendering
pub struct GlyphRenderer {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_count: usize,
    
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,
    
    cell_width: f32,
    cell_height: f32,
    baseline_offset: f32,
}

impl GlyphRenderer {
    /// Create a new GPU glyph renderer
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        atlas: &GlyphAtlas,
        cell_width: f32,
        cell_height: f32,
        baseline_offset: f32,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        // Create uniform buffer for screen dimensions
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Glyph Renderer Uniform Buffer"),
            size: std::mem::size_of::<ScreenUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Glyph Renderer Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Glyph Renderer Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create render pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Glyph Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/glyph.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Glyph Pipeline Layout"),
            bind_group_layouts: &[
                &atlas.bind_group_layout,           // @group(0) - Atlas texture
                &uniform_bind_group_layout,          // @group(1) - Screen uniforms
            ],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Glyph Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // size
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // uv_min
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // uv_max
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // color (foreground)
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // bg_color (background)
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        // flags (style bits)
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
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

        // Create instance buffer (initial capacity: 10,000 glyphs)
        let instance_capacity = INITIAL_INSTANCE_CAPACITY.min(MAX_INSTANCE_CAPACITY);
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Glyph Instance Buffer"),
            size: (instance_capacity * std::mem::size_of::<GlyphInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            instance_buffer,
            instance_capacity,
            instance_count: 0,
            uniform_buffer,
            uniform_bind_group_layout,
            uniform_bind_group,
            cell_width,
            cell_height,
            baseline_offset,
        }
    }

    /// Update cell dimensions (when font size changes)
    pub fn update_dimensions(&mut self, cell_width: f32, cell_height: f32, baseline_offset: f32) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.baseline_offset = baseline_offset;
    }

    /// Update screen dimensions
    pub fn update_screen_size(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        let uniforms = ScreenUniforms {
            width: width as f32,
            height: height as f32,
            _padding: [0.0, 0.0],
        };

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );
    }

    /// Generate instances from terminal grid
    pub fn generate_instances<T>(
        &mut self,
        queue: &wgpu::Queue,
        term: &Term<T>,
        atlas: &mut GlyphAtlas,
        font_manager: &FontManager,
        device: &wgpu::Device,
        scroll_offset: usize,
        palette: &ColorPalette,
        screen_width: u32,
        screen_height: u32,
    ) -> Result<()> {
        let mut instances = Vec::new();
        
        let rows = term.screen_lines();
        let cols = term.columns();
        
        // Clamp scroll offset to available history
        let history_size = term.grid().history_size();
        let scroll_offset = scroll_offset.min(history_size);

        // Iterate through terminal grid and generate instances
        for row_idx in 0..rows {
            let line = Line(row_idx as i32 - scroll_offset as i32);
            
            for col_idx in 0..cols {
                let column = Column(col_idx);
                let cell = &term.grid()[line][column];

                let c = cell.c;
                if c == '\0' || c == ' ' {
                    continue; // Skip null and space characters
                }

                // Get or add glyph to atlas
                let glyph_uv = match atlas.get_or_add_glyph(device, queue, font_manager, c) {
                    Ok(uv) => uv,
                    Err(e) => {
                        log::warn!("Failed to get/add glyph '{}': {}", c, e);
                        continue; // Skip if glyph cannot be added
                    }
                };

                // Extract style flags
                let is_bold = cell.flags.contains(Flags::BOLD);
                let is_underline = cell.flags.contains(Flags::UNDERLINE);
                let is_inverse = cell.flags.contains(Flags::INVERSE);
                
                // Get colors from palette
                let mut fg = ansi_to_rgb_with_palette(&cell.fg, palette);
                let mut bg = ansi_to_rgb_with_palette(&cell.bg, palette);
                
                // Handle reverse video (swap foreground and background)
                if is_inverse {
                    std::mem::swap(&mut fg, &mut bg);
                }
                
                let (fg_r, fg_g, fg_b) = fg;
                let (bg_r, bg_g, bg_b) = bg;
                
                // Pack style flags into u32
                let mut flags: u32 = 0;
                if is_bold { flags |= 0x1; }
                if is_underline { flags |= 0x2; }
                if is_inverse { flags |= 0x4; }

                // Calculate pixel position
                let cell_x = PADDING_LEFT + col_idx as f32 * self.cell_width;
                let cell_y = PADDING_TOP + row_idx as f32 * self.cell_height;

                // Calculate glyph position using baseline alignment
                let baseline_y = cell_y + self.baseline_offset;
                let glyph_x = cell_x + glyph_uv.offset_x;
                let glyph_y = baseline_y - (glyph_uv.height + glyph_uv.offset_y);

                // Convert to NDC coordinates
                let ndc_x = (glyph_x / screen_width as f32) * 2.0 - 1.0;
                let ndc_y = -((glyph_y / screen_height as f32) * 2.0 - 1.0);
                
                let ndc_width = (glyph_uv.width / screen_width as f32) * 2.0;
                let ndc_height = -((glyph_uv.height / screen_height as f32) * 2.0);

                // Create instance
                instances.push(GlyphInstance {
                    position: [ndc_x, ndc_y],
                    size: [ndc_width, ndc_height],
                    uv_min: [glyph_uv.u_min, glyph_uv.v_min],
                    uv_max: [glyph_uv.u_max, glyph_uv.v_max],
                    color: [
                        fg_r as f32 / 255.0,
                        fg_g as f32 / 255.0,
                        fg_b as f32 / 255.0,
                        1.0,
                    ],
                    bg_color: [
                        bg_r as f32 / 255.0,
                        bg_g as f32 / 255.0,
                        bg_b as f32 / 255.0,
                        1.0,
                    ],
                    flags,
                    _padding: [0, 0, 0],
                });
            }
        }

        // Validate instance count against capacity before assignment
        let instances_len = instances.len();
        
        // Check if buffer needs resizing
        if instances_len > self.instance_capacity {
            // Double capacity to avoid frequent reallocations, but cap at MAX_INSTANCE_CAPACITY
            let new_capacity = std::cmp::min(
                MAX_INSTANCE_CAPACITY,
                std::cmp::max(
                    instances_len,
                    self.instance_capacity * 2
                )
            );
            
            log::info!(
                "Resizing instance buffer from {} to {} glyphs (required: {})",
                self.instance_capacity,
                new_capacity,
                self.instance_count
            );
            
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Glyph Instance Buffer"),
                size: (new_capacity * std::mem::size_of::<GlyphInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            
            self.instance_capacity = new_capacity;
        }

        // Clamp instances to fit within buffer capacity to prevent overflow
        let safe_instance_count = instances_len.min(self.instance_capacity);
        
        if safe_instance_count < instances_len {
            log::warn!(
                "Truncating {} instances to capacity {} to prevent buffer overflow",
                instances_len,
                self.instance_capacity
            );
        }
        
        // Update instance count with validated value
        self.instance_count = safe_instance_count;

        // Upload instances to GPU with bounds checking
        if safe_instance_count > 0 {
            let safe_instances = &instances[..safe_instance_count];
            let buffer_size = (self.instance_capacity * std::mem::size_of::<GlyphInstance>()) as u64;
            let write_size = (safe_instance_count * std::mem::size_of::<GlyphInstance>()) as u64;
            
            // Double-check write size doesn't exceed buffer size
            if write_size <= buffer_size {
                queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(safe_instances));
            } else {
                log::error!(
                    "Buffer write would overflow: write_size={} > buffer_size={}",
                    write_size,
                    buffer_size
                );
                return Err(anyhow::anyhow!("Instance buffer overflow prevented"));
            }
        }

        log::debug!("Generated {} glyph instances", self.instance_count);

        Ok(())
    }

    /// Render glyphs
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, atlas: &'a GlyphAtlas) {
        if self.instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &atlas.bind_group, &[]);
        render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..self.instance_count as u32);
    }

    /// Get current instance count
    pub fn instance_count(&self) -> usize {
        self.instance_count
    }
}
