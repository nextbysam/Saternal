use anyhow::Result;
use image::GenericImageView;
use log::{debug, info};
use wgpu;

/// Manages wallpaper texture loading and GPU resources
///
/// This module provides a clean interface for:
/// - Loading wallpaper images from disk (PNG, JPG, WEBP)
/// - Managing GPU texture resources
/// - Providing fallback dummy texture when no wallpaper is loaded
pub struct WallpaperManager {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    has_wallpaper: bool,
}

impl WallpaperManager {
    /// Create a new wallpaper manager with a dummy 1x1 transparent texture
    /// This ensures we always have valid textures to bind, even when no wallpaper is loaded
    pub fn new(device: &wgpu::Device) -> Self {
        // Create bind group layout for wallpaper texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // Wallpaper texture
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
                // Wallpaper sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Wallpaper Bind Group Layout"),
        });

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

        // Create dummy 1x1 transparent texture
        let (texture, view) = Self::create_dummy_texture(device);

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("Wallpaper Bind Group"),
        });

        Self {
            texture,
            view,
            sampler,
            bind_group_layout,
            bind_group,
            has_wallpaper: false,
        }
    }

    /// Load a wallpaper from a file path
    pub fn load(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &str) -> Result<()> {
        info!("Loading wallpaper from: {}", path);

        // Load and decode image
        let img = image::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to load image: {}", e))?;
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();

        debug!("Wallpaper loaded: {}x{}", dimensions.0, dimensions.1);

        // Create GPU texture
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Wallpaper Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload texture data to GPU
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
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Update bind group with new texture
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("Wallpaper Bind Group"),
        });

        self.texture = texture;
        self.view = view;
        self.bind_group = bind_group;
        self.has_wallpaper = true;

        info!("Wallpaper loaded successfully");
        Ok(())
    }

    /// Clear the wallpaper and return to transparent background
    pub fn clear(&mut self, device: &wgpu::Device) {
        info!("Clearing wallpaper");

        // Replace with dummy texture
        let (texture, view) = Self::create_dummy_texture(device);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
            label: Some("Wallpaper Bind Group"),
        });

        self.texture = texture;
        self.view = view;
        self.bind_group = bind_group;
        self.has_wallpaper = false;

        info!("Wallpaper cleared");
    }

    /// Create a 1x1 transparent dummy texture
    fn create_dummy_texture(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy Wallpaper Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
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

    /// Get bind group layout (needed for pipeline creation)
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get bind group (needed for render pass)
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Check if a wallpaper is currently loaded
    pub fn has_wallpaper(&self) -> bool {
        self.has_wallpaper
    }
}
