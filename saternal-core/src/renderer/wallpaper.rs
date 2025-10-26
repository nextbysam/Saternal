use anyhow::{Context, Result};
use std::path::Path;
use wgpu;

/// Manages wallpaper texture and GPU resources
///
/// This module handles:
/// - Loading images from disk (PNG, JPG, WEBP)
/// - Creating GPU textures and bind groups
/// - Providing a dummy fallback texture when no wallpaper is set
pub struct WallpaperManager {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    has_wallpaper: bool,
}

impl WallpaperManager {
    /// Create a new wallpaper manager with a dummy 1x1 transparent texture
    pub fn new(device: &wgpu::Device) -> Self {
        // Create bind group layout (same for dummy and real wallpapers)
        let bind_group_layout = Self::create_bind_group_layout(device);

        // Create dummy 1x1 transparent texture
        let (texture, view) = Self::create_dummy_texture(device);

        // Create sampler (linear filtering for smooth scaling)
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group
        let bind_group = Self::create_bind_group(device, &bind_group_layout, &view, &sampler);

        Self {
            texture,
            view,
            sampler,
            bind_group,
            bind_group_layout,
            has_wallpaper: false,
        }
    }

    /// Load a wallpaper image from a file path
    pub fn load(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &str) -> Result<()> {
        // Expand tilde to home directory
        let expanded_path = if path.starts_with('~') {
            if let Some(home) = std::env::var_os("HOME") {
                let mut home_path = std::path::PathBuf::from(home);
                home_path.push(&path[2..]); // Skip "~/"
                home_path.to_string_lossy().to_string()
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        log::info!("Loading wallpaper from: {}", expanded_path);

        // Load and decode image
        let img = image::open(Path::new(&expanded_path))
            .context(format!("Failed to open wallpaper image: {}", expanded_path))?;

        // Convert to RGBA8
        let rgba = img.to_rgba8();
        let dimensions = rgba.dimensions();

        log::info!(
            "Wallpaper loaded: {}x{} pixels ({} bytes)",
            dimensions.0,
            dimensions.1,
            rgba.len()
        );

        // Create texture
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Wallpaper Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload image data to GPU with proper alignment
        // wgpu requires bytes_per_row to be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256 bytes)
        const ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bytes_per_row = 4 * dimensions.0;
        let padded_bytes_per_row = (unpadded_bytes_per_row + ALIGNMENT - 1) / ALIGNMENT * ALIGNMENT;

        if unpadded_bytes_per_row == padded_bytes_per_row {
            // No padding needed - image width naturally aligns
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                size,
            );
        } else {
            // Padding required - create aligned buffer
            let padded_size = (padded_bytes_per_row * dimensions.1) as usize;
            let mut padded_data = vec![0u8; padded_size];
            let rgba_bytes = rgba.as_raw();

            // Copy each row with padding
            for y in 0..dimensions.1 {
                let src_offset = (y * unpadded_bytes_per_row) as usize;
                let dst_offset = (y * padded_bytes_per_row) as usize;
                padded_data[dst_offset..dst_offset + unpadded_bytes_per_row as usize]
                    .copy_from_slice(&rgba_bytes[src_offset..src_offset + unpadded_bytes_per_row as usize]);
            }

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &padded_data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                size,
            );
        }

        // Create view
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update bind group with new texture
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &view,
            &self.sampler,
        );

        // Update state
        self.texture = texture;
        self.view = view;
        self.has_wallpaper = true;

        log::info!("Wallpaper loaded successfully");
        Ok(())
    }

    /// Clear wallpaper and reset to dummy texture
    pub fn clear(&mut self, device: &wgpu::Device) {
        log::info!("Clearing wallpaper");

        // Create dummy texture
        let (texture, view) = Self::create_dummy_texture(device);

        // Update bind group
        self.bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &view,
            &self.sampler,
        );

        // Update state
        self.texture = texture;
        self.view = view;
        self.has_wallpaper = false;

        log::info!("Wallpaper cleared");
    }

    /// Check if a wallpaper is currently loaded
    pub fn has_wallpaper(&self) -> bool {
        self.has_wallpaper
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get the bind group layout for pipeline creation
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Create bind group layout (shared by all wallpaper textures)
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Wallpaper Bind Group Layout"),
        })
    }

    /// Create bind group for a texture view and sampler
    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
            label: Some("Wallpaper Bind Group"),
        })
    }

    /// Create a 1x1 transparent dummy texture
    ///
    /// This ensures we always have a valid texture bound, even when no wallpaper is set.
    /// This avoids null checks and conditional binding in the render pipeline.
    fn create_dummy_texture(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Wallpaper Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }
}
