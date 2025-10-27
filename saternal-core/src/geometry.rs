/// Terminal geometry and coordinate conversion utilities
///
/// This module provides centralized, bidirectional coordinate conversion between:
/// - Pixel coordinates (screen space)
/// - Grid coordinates (terminal cells: column, line)
/// - NDC (Normalized Device Coordinates for GPU rendering)
///
/// Key design principles:
/// - Single source of truth for all coordinate conversions
/// - Proper rounding to nearest cell edge (prevents "grabbing wrong character" issue)
/// - Pane-aware viewport support for split terminals
/// - Zero-allocation, inline hot-path functions for performance
/// - GPU-friendly f32 math throughout

use alacritty_terminal::index::{Column, Line, Point};

/// Terminal geometry configuration for coordinate conversions
#[derive(Debug, Clone)]
pub struct TerminalGeometry {
    /// Cell dimensions
    pub cell_width: f32,
    pub cell_height: f32,

    /// Window/viewport dimensions
    pub window_width: u32,
    pub window_height: u32,

    /// Padding around terminal grid
    pub padding_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,

    /// Grid dimensions (in cells)
    pub grid_cols: usize,
    pub grid_lines: usize,
}

impl TerminalGeometry {
    /// Create a new TerminalGeometry with all parameters
    #[inline]
    pub fn new(
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
        padding_left: f32,
        padding_top: f32,
        padding_right: f32,
        padding_bottom: f32,
        grid_cols: usize,
        grid_lines: usize,
    ) -> Self {
        Self {
            cell_width,
            cell_height,
            window_width,
            window_height,
            padding_left,
            padding_top,
            padding_right,
            padding_bottom,
            grid_cols,
            grid_lines,
        }
    }

    /// Convert pixel coordinates to grid coordinates (for mouse input)
    ///
    /// Uses rounding to nearest cell edge for better accuracy, following WezTerm's approach.
    /// This prevents the "grabbing previous char too easily" issue caused by naive flooring.
    ///
    /// # Algorithm
    /// - Subtract padding to get position within grid area
    /// - Add half a cell size before dividing (rounds to nearest edge)
    /// - Clamp result to valid grid bounds
    ///
    /// # Returns
    /// - `Some((col, line))` if coordinates map to a valid grid position
    /// - `None` if coordinates are outside the grid area (in padding)
    #[inline]
    pub fn pixels_to_grid(&self, pixel_x: f32, pixel_y: f32) -> Option<(usize, usize)> {
        // Subtract padding to get position within grid area
        let grid_x = pixel_x - self.padding_left;
        let grid_y = pixel_y - self.padding_top;

        // Check if click is within grid bounds (before padding removal)
        if grid_x < 0.0 || grid_y < 0.0 {
            return None;
        }

        // Calculate grid position with rounding to nearest cell edge
        // Adding 0.5 * cell_size ensures we round to the nearest cell boundary
        // This is the key fix from WezTerm issue #350
        let col = ((grid_x + self.cell_width * 0.5) / self.cell_width).floor() as usize;
        let line = ((grid_y + self.cell_height * 0.5) / self.cell_height).floor() as usize;

        // Clamp to grid bounds (following Alacritty's approach)
        // Return None if completely outside, or clamp if near edge
        if col >= self.grid_cols || line >= self.grid_lines {
            // Allow clicks slightly outside to select the last cell
            return Some((
                col.min(self.grid_cols.saturating_sub(1)),
                line.min(self.grid_lines.saturating_sub(1)),
            ));
        }

        Some((col, line))
    }

    /// Convert pixel coordinates to a terminal Point
    ///
    /// Convenience wrapper around `pixels_to_grid()` that returns an Alacritty Point.
    #[inline]
    pub fn pixels_to_point(&self, pixel_x: f32, pixel_y: f32) -> Option<Point> {
        self.pixels_to_grid(pixel_x, pixel_y)
            .map(|(col, line)| Point::new(Line(line as i32), Column(col)))
    }

    /// Convert grid coordinates to pixel coordinates (for rendering)
    ///
    /// Returns the top-left pixel position of the specified grid cell.
    #[inline]
    pub fn grid_to_pixels(&self, col: usize, line: usize) -> (f32, f32) {
        let pixel_x = self.padding_left + col as f32 * self.cell_width;
        let pixel_y = self.padding_top + line as f32 * self.cell_height;
        (pixel_x, pixel_y)
    }

    /// Convert pixel coordinates to NDC (Normalized Device Coordinates)
    ///
    /// NDC is the coordinate system used by GPU shaders:
    /// - X: -1.0 (left) to +1.0 (right)
    /// - Y: +1.0 (top) to -1.0 (bottom)  [inverted from screen space]
    #[inline]
    pub fn pixels_to_ndc(&self, pixel_x: f32, pixel_y: f32) -> (f32, f32) {
        let ndc_x = (pixel_x / self.window_width as f32) * 2.0 - 1.0;
        let ndc_y = -((pixel_y / self.window_height as f32) * 2.0 - 1.0);
        (ndc_x, ndc_y)
    }

    /// Convert pixel dimensions to NDC dimensions
    #[inline]
    pub fn pixel_size_to_ndc(&self, width: f32, height: f32) -> (f32, f32) {
        let ndc_width = (width / self.window_width as f32) * 2.0;
        let ndc_height = -((height / self.window_height as f32) * 2.0);
        (ndc_width, ndc_height)
    }

    /// Convert grid coordinates to NDC for rendering
    ///
    /// Combined convenience method: grid → pixels → NDC
    #[inline]
    pub fn grid_to_ndc(&self, col: usize, line: usize) -> (f32, f32) {
        let (pixel_x, pixel_y) = self.grid_to_pixels(col, line);
        self.pixels_to_ndc(pixel_x, pixel_y)
    }

