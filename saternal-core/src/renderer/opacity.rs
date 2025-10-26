use wgpu;
use wgpu::util::DeviceExt;

/// Manages opacity-related uniforms for wallpaper and background rendering
///
/// This module provides a clean interface for controlling:
/// - Wallpaper opacity (how visible the wallpaper is)
/// - Background opacity (overall window transparency)
/// - Wallpaper presence flag (for shader branching)
pub struct OpacityUniforms {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,

    // Cached values for comparison
    wallpaper_opacity: f32,
    background_opacity: f32,
    has_wallpaper: bool,
}

/// Uniform data structure matching shader layout
/// Must be 16-byte aligned for WGSL uniform buffer requirements
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct OpacityUniformsData {
    wallpaper_opacity: f32,
    background_opacity: f32,
    has_wallpaper: u32,
    _padding: f32, // Ensure 16-byte alignment
}

unsafe impl bytemuck::Pod for OpacityUniformsData {}
unsafe impl bytemuck::Zeroable for OpacityUniformsData {}

impl OpacityUniforms {
    /// Create new opacity uniforms with default values
    pub fn new(
        device: &wgpu::Device,
        wallpaper_opacity: f32,
        background_opacity: f32,
        has_wallpaper: bool,
    ) -> Self {
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Opacity Uniforms Bind Group Layout"),
        });

        // Create uniform buffer with initial data
        let data = OpacityUniformsData {
            wallpaper_opacity,
            background_opacity,
            has_wallpaper: if has_wallpaper { 1 } else { 0 },
            _padding: 0.0,
        };

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Opacity Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[data]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("Opacity Uniforms Bind Group"),
        });

        Self {
            buffer,
            bind_group,
            bind_group_layout,
            wallpaper_opacity,
            background_opacity,
            has_wallpaper,
        }
    }

    /// Update opacity values (only uploads to GPU if values changed)
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        wallpaper_opacity: f32,
        background_opacity: f32,
        has_wallpaper: bool,
    ) {
        // Only update if values changed (avoid unnecessary GPU uploads)
        if self.wallpaper_opacity == wallpaper_opacity
            && self.background_opacity == background_opacity
            && self.has_wallpaper == has_wallpaper
        {
            return;
        }

        self.wallpaper_opacity = wallpaper_opacity;
        self.background_opacity = background_opacity;
        self.has_wallpaper = has_wallpaper;

        let data = OpacityUniformsData {
            wallpaper_opacity,
            background_opacity,
            has_wallpaper: if has_wallpaper { 1 } else { 0 },
            _padding: 0.0,
        };

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[data]));
    }

    /// Get the bind group layout (needed for pipeline creation)
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the bind group (needed for render pass)
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get current wallpaper opacity value
    pub fn wallpaper_opacity(&self) -> f32 {
        self.wallpaper_opacity
    }

    /// Get current background opacity value
    pub fn background_opacity(&self) -> f32 {
        self.background_opacity
    }

    /// Check if wallpaper is currently enabled
    pub fn has_wallpaper(&self) -> bool {
        self.has_wallpaper
    }
}
