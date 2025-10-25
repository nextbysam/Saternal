# Cursor Position Fix for Multiple Panes

## Problem

The cursor only respects the first pane position, even when focus changes to the second pane. The cursor blinking works correctly, but the **position** is wrong for non-first panes.

## Root Cause

In `saternal-core/src/renderer/mod.rs:336-375`, the `update_cursor_position_with_viewport()` function:

1. **Correctly calculates** the cursor position relative to the viewport (lines 352-357):
   ```rust
   let cursor_pixel_x = viewport.x as f32 + cursor_pos.column.0 as f32 * cell_width + 10.0;
   let cursor_pixel_y = viewport.y as f32 + cursor_pos.line.0 as f32 * cell_height + 5.0;
   let ndc_x = (cursor_pixel_x / self.config.width as f32) * 2.0 - 1.0;
   let ndc_y = -((cursor_pixel_y / self.config.height as f32) * 2.0 - 1.0);
   ```

2. **Then ignores those calculations** and calls `cursor_state.update_position()` with the original cursor position without viewport offsets (lines 364-372):
   ```rust
   self.cursor_state.update_position(
       cursor_pos,  // ❌ Original position, not adjusted for viewport!
       cell_width,
       cell_height,
       self.config.width,   // ❌ Using full window dimensions
       self.config.height,  // ❌ Not viewport dimensions
       0,
       hide_cursor,
   );
   ```

The problem is that `update_position()` **recalculates** the NDC coordinates from scratch using the cursor's grid position and the full window dimensions, which assumes the terminal starts at (0, 0). This works for the first pane but not for split panes with viewport offsets.

## How Other Terminal Emulators Handle Multiple Panes

### WezTerm
- Maintains cursor state **per pane**
- Only shows blinking cursor in the focused pane
- Unfocused panes show an outline/hollow cursor that doesn't blink
- Commit `d2e73e1` standardized behavior across cursor styles (block, beam, underline)

### Key Pattern
Most terminal emulators use one of two approaches:
1. **Hide cursor entirely** in unfocused panes (simplest)
2. **Different visual style** for unfocused panes (hollow/outlined, non-blinking)

## Solution Options

### Option 1: Fix Position Calculation (Immediate Fix)

Modify `update_cursor_position_with_viewport()` to use the already-calculated NDC coordinates instead of recalculating them.

**Changes needed in `saternal-core/src/renderer/mod.rs:336-375`:**

```rust
fn update_cursor_position_with_viewport<T>(&mut self, term: &Term<T>, viewport: &PaneViewport) {
    let cursor_pos = term.grid().cursor.point;

    let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR)
                      || self.scroll_offset > 0.01;

    let effective_size = self.font_manager.effective_font_size();
    let line_metrics = self.font_manager.font()
        .horizontal_line_metrics(effective_size)
        .unwrap();
    let cell_width = self.font_manager.font()
        .metrics('M', effective_size)
        .advance_width;
    let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

    // Calculate cursor position relative to viewport
    let cursor_pixel_x = viewport.x as f32 + cursor_pos.column.0 as f32 * cell_width + 10.0;
    let cursor_pixel_y = viewport.y as f32 + cursor_pos.line.0 as f32 * cell_height + 5.0;

    // Convert to NDC
    let ndc_x = (cursor_pixel_x / self.config.width as f32) * 2.0 - 1.0;
    let ndc_y = -((cursor_pixel_y / self.config.height as f32) * 2.0 - 1.0);

    // Calculate size based on cursor style
    let (width, height) = match self.cursor_state.config.style {
        CursorStyle::Block => (cell_width, cell_height),
        CursorStyle::Beam => (2.0, cell_height),
        CursorStyle::Underline => (cell_width, 2.0),
    };

    let ndc_width = (width / self.config.width as f32) * 2.0;
    let ndc_height = -((height / self.config.height as f32) * 2.0);

    // Adjust Y for underline style
    let ndc_y = if matches!(self.cursor_state.config.style, CursorStyle::Underline) {
        ndc_y + (cell_height - 2.0) / self.config.height as f32 * 2.0
    } else {
        ndc_y
    };

    // Determine visibility
    let visible = if hide_cursor {
        0
    } else if self.cursor_state.config.blink {
        self.cursor_state.blink_state.visible as u32
    } else {
        1
    };

    // Directly set cursor uniforms using viewport-adjusted NDC coordinates
    self.cursor_state.current_uniforms = CursorUniforms {
        position: [ndc_x, ndc_y],
        size: [ndc_width, ndc_height],
        color: self.cursor_state.config.color,
        visible,
        style: self.cursor_state.config.style as u32,
        _padding: [0, 0],
    };

    self.cursor_state.upload_uniforms(&self.queue);
}
```

