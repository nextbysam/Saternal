use crate::font::FontManager;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;
use anyhow::Result;
use wgpu;

use super::color::ansi_to_rgb_with_palette;
use super::theme::ColorPalette;

/// Rasterizes terminal text to a pixel buffer for GPU upload
pub(crate) struct TextRasterizer {
    cell_width: f32,
    cell_height: f32,
    baseline_offset: f32,
    padding_left: f32,
    padding_top: f32,
}

impl TextRasterizer {
    /// Create a new text rasterizer with cell dimensions
    pub fn new(cell_width: f32, cell_height: f32, baseline_offset: f32) -> Self {
        // NOTE: Padding values must match App::calculate_terminal_size() in saternal/src/app/state.rs
        // to ensure terminal PTY size accounts for these margins
        Self {
            cell_width,
            cell_height,
            baseline_offset,
            padding_left: 10.0,
            padding_top: 5.0,
        }
    }

    /// Update cell dimensions (called when font size changes)
    pub fn update_dimensions(&mut self, cell_width: f32, cell_height: f32, baseline_offset: f32) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.baseline_offset = baseline_offset;
        // Padding remains the same regardless of font size
    }

    /// Render terminal content to texture buffer
    pub fn render_to_buffer<T>(
        &self,
        term: &Term<T>,
        font_manager: &FontManager,
        width: u32,
        height: u32,
        scroll_offset: usize,
        surface_format: wgpu::TextureFormat,
        palette: &ColorPalette,
    ) -> Result<Vec<u8>> {
        let rows = term.screen_lines();
        let cols = term.columns();
        let cursor = term.grid().cursor.point;
        
        // CRITICAL: Clamp scroll_offset to available history to prevent out-of-bounds access
        // The grid can access lines from -history_size to screen_lines-1
        let history_size = term.grid().history_size();
        let scroll_offset = scroll_offset.min(history_size);
        
        log::info!("Rendering terminal: {}x{} cells, cursor at ({}, {}), scroll_offset={}, history_size={}",
                   cols, rows, cursor.column.0, cursor.line.0, scroll_offset, history_size);

        // Determine if we need BGRA or RGBA based on surface format
        let is_bgra = matches!(
            surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Create buffer for rendering terminal text
        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Render each cell from the terminal grid
        let mut char_count = 0;
        for row_idx in 0..rows {
            // Apply scroll offset: negative Line indices access scrollback
            let line = Line(row_idx as i32 - scroll_offset as i32);
            for col_idx in 0..cols {
                let column = Column(col_idx);
                let cell = &term.grid()[line][column];

                // Get character
                let c = cell.c;

                if c == '\0' || c == ' ' {
                    continue; // Skip null cells and spaces
                }
                char_count += 1;

                // Get colors from palette
                let (fg_r, fg_g, fg_b) = ansi_to_rgb_with_palette(&cell.fg, palette);

                // Rasterize glyph
                let (metrics, bitmap) = font_manager.rasterize(c);

                // Calculate cell position in window coordinates with padding
                let cell_x = self.padding_left + col_idx as f32 * self.cell_width;
                let cell_y = self.padding_top + row_idx as f32 * self.cell_height;

                // Calculate baseline position (from top of cell)
                let baseline_y = cell_y + self.baseline_offset;

                // Calculate glyph position using proper baseline alignment
                let glyph_x = cell_x;
                let glyph_y = baseline_y - (metrics.height as f32 + metrics.ymin as f32);

                if row_idx == 0 && col_idx < 5 {
                    log::debug!("Char '{}' at cell ({}, {}) -> glyph ({:.1}, {:.1}), baseline {:.1}, metrics: h={} ymin={}",
                               c, cell_x, cell_y, glyph_x, glyph_y, baseline_y, metrics.height, metrics.ymin);
                }

                // Draw glyph to buffer with premultiplied alpha
                self.draw_glyph(
                    &mut buffer,
                    &bitmap,
                    &metrics,
                    glyph_x,
                    glyph_y,
                    fg_r,
                    fg_g,
                    fg_b,
                    width,
                    height,
                    is_bgra,
                );
            }
        }

        log::info!("Rendered {} non-empty characters from terminal grid", char_count);

        Ok(buffer)
    }

    /// Draw a single glyph to the buffer
    fn draw_glyph(
        &self,
        buffer: &mut [u8],
        bitmap: &[u8],
        metrics: &fontdue::Metrics,
        glyph_x: f32,
        glyph_y: f32,
        fg_r: u8,
        fg_g: u8,
        fg_b: u8,
        width: u32,
        height: u32,
        is_bgra: bool,
    ) {
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let px = glyph_x as i32 + gx as i32;
                let py = glyph_y as i32 + gy as i32;

                // Bounds check
                if px >= 0 && py >= 0 && px < width as i32 && py < height as i32 {
                    let glyph_idx = gy * metrics.width + gx;
                    let coverage = bitmap[glyph_idx];

                    if coverage > 0 {
                        let buffer_idx = ((py as usize * width as usize) + px as usize) * 4;

                        // Premultiply the color channels by alpha for correct blending
                        let alpha = coverage as f32 / 255.0;
                        let fg_r_pre = (fg_r as f32 * alpha) as u8;
                        let fg_g_pre = (fg_g as f32 * alpha) as u8;
                        let fg_b_pre = (fg_b as f32 * alpha) as u8;

                        // Write in correct channel order (BGRA or RGBA)
                        if is_bgra {
                            buffer[buffer_idx] = fg_b_pre;     // B (premultiplied)
                            buffer[buffer_idx + 1] = fg_g_pre; // G (premultiplied)
                            buffer[buffer_idx + 2] = fg_r_pre; // R (premultiplied)
                            buffer[buffer_idx + 3] = coverage; // A
                        } else {
                            buffer[buffer_idx] = fg_r_pre;     // R (premultiplied)
                            buffer[buffer_idx + 1] = fg_g_pre; // G (premultiplied)
                            buffer[buffer_idx + 2] = fg_b_pre; // B (premultiplied)
                            buffer[buffer_idx + 3] = coverage; // A
                        }
                    }
                }
            }
        }
    }


}
