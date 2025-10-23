use super::config::{CursorConfig, CursorStyle};
use alacritty_terminal::index::Point;
use std::time::{Duration, Instant};
use wgpu;
use wgpu::util::DeviceExt;

/// Cursor uniform data (matches shader layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CursorUniforms {
    position: [f32; 2],      // NDC position
    size: [f32; 2],          // NDC size
    color: [f32; 4],         // RGBA
    visible: u32,            // 0 or 1
    style: u32,              // 0=block, 1=beam, 2=underline
    _padding: [u32; 2],      // Alignment to 16 bytes
}

unsafe impl bytemuck::Pod for CursorUniforms {}
unsafe impl bytemuck::Zeroable for CursorUniforms {}

/// Cursor blinking state
struct BlinkState {
    visible: bool,
    last_toggle: Instant,
    interval: Duration,
}

impl BlinkState {
    fn new(interval_ms: u64) -> Self {
        Self {
            visible: true,
            last_toggle: Instant::now(),
            interval: Duration::from_millis(interval_ms),
        }
    }

    fn update(&mut self) -> bool {
        let elapsed = self.last_toggle.elapsed();
        if elapsed >= self.interval {
            self.visible = !self.visible;
            self.last_toggle = Instant::now();
            true // State changed
        } else {
            false // No change
        }
    }
}

/// Cursor state management
pub struct CursorState {
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    blink_state: BlinkState,
    config: CursorConfig,
    current_uniforms: CursorUniforms,
}

impl CursorState {
    pub fn new(device: &wgpu::Device, config: CursorConfig) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cursor Bind Group Layout"),
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

        let initial_uniforms = CursorUniforms {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: config.color,
            visible: 1,
            style: config.style as u32,
            _padding: [0, 0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cursor Uniform Buffer"),
            contents: bytemuck::cast_slice(&[initial_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cursor Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            uniform_buffer,
            bind_group,
            bind_group_layout,
            blink_state: BlinkState::new(config.blink_interval_ms),
            config,
            current_uniforms: initial_uniforms,
        }
    }

    /// Update cursor blink state
    pub fn update_blink(&mut self) -> bool {
        if self.config.blink {
            self.blink_state.update()
        } else {
            false
        }
    }

    /// Update cursor position and visibility
    pub fn update_position(
        &mut self,
        cursor_pos: Point,
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
        scroll_offset: usize,
        hide_cursor: bool,
    ) {
        // Hide cursor if scrolled or terminal mode requests it
        let should_hide = scroll_offset > 0 || hide_cursor;
        
        // Calculate pixel position
        let pixel_x = cursor_pos.column.0 as f32 * cell_width;
        let pixel_y = cursor_pos.line.0 as f32 * cell_height;

        // Convert to normalized device coordinates (-1 to 1)
        let ndc_x = (pixel_x / window_width as f32) * 2.0 - 1.0;
        let ndc_y = -((pixel_y / window_height as f32) * 2.0 - 1.0); // Flip Y

        // Calculate NDC size based on style
        let (width, height) = match self.config.style {
            CursorStyle::Block => (cell_width, cell_height),
            CursorStyle::Beam => (2.0, cell_height),
            CursorStyle::Underline => (cell_width, 2.0),
        };

        let ndc_width = (width / window_width as f32) * 2.0;
        let ndc_height = (height / window_height as f32) * 2.0;

        // Determine visibility
        let visible = if should_hide {
            0
        } else if self.config.blink {
            self.blink_state.visible as u32
        } else {
            1
        };

        self.current_uniforms = CursorUniforms {
            position: [ndc_x, ndc_y],
            size: [ndc_width, ndc_height],
            color: self.config.color,
            visible,
            style: self.config.style as u32,
            _padding: [0, 0],
        };
    }

    /// Upload uniforms to GPU
    pub fn upload_uniforms(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.current_uniforms]),
        );
    }

    /// Check if cursor should be rendered
    pub fn is_visible(&self) -> bool {
        self.current_uniforms.visible == 1
    }
}
