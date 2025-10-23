use anyhow::Result;
use fontdue::{Font, FontSettings};
use log::info;
use std::collections::HashMap;

/// Manages font loading and glyph rasterization
pub struct FontManager {
    font: Font,
    font_size: f32,
    /// Cache of rasterized glyphs: (char, size) -> (width, height, bitmap)
    glyph_cache: HashMap<(char, u32), (usize, usize, Vec<u8>)>,
}

impl FontManager {
    /// Load a font from the system
    pub fn new(font_family: &str, font_size: f32) -> Result<Self> {
        info!("Loading font: {} at size {}", font_family, font_size);

        // For now, load a default monospace font
        // In production, we'd search system fonts
        let font_data = Self::load_default_font()?;

        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        Ok(Self {
            font,
            font_size,
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

    /// Get or rasterize a glyph
    pub fn get_glyph(&mut self, ch: char) -> Result<&(usize, usize, Vec<u8>)> {
        let size_key = (self.font_size * 2.0) as u32; // Scale for retina

        if !self.glyph_cache.contains_key(&(ch, size_key)) {
            let (metrics, bitmap) = self.font.rasterize(ch, self.font_size);

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

    /// Get font size
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Update font size and clear cache
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size;
        self.glyph_cache.clear();
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
        self.font.rasterize(ch, self.font_size)
    }
}
