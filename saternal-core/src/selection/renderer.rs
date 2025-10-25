/// GPU-accelerated selection highlight rendering and pane border rendering
use super::range::SelectionRange;
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::term::cell::Cell;
use crate::pane::PaneNode;
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

/// Viewport for rendering a single pane
#[derive(Debug, Clone)]
pub struct PaneViewport {
    pub pane_id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

/// Calculate viewports for all panes in the tree
pub fn calculate_pane_viewports(
    pane_tree: &PaneNode,
    window_width: u32,
    window_height: u32,
) -> Vec<PaneViewport> {
    let mut viewports = Vec::new();
    calculate_viewports_recursive(
        pane_tree,
        0, 0,
        window_width, window_height,
        &mut viewports
    );
    viewports
}

fn calculate_viewports_recursive(
    node: &PaneNode,
    x: u32, y: u32,
    width: u32, height: u32,
    viewports: &mut Vec<PaneViewport>
) {
    use crate::pane::{PaneNode as PN, SplitDirection};
    
    match node {
        PN::Leaf { pane } => {
            viewports.push(PaneViewport {
                pane_id: pane.id,
                x, y, width, height,
                focused: pane.focused,
            });
        }
        PN::Split { direction, children, ratio } => {
            const BORDER_WIDTH: u32 = 2;

            match direction {
                SplitDirection::Horizontal => {
                    // Top/bottom split
                    let split_y = (height as f32 * ratio) as u32;

                    if let Some(top) = children.get(0) {
                        calculate_viewports_recursive(
                            top,
                            x, y,
                            width,
                            split_y.saturating_sub(BORDER_WIDTH / 2),
                            viewports
                        );
                    }

                    if let Some(bottom) = children.get(1) {
                        calculate_viewports_recursive(
                            bottom,
                            x,
                            y + split_y + BORDER_WIDTH / 2,
                            width,
                            height.saturating_sub(split_y + BORDER_WIDTH),
                            viewports
                        );
                    }
                }
                SplitDirection::Vertical => {
                    // Left/right split
                    let split_x = (width as f32 * ratio) as u32;

                    if let Some(left) = children.get(0) {
                        calculate_viewports_recursive(
                            left,
                            x, y,
                            split_x.saturating_sub(BORDER_WIDTH / 2),
                            height,
                            viewports
                        );
                    }

                    if let Some(right) = children.get(1) {
                        calculate_viewports_recursive(
                            right,
                            x + split_x + BORDER_WIDTH / 2,
                            y,
                            width.saturating_sub(split_x + BORDER_WIDTH),
                            height,
                            viewports
                        );
                    }
                }
            }
        }
    }
}

/// Selection uniform data (matches shader layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct SelectionUniforms {
    spans: [SelectionSpan; 64],  // Up to 64 spans (1024 bytes)
    count: u32,                   // 4 bytes
    _padding1: [u32; 7],          // 28 bytes padding to match std140 layout (vec3<u32> alignment + vec4<f32> alignment)
    color: [f32; 4],              // 16 bytes (vec4<f32>)
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
            _padding1: [0, 0, 0, 0, 0, 0, 0],
            color: [0.3, 0.5, 0.8, 0.3],  // Semi-transparent blue
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
        grid_lines: usize,
    ) {
        if let Some(range) = range {
            let spans = self.range_to_spans(range, cell_width, cell_height, window_width, window_height, grid_cols, grid_lines);
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
        grid_lines: usize,
    ) -> Vec<SelectionSpan> {
        let (start, end) = range.normalized();
        let mut spans = Vec::new();
        
        // Clamp to grid bounds
        let max_col = grid_cols.saturating_sub(1);
        let max_line = (grid_lines as i32).saturating_sub(1);
        let start_col = start.column.0.min(max_col);
        let end_col = end.column.0.min(max_col);
        let start_line = start.line.0.max(0).min(max_line);
        let end_line = end.line.0.max(0).min(max_line);

        if start_line == end_line {
            // Single line selection
            let width = end_col.saturating_sub(start_col) + 1;
            let span = self.create_span(
                start_line as usize,
                start_col,
                width,
                cell_width,
                cell_height,
                window_width,
                window_height,
            );
            spans.push(span);
        } else {
            // Multi-line selection
            // First line (from start to end of line)
            let first_width = grid_cols.saturating_sub(start_col);
            let first_span = self.create_span(
                start_line as usize,
                start_col,
                first_width,
                cell_width,
                cell_height,
                window_width,
                window_height,
            );
            spans.push(first_span);

            // Middle lines (full width)
            for line in (start_line + 1)..end_line {
                let span = self.create_span(
                    line as usize,
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
            let last_width = (end_col + 1).min(grid_cols);
            let last_span = self.create_span(
                end_line as usize,
                0,
                last_width,
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
        // Padding constants (must match TextRasterizer padding)
        const PADDING_LEFT: f32 = 10.0;
        const PADDING_TOP: f32 = 5.0;
        
        let pixel_x = PADDING_LEFT + col as f32 * cell_width;
        let pixel_y = PADDING_TOP + line as f32 * cell_height;
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