**Problem with Option 1:** This requires accessing private fields of `CursorState` (`config`, `blink_state`, `current_uniforms`).

### Option 2: Add New Method to CursorState

Add a method to `CursorState` that accepts pre-calculated NDC coordinates:

**In `saternal-core/src/renderer/cursor/state.rs`:**

```rust
impl CursorState {
    /// Update cursor with pre-calculated NDC coordinates (for viewports)
    pub fn update_position_ndc(
        &mut self,
        ndc_x: f32,
        ndc_y: f32,
        ndc_width: f32,
        ndc_height: f32,
        hide_cursor: bool,
    ) {
        // Determine visibility
        let visible = if hide_cursor {
            0
        } else if self.config.blink {
            self.blink_state.visible as u32
        } else {
            1
        };

        self.current_uniforms = CursorUniforms {
            position: [ndc_x, ndc_y],
            size: [ndc_width, ndc_height],
            color: self.config.color,
            visible,
            style: self.config.style as u32,
            _padding: [0, 0],
        };
    }
}
```

**Then in `saternal-core/src/renderer/mod.rs:336-375`:**

```rust
fn update_cursor_position_with_viewport<T>(&mut self, term: &Term<T>, viewport: &PaneViewport) {
    let cursor_pos = term.grid().cursor.point;

    let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR)
                      || self.scroll_offset > 0.01;

    let effective_size = self.font_manager.effective_font_size();
    let line_metrics = self.font_manager.font()
        .horizontal_line_metrics(effective_size)
        .unwrap();
    let cell_width = self.font_manager.font()
        .metrics('M', effective_size)
        .advance_width;
    let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

    // Calculate cursor position relative to viewport
    let cursor_pixel_x = viewport.x as f32 + cursor_pos.column.0 as f32 * cell_width + 10.0;
    let cursor_pixel_y = viewport.y as f32 + cursor_pos.line.0 as f32 * cell_height + 5.0;

    // Convert to NDC
    let ndc_x = (cursor_pixel_x / self.config.width as f32) * 2.0 - 1.0;
    let ndc_y = -((cursor_pixel_y / self.config.height as f32) * 2.0 - 1.0);

    // Calculate size based on cursor style
    let cursor_config = &self.cursor_state.config;
    let (width, height) = match cursor_config.style {
        CursorStyle::Block => (cell_width, cell_height),
        CursorStyle::Beam => (2.0, cell_height),
        CursorStyle::Underline => (cell_width, 2.0),
    };

    let ndc_width = (width / self.config.width as f32) * 2.0;
    let ndc_height = -((height / self.config.height as f32) * 2.0);

    // Adjust Y for underline style
    let ndc_y = if matches!(cursor_config.style, CursorStyle::Underline) {
        ndc_y + (cell_height - 2.0) / self.config.height as f32 * 2.0
    } else {
        ndc_y
    };

    log::debug!("Cursor at viewport offset: pixel=({}, {}), ndc=({}, {})",
               cursor_pixel_x, cursor_pixel_y, ndc_x, ndc_y);

    // Use NDC coordinates directly
    self.cursor_state.update_position_ndc(
        ndc_x,
        ndc_y,
        ndc_width,
        ndc_height,
        hide_cursor,
    );

    self.cursor_state.upload_uniforms(&self.queue);
}
```

### Option 3: Hide Cursor in Unfocused Panes (Simplest)

Modify the rendering to only show cursor in the focused pane:

**In `saternal-core/src/renderer/mod.rs` around line 230:**

```rust
// Update cursor position if this is the focused pane
if viewport.focused {
    self.update_cursor_position_with_viewport(&term_lock, viewport);
} else {
    // Hide cursor for unfocused panes
    self.cursor_state.current_uniforms.visible = 0;
    self.cursor_state.upload_uniforms(&self.queue);
}
```

## Recommended Approach

**Option 2** is the cleanest solution:
1. Add `update_position_ndc()` method to `CursorState`
2. Modify `update_cursor_position_with_viewport()` to use it
3. This preserves the encapsulation of `CursorState` while fixing the position bug

**Option 3** could be added as an additional enhancement to ensure only focused panes show cursors.

## Testing

After implementing the fix:
1. Create two panes with `Cmd+D`
2. Switch focus with `Cmd+Shift+[` or `Cmd+Shift+]`
3. Verify cursor appears at the correct position in the focused pane
4. Type in each pane to confirm cursor tracks properly

## References

- WezTerm Discussion #1445: "Can I hide Bar cursor in inactive panes?"
- WezTerm Commit d2e73e1: Standardized cursor outline behavior for unfocused panes
- Your existing implementation: `saternal-core/src/renderer/cursor/state.rs:120-185`
