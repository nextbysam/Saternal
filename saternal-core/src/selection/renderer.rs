/// GPU-accelerated selection highlight rendering
use super::range::SelectionRange;
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::term::cell::Cell;
use wgpu;
use wgpu::util::DeviceExt;

/// Single selection span (highlight rectangle)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct SelectionSpan {
    position: [f32; 2],  // NDC position
    size: [f32; 2],      // NDC size
}

unsafe impl bytemuck::Pod for SelectionSpan {}
unsafe impl bytemuck::Zeroable for SelectionSpan {}

/// Selection uniform data (matches shader layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct SelectionUniforms {
    spans: [SelectionSpan; 64],  // Up to 64 spans
    count: u32,
    color: [f32; 4],
    _padding: [u32; 3],
}

unsafe impl bytemuck::Pod for SelectionUniforms {}
unsafe impl bytemuck::Zeroable for SelectionUniforms {}

/// Selection highlight renderer
pub struct SelectionRenderer {
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    current_uniforms: SelectionUniforms,
    dirty: bool,
}

impl SelectionRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Selection Bind Group Layout"),
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

        let initial_uniforms = SelectionUniforms {
            spans: [SelectionSpan { position: [0.0, 0.0], size: [0.0, 0.0] }; 64],
            count: 0,
            color: [0.3, 0.5, 0.8, 0.3],  // Semi-transparent blue
            _padding: [0, 0, 0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Selection Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Selection Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline = create_selection_pipeline(device, &bind_group_layout, surface_format);

        Self {
            uniform_buffer,
            bind_group,
            bind_group_layout,
            pipeline,
            current_uniforms: initial_uniforms,
            dirty: false,
        }
    }

    /// Update selection spans from grid range
    pub fn update(
        &mut self,
        range: Option<SelectionRange>,
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
        grid_cols: usize,
    ) {
        if let Some(range) = range {
            let spans = self.range_to_spans(range, cell_width, cell_height, window_width, window_height, grid_cols);
            self.current_uniforms.count = spans.len() as u32;
            for (i, span) in spans.iter().enumerate() {
                if i < 64 {
                    self.current_uniforms.spans[i] = *span;
                }
            }
            self.dirty = true;
        } else {
            self.current_uniforms.count = 0;
            self.dirty = true;
        }
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

    /// Check if selection should be rendered
    pub fn has_selection(&self) -> bool {
        self.current_uniforms.count > 0
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

    /// Convert selection range to NDC spans
    fn range_to_spans(
        &self,
        range: SelectionRange,
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
        grid_cols: usize,
    ) -> Vec<SelectionSpan> {
        let (start, end) = range.normalized();
        let mut spans = Vec::new();

        if start.line == end.line {
            // Single line selection
            let span = self.create_span(
                start.line.0,
                start.column.0,
                end.column.0 - start.column.0 + 1,
                cell_width,
                cell_height,
                window_width,
                window_height,
            );
            spans.push(span);
        } else {
            // Multi-line selection
            // First line (from start to end of line)
            let first_span = self.create_span(
                start.line.0,
                start.column.0,
                grid_cols - start.column.0,
                cell_width,
                cell_height,
                window_width,
                window_height,
            );
            spans.push(first_span);

            // Middle lines (full width)
            for line in (start.line.0 + 1)..end.line.0 {
                let span = self.create_span(
                    line,
                    0,
                    grid_cols,
                    cell_width,
                    cell_height,
                    window_width,
                    window_height,
                );
                spans.push(span);
            }

            // Last line (from start of line to end)
            let last_span = self.create_span(
                end.line.0,
                0,
                end.column.0 + 1,
                cell_width,
                cell_height,
                window_width,
                window_height,
            );
            spans.push(last_span);
        }

        spans
    }

    /// Create a single span in NDC coordinates
    #[inline]
    fn create_span(
        &self,
        line: usize,
        col: usize,
        width_cells: usize,
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
    ) -> SelectionSpan {
        let pixel_x = col as f32 * cell_width;
        let pixel_y = line as f32 * cell_height;
        let pixel_width = width_cells as f32 * cell_width;

        // Convert to NDC
        let ndc_x = (pixel_x / window_width as f32) * 2.0 - 1.0;
        let ndc_y = -((pixel_y / window_height as f32) * 2.0 - 1.0);
        let ndc_width = (pixel_width / window_width as f32) * 2.0;
        let ndc_height = -((cell_height / window_height as f32) * 2.0);

        SelectionSpan {
            position: [ndc_x, ndc_y],
            size: [ndc_width, ndc_height],
        }
    }
}

/// Create selection render pipeline
fn create_selection_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Selection Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/selection.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Selection Pipeline Layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Selection Pipeline"),
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
