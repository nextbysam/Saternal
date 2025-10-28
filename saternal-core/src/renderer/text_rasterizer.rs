use crate::constants::{PADDING_LEFT, PADDING_TOP};
use crate::font::FontManager;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Cell;
use anyhow::Result;
use wgpu;

use super::color::ansi_to_rgb_with_palette;
use super::theme::ColorPalette;

/// Rasterizes terminal text to a pixel buffer for GPU upload
pub(crate) struct TextRasterizer {
    cell_width: f32,
    cell_height: f32,
    baseline_offset: f32,
}

impl TextRasterizer {
    /// Create a new text rasterizer with cell dimensions
    ///
    /// Padding values are sourced from shared constants to ensure they match
    /// App::calculate_terminal_size() in saternal/src/app/state.rs
    pub fn new(cell_width: f32, cell_height: f32, baseline_offset: f32) -> Self {
        Self {
            cell_width,
            cell_height,
            baseline_offset,
        }
    }

    /// Update cell dimensions (called when font size changes)
    pub fn update_dimensions(&mut self, cell_width: f32, cell_height: f32, baseline_offset: f32) {
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.baseline_offset = baseline_offset;
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

        // Determine if we need BGRA or RGBA based on surface format
        let is_bgra = matches!(
            surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Create buffer filled with background color (opaque)
        // The wallpaper is rendered BEFORE this in a separate pass
        let bg = palette.background;
        let bg_r = (bg[0] * 255.0) as u8;
        let bg_g = (bg[1] * 255.0) as u8;
        let bg_b = (bg[2] * 255.0) as u8;
        let bg_a = (bg[3] * 255.0) as u8;

        let mut buffer = vec![0u8; (width * height * 4) as usize];

        // Fill buffer with background color
        for pixel in buffer.chunks_exact_mut(4) {
            if is_bgra {
                pixel[0] = bg_b;
                pixel[1] = bg_g;
                pixel[2] = bg_r;
                pixel[3] = bg_a;
            } else {
                pixel[0] = bg_r;
                pixel[1] = bg_g;
                pixel[2] = bg_b;
                pixel[3] = bg_a;
            }
        }

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
                let cell_x = PADDING_LEFT + col_idx as f32 * self.cell_width;
                let cell_y = PADDING_TOP + row_idx as f32 * self.cell_height;

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

    /// Overlay UI box cells onto an existing buffer
    /// Renders cells at specified grid position
    pub fn overlay_cells(
        &self,
        buffer: &mut [u8],
        cells: &[Vec<Cell>],
        start_row: usize,
        start_col: usize,
        width: u32,
        height: u32,
        font_manager: &FontManager,
        surface_format: wgpu::TextureFormat,
        palette: &ColorPalette,
    ) {
        let is_bgra = matches!(
            surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Semi-transparent background for UI box
        let box_bg = [0.0, 0.0, 0.0, 0.8]; // Black with 80% opacity

        for (row_offset, row_cells) in cells.iter().enumerate() {
            let row = start_row + row_offset;
            
            for (col_offset, cell) in row_cells.iter().enumerate() {
                let col = start_col + col_offset;
                
                let x = (col as f32 * self.cell_width + PADDING_LEFT) as u32;
                let y = (row as f32 * self.cell_height + PADDING_TOP) as u32;
                
                // Draw cell background (semi-transparent box background)
                for dy in 0..(self.cell_height as u32) {
                    for dx in 0..(self.cell_width as u32) {
                        let px = x + dx;
                        let py = y + dy;
                        
                        if px < width && py < height {
                            let buffer_idx = ((py * width + px) * 4) as usize;
                            if buffer_idx + 3 < buffer.len() {
                                // Alpha blend with existing pixel
                                let alpha = box_bg[3];
                                let bg_r = (box_bg[0] * 255.0) as u8;
                                let bg_g = (box_bg[1] * 255.0) as u8;
                                let bg_b = (box_bg[2] * 255.0) as u8;
                                
                                let existing_r = buffer[buffer_idx] as f32 / 255.0;
                                let existing_g = buffer[buffer_idx + 1] as f32 / 255.0;
                                let existing_b = buffer[buffer_idx + 2] as f32 / 255.0;
                                
                                let blended_r = (bg_r as f32 / 255.0 * alpha + existing_r * (1.0 - alpha)) * 255.0;
                                let blended_g = (bg_g as f32 / 255.0 * alpha + existing_g * (1.0 - alpha)) * 255.0;
                                let blended_b = (bg_b as f32 / 255.0 * alpha + existing_b * (1.0 - alpha)) * 255.0;
                                
                                if is_bgra {
                                    buffer[buffer_idx] = blended_b as u8;
                                    buffer[buffer_idx + 1] = blended_g as u8;
                                    buffer[buffer_idx + 2] = blended_r as u8;
                                } else {
                                    buffer[buffer_idx] = blended_r as u8;
                                    buffer[buffer_idx + 1] = blended_g as u8;
                                    buffer[buffer_idx + 2] = blended_b as u8;
                                }
                            }
                        }
                    }
                }
                
                // Draw character if not space
                let c = cell.c;
                if c != ' ' && c != '\0' {
                    let glyph_bitmap = font_manager.rasterize_glyph(c);
                    let fg_rgb = ansi_to_rgb_with_palette(&cell.fg, palette);
                    let fg_r = (fg_rgb[0] * 255.0) as u8;
                    let fg_g = (fg_rgb[1] * 255.0) as u8;
                    let fg_b = (fg_rgb[2] * 255.0) as u8;
                    
                    // Draw glyph
                    for (glyph_y, glyph_row) in glyph_bitmap.iter().enumerate() {
                        for (glyph_x, &coverage) in glyph_row.iter().enumerate() {
                            if coverage > 0 {
                                let px = x + glyph_x as u32;
                                let py = y + self.baseline_offset as u32 + glyph_y as u32;
                                
                                if px < width && py < height {
                                    let buffer_idx = ((py * width + px) * 4) as usize;
                                    if buffer_idx + 3 < buffer.len() {
                                        let alpha = coverage as f32 / 255.0;
                                        let fg_r_pre = (fg_r as f32 * alpha) as u8;
                                        let fg_g_pre = (fg_g as f32 * alpha) as u8;
                                        let fg_b_pre = (fg_b as f32 * alpha) as u8;
                                        
                                        if is_bgra {
                                            buffer[buffer_idx] = fg_b_pre;
                                            buffer[buffer_idx + 1] = fg_g_pre;
                                            buffer[buffer_idx + 2] = fg_r_pre;
                                            buffer[buffer_idx + 3] = coverage;
                                        } else {
                                            buffer[buffer_idx] = fg_r_pre;
                                            buffer[buffer_idx + 1] = fg_g_pre;
                                            buffer[buffer_idx + 2] = fg_b_pre;
                                            buffer[buffer_idx + 3] = coverage;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
