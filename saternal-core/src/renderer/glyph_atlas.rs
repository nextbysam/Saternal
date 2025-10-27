use crate::font::FontManager;
use anyhow::Result;
use std::collections::HashMap;
use wgpu;

/// UV coordinates for a glyph in the atlas texture
#[derive(Debug, Clone, Copy)]
pub struct GlyphUV {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
    pub width: f32,   // Glyph width in pixels
    pub height: f32,  // Glyph height in pixels
    pub offset_x: f32, // Horizontal bearing
    pub offset_y: f32, // Vertical bearing (distance from baseline)
}

/// Manages a texture atlas of pre-rasterized glyphs
pub struct GlyphAtlas {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    
    /// Map from character to UV coordinates
    glyph_map: HashMap<char, GlyphUV>,
    
    /// Atlas dimensions
    atlas_width: u32,
    atlas_height: u32,
    
    /// Current packing position
    pack_x: u32,
    pack_y: u32,
    row_height: u32,
}

impl GlyphAtlas {
    /// Create a new glyph atlas with pre-generated common characters
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_manager: &FontManager,
        atlas_size: u32,
    ) -> Result<Self> {
        // Create atlas texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Texture"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // Single channel for grayscale
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Glyph Atlas Bind Group Layout"),
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
            label: Some("Glyph Atlas Bind Group"),
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
        });

        let mut atlas = Self {
            texture,
            view,
            sampler,
            bind_group_layout,
            bind_group,
            glyph_map: HashMap::new(),
            atlas_width: atlas_size,
            atlas_height: atlas_size,
            pack_x: 0,
            pack_y: 0,
            row_height: 0,
        };

        // Pre-generate common ASCII characters
        atlas.generate_ascii_set(device, queue, font_manager)?;

        log::info!("Created glyph atlas {}x{} with {} glyphs", 
                   atlas_size, atlas_size, atlas.glyph_map.len());

        Ok(atlas)
    }

    /// Generate all printable ASCII characters
    fn generate_ascii_set(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_manager: &FontManager,
    ) -> Result<()> {
        // Printable ASCII (space to ~)
        for c in 0x20u8..=0x7E {
            self.add_glyph(device, queue, font_manager, c as char)?;
        }

        // Common extended characters
        let extended = "€£¥©®™°±×÷";
        for c in extended.chars() {
            let _ = self.add_glyph(device, queue, font_manager, c);
        }

        Ok(())
    }

    /// Add a single glyph to the atlas
    pub fn add_glyph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_manager: &FontManager,
        c: char,
    ) -> Result<()> {
        // Skip if already in atlas
        if self.glyph_map.contains_key(&c) {
            return Ok(());
        }

        // Rasterize glyph
        let (metrics, bitmap) = font_manager.rasterize(c);
        
        let glyph_width = metrics.width as u32;
        let glyph_height = metrics.height as u32;

        // Check if we need to move to next row
        if self.pack_x + glyph_width + 2 > self.atlas_width {
            self.pack_x = 0;
            self.pack_y += self.row_height + 2; // 2px padding
            self.row_height = 0;
        }

        // Check if atlas is full
        if self.pack_y + glyph_height + 2 > self.atlas_height {
            anyhow::bail!("Glyph atlas is full, cannot add '{}'", c);
        }

        // Upload glyph bitmap to atlas
        if !bitmap.is_empty() {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: self.pack_x,
                        y: self.pack_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &bitmap,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(glyph_width),
                    rows_per_image: Some(glyph_height),
                },
                wgpu::Extent3d {
                    width: glyph_width,
                    height: glyph_height,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Calculate UV coordinates
        let u_min = self.pack_x as f32 / self.atlas_width as f32;
        let v_min = self.pack_y as f32 / self.atlas_height as f32;
        let u_max = (self.pack_x + glyph_width) as f32 / self.atlas_width as f32;
        let v_max = (self.pack_y + glyph_height) as f32 / self.atlas_height as f32;

        // Store UV mapping with metrics
        self.glyph_map.insert(c, GlyphUV {
            u_min,
            v_min,
            u_max,
            v_max,
            width: metrics.width as f32,
            height: metrics.height as f32,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
        });

        // Update packing position
        self.pack_x += glyph_width + 2; // 2px padding
        self.row_height = self.row_height.max(glyph_height);

        Ok(())
    }

    /// Get UV coordinates for a character
    pub fn get_glyph(&self, c: char) -> Option<&GlyphUV> {
        self.glyph_map.get(&c)
    }

    /// Get or add a glyph (lazy loading)
    pub fn get_or_add_glyph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        font_manager: &FontManager,
        c: char,
    ) -> Result<&GlyphUV> {
        if !self.glyph_map.contains_key(&c) {
            self.add_glyph(device, queue, font_manager, c)?;
        }
        self.glyph_map.get(&c)
            .ok_or_else(|| anyhow::anyhow!("Glyph '{}' not found in atlas after add attempt", c))
    }

    /// Get atlas dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.atlas_width, self.atlas_height)
    }

    /// Get number of cached glyphs
    pub fn glyph_count(&self) -> usize {
        self.glyph_map.len()
    }
}
