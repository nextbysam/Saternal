# Selection Accuracy Fix Proposal

## Problem Statement

The current selection implementation in Saternal has accuracy issues where the selection highlighting doesn't accurately correspond to where the user points their cursor in the window. This makes text selection frustrating and unpredictable.

## Current Implementation Analysis

### Current Architecture
- **Selection Logic**: `saternal-core/src/selection/mod.rs` - Manages selection state with start/end points
- **Rendering**: `saternal-core/src/selection/renderer.rs` - GPU-accelerated rendering using WGPU
- **Shader**: `saternal-core/src/shaders/selection.wgsl` - Fragment/vertex shaders for highlighting

### Current Coordinate Conversion (renderer.rs:336-365)

```rust
fn create_span(
    &self,
    line: usize,
    col: usize,
    width_cells: usize,
    cell_width: f32,
    cell_height: f32,
    window_width: u32,
    window_height: u32,
) -> SelectionSpan {
    // Padding constants (must match TextRasterizer padding)
    const PADDING_LEFT: f32 = 10.0;
    const PADDING_TOP: f32 = 5.0;

    let pixel_x = PADDING_LEFT + col as f32 * cell_width;
    let pixel_y = PADDING_TOP + line as f32 * cell_height;
    let pixel_width = width_cells as f32 * cell_width;

    // Convert to NDC
    let ndc_x = (pixel_x / window_width as f32) * 2.0 - 1.0;
    let ndc_y = -((pixel_y / window_height as f32) * 2.0 - 1.0);
    let ndc_width = (pixel_width / window_width as f32) * 2.0;
    let ndc_height = -((cell_height / window_height as f32) * 2.0);

    SelectionSpan {
        position: [ndc_x, ndc_y],
        size: [ndc_width, ndc_height],
    }
}
```

### Identified Issues

1. **Missing Reverse Conversion**: There's no corresponding function to convert mouse pixel coordinates back to grid coordinates
2. **Hard-coded Padding**: The padding values (10.0, 5.0) are hard-coded and duplicated
3. **No Rounding Logic**: When converting pixels to grid cells, there's no intelligent rounding to determine which cell the cursor is actually in
4. **Pane Viewport Handling**: The current code doesn't account for split panes properly when converting coordinates

## Research: How Other Terminal Emulators Handle This

### Alacritty's Approach

**Key Findings:**
- Uses a `SizeInfo` struct that can "convert window space pixels to terminal grid coordinates"
- Coordinates outside the grid (in padding areas) are "clamped to the closest grid coordinates"
- Selection struct has start/end anchors with both point (line/column) and side (left/right)
- Supports multiple selection types: basic, block, semantic (words), and line selection

**Critical Implementation Detail:**
The coordinate conversion accounts for:
- Cell dimensions (width/height)
- Window padding on all sides
- Proper clamping to prevent out-of-bounds selection

### WezTerm's Approach