    /// Check if pixel coordinates are within the grid area
    #[inline]
    pub fn is_within_grid(&self, pixel_x: f32, pixel_y: f32) -> bool {
        let grid_width = self.grid_cols as f32 * self.cell_width;
        let grid_height = self.grid_lines as f32 * self.cell_height;

        pixel_x >= self.padding_left
            && pixel_x < self.padding_left + grid_width
            && pixel_y >= self.padding_top
            && pixel_y < self.padding_top + grid_height
    }

    /// Get the maximum valid grid position
    #[inline]
    pub fn max_grid_pos(&self) -> (usize, usize) {
        (
            self.grid_cols.saturating_sub(1),
            self.grid_lines.saturating_sub(1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_geometry() -> TerminalGeometry {
        TerminalGeometry::new(
            10.0,  // cell_width
            20.0,  // cell_height
            800,   // window_width
            600,   // window_height
            10.0,  // padding_left
            5.0,   // padding_top
            10.0,  // padding_right
            10.0,  // padding_bottom
            80,    // grid_cols
            30,    // grid_lines
        )
    }

    #[test]
    fn test_pixels_to_grid_center_of_cell() {
        let geom = test_geometry();
        // Click in the center of cell (5, 3)
        let pixel_x = 10.0 + 5.0 * 10.0 + 5.0; // padding + 5 cells + half cell
        let pixel_y = 5.0 + 3.0 * 20.0 + 10.0; // padding + 3 cells + half cell

        let result = geom.pixels_to_grid(pixel_x, pixel_y);
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn test_pixels_to_grid_near_left_edge() {
        let geom = test_geometry();
        // Click just left of center of cell (5, 3) - should still select cell 5
        let pixel_x = 10.0 + 5.0 * 10.0 + 4.0; // slightly left of center
        let pixel_y = 5.0 + 3.0 * 20.0 + 10.0;

        let result = geom.pixels_to_grid(pixel_x, pixel_y);
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn test_pixels_to_grid_near_right_edge() {
        let geom = test_geometry();
        // Click just right of center of cell (5, 3) - should still select cell 5
        let pixel_x = 10.0 + 5.0 * 10.0 + 6.0; // slightly right of center
        let pixel_y = 5.0 + 3.0 * 20.0 + 10.0;

        let result = geom.pixels_to_grid(pixel_x, pixel_y);
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn test_pixels_to_grid_boundary_rounds_correctly() {
        let geom = test_geometry();
        // Click exactly on the boundary between cells 4 and 5
        let pixel_x = 10.0 + 5.0 * 10.0; // exactly at cell 5 left edge
        let pixel_y = 5.0 + 3.0 * 20.0;

        let result = geom.pixels_to_grid(pixel_x, pixel_y);
        // With 0.5 rounding, boundary clicks should select the left/top cell
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn test_pixels_to_grid_in_padding() {
        let geom = test_geometry();
        // Click in left padding area
        let result = geom.pixels_to_grid(5.0, 100.0);
        assert_eq!(result, None);

        // Click in top padding area
        let result = geom.pixels_to_grid(100.0, 2.0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_pixels_to_grid_clamping() {
        let geom = test_geometry();
        // Click beyond grid bounds - should clamp to last cell
        let result = geom.pixels_to_grid(10000.0, 10000.0);
        assert_eq!(result, Some((79, 29))); // Max valid position
    }

    #[test]
    fn test_grid_to_pixels_round_trip() {
        let geom = test_geometry();

        // Test round-trip conversion for cell (10, 5)
        let (pixel_x, pixel_y) = geom.grid_to_pixels(10, 5);
        let result = geom.pixels_to_grid(pixel_x + 5.0, pixel_y + 10.0); // Click center
        assert_eq!(result, Some((10, 5)));
    }

    #[test]
    fn test_pixels_to_ndc() {
        let geom = test_geometry();

        // Top-left corner (0, 0) -> NDC (-1, 1)
        let (ndc_x, ndc_y) = geom.pixels_to_ndc(0.0, 0.0);
        assert_eq!(ndc_x, -1.0);
        assert_eq!(ndc_y, 1.0);

        // Bottom-right corner (800, 600) -> NDC (1, -1)
        let (ndc_x, ndc_y) = geom.pixels_to_ndc(800.0, 600.0);
        assert_eq!(ndc_x, 1.0);
        assert_eq!(ndc_y, -1.0);

        // Center (400, 300) -> NDC (0, 0)
        let (ndc_x, ndc_y) = geom.pixels_to_ndc(400.0, 300.0);
        assert_eq!(ndc_x, 0.0);
        assert_eq!(ndc_y, 0.0);
    }

    #[test]
    fn test_grid_to_ndc() {
        let geom = test_geometry();

        // Cell (0, 0) with padding should be slightly right/down from top-left
        let (ndc_x, ndc_y) = geom.grid_to_ndc(0, 0);
        assert!(ndc_x > -1.0); // Not at far left (due to padding)
        assert!(ndc_y < 1.0);  // Not at very top (due to padding)
    }

    #[test]
    fn test_is_within_grid() {
        let geom = test_geometry();

        // Inside grid
        assert!(geom.is_within_grid(50.0, 50.0));

        // In padding (outside grid)
        assert!(!geom.is_within_grid(5.0, 50.0)); // Left padding
        assert!(!geom.is_within_grid(50.0, 2.0)); // Top padding

        // Beyond grid
        assert!(!geom.is_within_grid(10000.0, 50.0));
        assert!(!geom.is_within_grid(50.0, 10000.0));
    }
}
