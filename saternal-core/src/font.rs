use anyhow::Result;
use fontdue::{Font, FontSettings};
use log::info;
use std::collections::HashMap;

/// Manages font loading and glyph rasterization
pub struct FontManager {
    font: Font,
    configured_font_size: f32,      // Logical size from config
    current_scale_factor: f64,       // Current DPI scale (1.0, 2.0, etc.)
    /// Cache of rasterized glyphs: (char, size) -> (width, height, bitmap)
    glyph_cache: HashMap<(char, u32), (usize, usize, Vec<u8>)>,
}

impl FontManager {
    /// Load a font from the system
    pub fn new(font_family: &str, font_size: f32) -> Result<Self> {
        Self::new_with_scale(font_family, font_size, 1.0)
    }

    /// Load a font with explicit scale factor (DPI-aware)
    pub fn new_with_scale(font_family: &str, font_size: f32, scale_factor: f64) -> Result<Self> {
        info!("Loading font: {} at size {} (scale: {}x)", font_family, font_size, scale_factor);

        // For now, load a default monospace font
        // In production, we'd search system fonts
        let font_data = Self::load_default_font()?;

        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        Ok(Self {
            font,
            configured_font_size: font_size,
            current_scale_factor: scale_factor,
            glyph_cache: HashMap::new(),
        })
    }

    /// Load default monospace font
    fn load_default_font() -> Result<Vec<u8>> {
        // Try to load JetBrains Mono or fallback to system fonts
        let font_paths = vec![
            "/System/Library/Fonts/Monaco.ttf",
            "/System/Library/Fonts/Menlo.ttc",
            "/Library/Fonts/SF-Mono-Regular.otf",
        ];

        for path in font_paths {
            if let Ok(data) = std::fs::read(path) {
                info!("Loaded font from: {}", path);
                return Ok(data);
            }
        }

        anyhow::bail!("Could not find any monospace font")
    }

    /// Get effective font size (logical size * DPI scale)
    pub fn effective_font_size(&self) -> f32 {
        (self.configured_font_size * self.current_scale_factor as f32)
    }

    /// Update DPI scale factor and clear cache if changed
    pub fn update_scale_factor(&mut self, scale_factor: f64) {
        if (self.current_scale_factor - scale_factor).abs() > 0.001 {
            info!("DPI scale factor changed: {:.2}x -> {:.2}x", 
                  self.current_scale_factor, scale_factor);
            self.current_scale_factor = scale_factor;
            self.glyph_cache.clear();
        }
    }

    /// Get or rasterize a glyph
    pub fn get_glyph(&mut self, ch: char) -> Result<&(usize, usize, Vec<u8>)> {
        let effective_size = self.effective_font_size();
        let size_key = effective_size.round() as u32;

        if !self.glyph_cache.contains_key(&(ch, size_key)) {
            let (metrics, bitmap) = self.font.rasterize(ch, effective_size);

            // Convert grayscale to RGBA
            let mut rgba_bitmap = Vec::with_capacity(bitmap.len() * 4);
            for &alpha in &bitmap {
                rgba_bitmap.extend_from_slice(&[255, 255, 255, alpha]);
            }

            self.glyph_cache
                .insert((ch, size_key), (metrics.width, metrics.height, rgba_bitmap));
        }

        Ok(self.glyph_cache.get(&(ch, size_key)).unwrap())
    }

    /// Get the cell dimensions (width, height) for this font
    pub fn cell_dimensions(&mut self) -> (usize, usize) {
        // Use 'M' as reference character for cell size
        if let Ok((width, height, _)) = self.get_glyph('M') {
            (*width, *height)
        } else {
            (8, 16) // Fallback dimensions
        }
    }

    /// Get configured (logical) font size
    pub fn font_size(&self) -> f32 {
        self.configured_font_size
    }

    /// Update font size and clear cache
    pub fn set_font_size(&mut self, size: f32) {
        self.configured_font_size = size;
        self.glyph_cache.clear();
    }

    /// Get current scale factor
    pub fn scale_factor(&self) -> f64 {
        self.current_scale_factor
    }

    /// Clear glyph cache (useful for memory management)
    pub fn clear_cache(&mut self) {
        self.glyph_cache.clear();
    }

    /// Get reference to the underlying font
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Rasterize a glyph (returns metrics and grayscale bitmap)
    pub fn rasterize(&self, ch: char) -> (fontdue::Metrics, Vec<u8>) {
        self.font.rasterize(ch, self.effective_font_size())
    }
}