**Key Findings:**
- Divides mouse coordinates by cell size to determine selected cell
- Initially had accuracy issues (Issues #898, #350) where "mouse selection grabs previous char too easily"
- **Solution**: Changed from simple division (which biases left/top) to **rounding to the nearest cell edge**
- This prevents the selection from being biased toward one side

**Quote from Issue #350:**
> "The math biases to the left/top. I think it's reasonable to round that to the nearest cell edge to decide selection."

### Rio Terminal's Approach

**Key Findings:**
- Provides mouse event handling with grid coordinates
- Uses coordinate transformation for accurate cell selection
- Handles mouse events with proper coordinate mapping

### Common Patterns Across All Implementations

1. **Bidirectional Conversion**: All terminals have both:
   - Grid coordinates → Pixel coordinates (for rendering)
   - Pixel coordinates → Grid coordinates (for selection)

2. **Proper Rounding**: Modern implementations use **rounding to nearest cell edge** rather than simple truncation/flooring

3. **Padding Awareness**: All calculations must account for terminal padding consistently

4. **Clamping**: Coordinates outside valid grid bounds are clamped to the nearest valid cell

5. **Cell-Edge Bias**: When a click is near a cell boundary, round to the nearest edge to determine which cell was intended

## Proposed Solution

### 1. Create a Centralized Coordinate Conversion Module

Create `saternal-core/src/geometry.rs`:

```rust
/// Terminal geometry and coordinate conversion utilities
pub struct TerminalGeometry {
    pub cell_width: f32,
    pub cell_height: f32,
    pub window_width: u32,
    pub window_height: u32,
    pub padding_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub padding_bottom: f32,
    pub grid_cols: usize,
    pub grid_lines: usize,
}

impl TerminalGeometry {
    /// Convert pixel coordinates to grid coordinates (for mouse input)
    /// Uses rounding to nearest cell edge for better accuracy
    pub fn pixels_to_grid(&self, pixel_x: f32, pixel_y: f32) -> Option<(usize, usize)> {
        // Subtract padding to get position within grid area
        let grid_x = pixel_x - self.padding_left;
        let grid_y = pixel_y - self.padding_top;

        // Check if click is within grid bounds
        if grid_x < 0.0 || grid_y < 0.0 {
            return None;
        }

        // Calculate grid position with rounding to nearest cell edge
        // This prevents the "grabbing previous char" issue that WezTerm fixed
        let col = ((grid_x + self.cell_width * 0.5) / self.cell_width).floor() as usize;
        let line = ((grid_y + self.cell_height * 0.5) / self.cell_height).floor() as usize;

        // Clamp to grid bounds (following Alacritty's approach)
        if col >= self.grid_cols || line >= self.grid_lines {
            return Some((
                col.min(self.grid_cols.saturating_sub(1)),
                line.min(self.grid_lines.saturating_sub(1)),
            ));
        }

        Some((col, line))
    }

    /// Convert grid coordinates to pixel coordinates (for rendering)
    pub fn grid_to_pixels(&self, col: usize, line: usize) -> (f32, f32) {
        let pixel_x = self.padding_left + col as f32 * self.cell_width;
        let pixel_y = self.padding_top + line as f32 * self.cell_height;
        (pixel_x, pixel_y)
    }

    /// Convert pixel coordinates to NDC (Normalized Device Coordinates)
    pub fn pixels_to_ndc(&self, pixel_x: f32, pixel_y: f32) -> (f32, f32) {
        let ndc_x = (pixel_x / self.window_width as f32) * 2.0 - 1.0;
        let ndc_y = -((pixel_y / self.window_height as f32) * 2.0 - 1.0);
        (ndc_x, ndc_y)
    }

    /// Convert grid coordinates to NDC for rendering
    pub fn grid_to_ndc(&self, col: usize, line: usize) -> (f32, f32) {
        let (pixel_x, pixel_y) = self.grid_to_pixels(col, line);
        self.pixels_to_ndc(pixel_x, pixel_y)
    }

    /// Check if pixel coordinates are within the grid area
    pub fn is_within_grid(&self, pixel_x: f32, pixel_y: f32) -> bool {
        let grid_width = self.grid_cols as f32 * self.cell_width;
        let grid_height = self.grid_lines as f32 * self.cell_height;

        pixel_x >= self.padding_left
            && pixel_x < self.padding_left + grid_width
            && pixel_y >= self.padding_top
            && pixel_y < self.padding_top + grid_height
    }
}
```

### 2. Update SelectionRenderer to Use TerminalGeometry

Modify `saternal-core/src/selection/renderer.rs`:

```rust
use crate::geometry::TerminalGeometry;

impl SelectionRenderer {
    /// Update selection spans from grid range using centralized geometry
    pub fn update_with_geometry(
        &mut self,
        range: Option<SelectionRange>,
        geometry: &TerminalGeometry,
    ) {
        if let Some(range) = range {
            let spans = self.range_to_spans(range, geometry);
            self.current_uniforms.count = spans.len() as u32;
            for (i, span) in spans.iter().enumerate() {
                if i < 64 {
                    self.current_uniforms.spans[i] = *span;
                }
            }
            self.dirty = true;
        } else {
            self.current_uniforms.count = 0;
            self.dirty = true;
        }
    }

    /// Convert selection range to NDC spans using geometry
    fn range_to_spans(
        &self,
        range: SelectionRange,
        geometry: &TerminalGeometry,
    ) -> Vec<SelectionSpan> {
        let (start, end) = range.normalized();
        let mut spans = Vec::new();

        // Clamp to grid bounds
        let max_col = geometry.grid_cols.saturating_sub(1);
        let max_line = (geometry.grid_lines as i32).saturating_sub(1);
        let start_col = start.column.0.min(max_col);
        let end_col = end.column.0.min(max_col);
        let start_line = start.line.0.max(0).min(max_line);
        let end_line = end.line.0.max(0).min(max_line);

        if start_line == end_line {
            // Single line selection
            let width = end_col.saturating_sub(start_col) + 1;
            let span = self.create_span_with_geometry(
                start_line as usize,
                start_col,
                width,
                geometry,
            );
            spans.push(span);
        } else {
            // Multi-line selection (same logic as before but using geometry)
            // ... (similar to existing code but using geometry methods)
        }

        spans
    }

    /// Create a single span in NDC coordinates using geometry
    fn create_span_with_geometry(
        &self,
        line: usize,
        col: usize,
        width_cells: usize,
        geometry: &TerminalGeometry,
    ) -> SelectionSpan {
        let (pixel_x, pixel_y) = geometry.grid_to_pixels(col, line);
        let pixel_width = width_cells as f32 * geometry.cell_width;

        let (ndc_x, ndc_y) = geometry.pixels_to_ndc(pixel_x, pixel_y);
        let (ndc_end_x, ndc_end_y) = geometry.pixels_to_ndc(
            pixel_x + pixel_width,
            pixel_y + geometry.cell_height
        );

        let ndc_width = ndc_end_x - ndc_x;
        let ndc_height = ndc_end_y - ndc_y;

        SelectionSpan {
            position: [ndc_x, ndc_y],
            size: [ndc_width, ndc_height],
        }
    }
}
```

### 3. Handle Split Pane Coordinates

Update the pane viewport calculation to provide per-pane geometry:

```rust
impl PaneViewport {
    /// Get geometry for this pane
    pub fn to_geometry(
        &self,
        cell_width: f32,
        cell_height: f32,
        padding_left: f32,
        padding_top: f32,
        grid_cols: usize,
        grid_lines: usize,
    ) -> TerminalGeometry {
        TerminalGeometry {
            cell_width,
            cell_height,
            window_width: self.width,
            window_height: self.height,
            padding_left: self.x as f32 + padding_left,
            padding_top: self.y as f32 + padding_top,
            padding_right: padding_left,
            padding_bottom: padding_top,
            grid_cols,
            grid_lines,
        }
    }

    /// Convert window-relative mouse coordinates to pane-relative
    pub fn window_to_pane_coords(&self, window_x: f32, window_y: f32) -> Option<(f32, f32)> {
        let pane_x = window_x - self.x as f32;
        let pane_y = window_y - self.y as f32;

        // Check if coordinates are within this pane
        if pane_x >= 0.0 && pane_x < self.width as f32
            && pane_y >= 0.0 && pane_y < self.height as f32 {
            Some((pane_x, pane_y))
        } else {
            None
        }
    }
}
```

### 4. Update Mouse Event Handling

The mouse event handler should use the new coordinate conversion:

```rust
// In the mouse event handler
fn handle_mouse_event(&mut self, window_x: f32, window_y: f32) {
    // Find which pane the mouse is in
    for viewport in &self.pane_viewports {
        if let Some((pane_x, pane_y)) = viewport.window_to_pane_coords(window_x, window_y) {
            // Get geometry for this pane
            let geometry = viewport.to_geometry(
                self.cell_width,
                self.cell_height,
                PADDING_LEFT,
                PADDING_TOP,
                self.grid_cols,
                self.grid_lines,
            );

            // Convert to grid coordinates
            if let Some((col, line)) = geometry.pixels_to_grid(pane_x, pane_y) {
                // Update selection with accurate grid coordinates
                let point = Point::new(
                    alacritty_terminal::index::Line(line as i32),
                    alacritty_terminal::index::Column(col)
                );

                if is_mouse_pressed {
                    self.selection_manager.start(point, SelectionMode::Simple);
                } else {
                    self.selection_manager.update(point);
                }
            }
        }
    }
}
```

## Implementation Plan

### Phase 1: Core Geometry Module
1. Create `geometry.rs` with the `TerminalGeometry` struct
2. Implement bidirectional coordinate conversion methods
3. Add comprehensive unit tests for coordinate conversion accuracy

### Phase 2: Refactor Selection Renderer
1. Update `SelectionRenderer` to use `TerminalGeometry`
2. Remove hard-coded padding constants
3. Replace direct coordinate calculations with geometry methods
4. Test rendering accuracy

### Phase 3: Mouse Input Integration
1. Update mouse event handlers to use `pixels_to_grid()`
2. Implement pane-aware coordinate conversion
3. Add proper bounds checking and clamping
4. Test with split panes

### Phase 4: Testing & Validation
1. Test single-pane selection accuracy
2. Test split-pane selection accuracy
3. Test edge cases (clicks in padding, near boundaries)
4. Test different font sizes and window sizes
5. Test with different DPI settings

## Benefits

1. **Accuracy**: Rounding to nearest cell edge (WezTerm's solution) fixes the "grabbing wrong character" issue
2. **Maintainability**: Centralized coordinate conversion means one source of truth
3. **Split Pane Support**: Proper viewport handling for accurate selection across panes
4. **Consistency**: Same geometry calculations used for both rendering and input
5. **Testability**: Geometry logic can be unit tested independently
6. **Flexibility**: Easy to adjust padding, cell sizes, or add new coordinate systems

## Potential Risks & Mitigations

### Risk: Breaking Existing Rendering
**Mitigation**: Implement alongside existing code, test thoroughly, then switch over

### Risk: Performance Impact
**Mitigation**: Geometry calculations are simple floating-point math, minimal overhead

### Risk: Different Results from Current Implementation
**Mitigation**: This is expected and desired - the current implementation has bugs

## References

- WezTerm Issue #350: "Mouse selection grabs previous char too easy"
- WezTerm Issue #898: "Mouse position events are inaccurate"
- Alacritty's SizeInfo documentation
- Current Saternal implementation in `saternal-core/src/selection/`

## Conclusion

By implementing a centralized coordinate conversion system with proper rounding logic (as proven effective by WezTerm), we can fix the selection accuracy issues in Saternal. The key insight from researching other terminal emulators is that **rounding to the nearest cell edge** rather than simple truncation is critical for accurate mouse-to-grid conversion.
