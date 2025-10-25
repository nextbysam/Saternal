/// GPU-accelerated pane border rendering
/// Generates border rectangles for rendering with the border shader
use crate::selection::renderer::PaneViewport;
use wgpu;
use wgpu::util::DeviceExt;

/// Single border rectangle in NDC coordinates
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct BorderRect {
    pub position: [f32; 2],  // NDC position
    pub size: [f32; 2],      // NDC size
}

unsafe impl bytemuck::Pod for BorderRect {}
unsafe impl bytemuck::Zeroable for BorderRect {}

/// Padded viewport ID for proper alignment (must be 16-byte aligned in uniform buffers)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ViewportId {
    pub id: u32,                  // Pane ID (4 bytes)
    pub _padding: [u32; 3],       // Padding to 16 bytes (12 bytes)
}

unsafe impl bytemuck::Pod for ViewportId {}
unsafe impl bytemuck::Zeroable for ViewportId {}

/// Border uniform data (matches shader layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct BorderUniforms {
    pub rects: [BorderRect; 32],        // Up to 32 border rectangles (512 bytes)
    pub count: u32,                      // Number of active borders (4 bytes)
    pub thickness: f32,                  // Border thickness in pixels (4 bytes)
    pub _padding1: [u32; 2],             // Padding (8 bytes)
    pub active_color: [f32; 4],          // RGBA color for focused pane (16 bytes)
    pub inactive_color: [f32; 4],        // RGBA color for unfocused panes (16 bytes)
    pub viewport_ids: [ViewportId; 32],  // Pane IDs with padding (512 bytes)
    pub focused_id: u32,                 // ID of focused pane (4 bytes)
    pub _padding2: [u32; 3],             // Final padding (12 bytes)
}

unsafe impl bytemuck::Pod for BorderUniforms {}
unsafe impl bytemuck::Zeroable for BorderUniforms {}

/// Border configuration
#[derive(Debug, Clone)]
pub struct BorderConfig {
    pub enabled: bool,
    pub thickness: u32,
    pub active_color: [f32; 4],    // RGBA
    pub inactive_color: [f32; 4],
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thickness: 2,
            active_color: [0.29, 0.56, 0.89, 1.0],   // #4A90E2 blue
            inactive_color: [0.24, 0.24, 0.24, 1.0], // #3C3C3C gray
        }
    }
}

/// Border renderer for panes
pub struct BorderRenderer {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    config: BorderConfig,
    current_uniforms: BorderUniforms,
    dirty: bool,
}

impl BorderRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let config = BorderConfig::default();

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Border Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let initial_uniforms = BorderUniforms {
            rects: [BorderRect { position: [0.0, 0.0], size: [0.0, 0.0] }; 32],
            count: 0,
            thickness: config.thickness as f32,
            _padding1: [0, 0],
            active_color: config.active_color,
            inactive_color: config.inactive_color,
            viewport_ids: [ViewportId { id: 0, _padding: [0, 0, 0] }; 32],
            focused_id: 0,
            _padding2: [0, 0, 0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Border Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Border Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline = create_border_pipeline(device, &bind_group_layout, surface_format);

        Self {
            uniform_buffer,
            bind_group,
            bind_group_layout,
            pipeline,
            config,
            current_uniforms: initial_uniforms,
            dirty: false,
        }
    }

    /// Update border rectangles from pane viewports
    pub fn update(&mut self, viewports: &[PaneViewport], window_width: u32, window_height: u32) {
        if viewports.is_empty() {
            self.current_uniforms.count = 0;
            self.dirty = true;
            return;
        }

        // Find focused pane ID
        let focused_id = viewports
            .iter()
            .find(|vp| vp.focused)
            .map(|vp| vp.pane_id as u32)
            .unwrap_or(0);

        // Generate border rectangles (4 per pane: top, bottom, left, right)
        let mut rect_index = 0;
        let thickness = self.config.thickness as f32;

        for viewport in viewports {
            let rects = generate_viewport_borders(
                viewport,
                thickness,
                window_width,
                window_height,
            );

            // Add all 4 border rectangles for this viewport
            for rect in rects {
                if rect_index < 32 {
                    self.current_uniforms.rects[rect_index] = rect;
                    self.current_uniforms.viewport_ids[rect_index] = viewport.pane_id as u32;
                    rect_index += 1;
                }
            }
        }

        self.current_uniforms.count = rect_index as u32;
        self.current_uniforms.focused_id = focused_id;
        self.current_uniforms.thickness = thickness;
        self.dirty = true;
    }

    /// Upload uniforms to GPU
    pub fn upload_uniforms(&mut self, queue: &wgpu::Queue) {
        if self.dirty {
            queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.current_uniforms]),
            );
            self.dirty = false;
        }
    }

    /// Check if borders should be rendered
    pub fn has_borders(&self) -> bool {
        self.config.enabled && self.current_uniforms.count > 0
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get the pipeline
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Get instance count for rendering
    pub fn instance_count(&self) -> u32 {
        self.current_uniforms.count
    }
}

/// Generate 4 border rectangles for a viewport (top, bottom, left, right)
fn generate_viewport_borders(
    viewport: &PaneViewport,
    thickness: f32,
    window_width: u32,
    window_height: u32,
) -> [BorderRect; 4] {
    let x = viewport.x as f32;
    let y = viewport.y as f32;
    let w = viewport.width as f32;
    let h = viewport.height as f32;

    // Convert pixel coordinates to NDC
    let to_ndc_x = |px: f32| (px / window_width as f32) * 2.0 - 1.0;
    let to_ndc_y = |py: f32| -((py / window_height as f32) * 2.0 - 1.0);
    let to_ndc_width = |pw: f32| (pw / window_width as f32) * 2.0;
    let to_ndc_height = |ph: f32| -((ph / window_height as f32) * 2.0);

    [
        // Top border
        BorderRect {
            position: [to_ndc_x(x), to_ndc_y(y)],
            size: [to_ndc_width(w), to_ndc_height(thickness)],
        },
        // Bottom border
        BorderRect {
            position: [to_ndc_x(x), to_ndc_y(y + h - thickness)],
            size: [to_ndc_width(w), to_ndc_height(thickness)],
        },
        // Left border
        BorderRect {
            position: [to_ndc_x(x), to_ndc_y(y)],
            size: [to_ndc_width(thickness), to_ndc_height(h)],
        },
        // Right border
        BorderRect {
            position: [to_ndc_x(x + w - thickness), to_ndc_y(y)],
            size: [to_ndc_width(thickness), to_ndc_height(h)],
        },
    ]
}

/// Create border render pipeline
fn create_border_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Border Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/border.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Border Pipeline Layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Border Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
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
    })
}
