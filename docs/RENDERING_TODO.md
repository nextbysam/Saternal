# Terminal Rendering Implementation TODO

## Current State (Updated 2025-10-23)

The application now has:
- ✅ Working hotkey toggle (Cmd+`)
- ✅ Dropdown window animation (slides from top, fades in/out)
- ✅ Terminal backend (PTY, shell, VTE processor) - **CONFIRMED WORKING**
- ✅ Keyboard input capture and forwarding to terminal
- ✅ Terminal output processing - **165 BYTES FROM SHELL CONFIRMED**
- ✅ Renderer infrastructure with wgpu/Metal
- ✅ Font manager with glyph caching
- ✅ Terminal grid access in renderer - **29 CHARS IN GRID CONFIRMED**
- ✅ Text rendering to texture buffer - **PIXELS WRITTEN CONFIRMED**
- ✅ Texture upload to GPU - **3024x982 TEXTURE UPLOADED**
- ❌ **DISPLAY TO SCREEN - NOT WORKING**

## Critical Issue: Texture Not Displaying

**Confirmed via debugging:**
1. Shell outputs data: `Processed 165 bytes total from shell` ✅
2. Grid populated: `Rendered 29 non-empty characters` (sam@S...) ✅
3. Pixels written: `Writing pixel at (0,0) = RGBA(229,229,229,240)` ✅
4. Texture uploaded: `Uploading 3024x982 texture to GPU` ✅
5. **Screen shows: NOTHING (black window)** ❌

**Test performed:**
- Filled entire texture with solid green (0,255,0,255)
- Result: Still black screen
- Conclusion: Shader/rendering pipeline broken, not text generation

## What Needs Fixing:

### 1. Create a Text Rendering Pipeline

**File**: `saternal-core/src/renderer.rs`

The renderer needs:
- **Vertex and fragment shaders** for rendering glyph quads
- **Texture atlas** to store rasterized glyphs
- **Vertex buffer** to store quad positions and texture coordinates
- **Render pipeline** configured for text rendering

### 2. Implement Grid-to-Glyph Rendering

In the `render()` method, iterate through the terminal grid:

```rust
if let Some(term_lock) = term_arc.try_lock() {
    let rows = term_lock.screen_lines();
    let cols = term_lock.columns();
    
    // Access terminal grid (using alacritty's API)
    // Note: Need to find the correct way to access the grid in alacritty 0.25
    // Likely through term.grid() or similar method
    
    for row in 0..rows {
        for col in 0..cols {
            // Get cell at (col, row)
            // Extract character, foreground color, background color, flags
            // Rasterize glyph using font_manager
            // Upload to texture atlas
            // Add quad to vertex buffer
        }
    }
    
    // Draw all quads in one draw call
}
```

### 3. Handle Terminal Colors

Terminal cells have:
- Foreground color (text color)
- Background color (cell background)
- Flags (bold, italic, underline, etc.)

You need to:
- Parse ANSI colors from terminal cells
- Pass colors to fragment shader
- Support 256-color and true-color modes

### 4. Optimize Rendering

- **Damage tracking**: Only redraw changed cells
- **Texture atlas management**: Cache frequently used glyphs
- **Batch rendering**: Draw all text in one or few draw calls
- **Double buffering**: Use previous frame state to detect changes

## Alternative: Use a Pre-built Text Renderer

Instead of implementing from scratch, consider:

1. **wgpu_glyph**: A text rendering library for wgpu
   - Add to Cargo.toml: `wgpu_glyph = "0.19"`
   - Simplifies glyph rendering significantly

2. **glyphon**: Another wgpu text renderer
   - More modern, better performance
   - Built specifically for terminal-like applications

## Quick Path Forward

### Option A: Minimal Implementation (Show Something ASAP)

For immediate visual feedback, implement a simple CPU-based text renderer:
1. Use `fontdue` to rasterize glyphs
2. Create an RGBA texture for the entire terminal
3. Draw each glyph directly to texture pixels
4. Upload texture to GPU each frame
5. Render as a single quad

This is slower but gets text on screen quickly for testing.

### Option B: Proper GPU Implementation

Follow the full pipeline above. This takes more time but provides:
- Better performance
- Smooth scrolling
- Proper damage tracking
- Support for all terminal features

## Files to Modify

1. **saternal-core/src/renderer.rs** - Main rendering logic
2. **saternal-core/src/font.rs** - May need texture atlas support
3. Create **saternal-core/src/shaders.wgsl** - WGSL shaders for text

## Testing the Current State

Even without text rendering, you can verify:
1. Window toggles (Cmd+Shift+T)
2. Window animates smoothly
3. Application doesn't crash
4. Terminal process spawns (check Activity Monitor for shell)
5. Keyboard input is captured (add debug logging)

## Recommendation

Start with **Option A** to get visual feedback, then refactor to **Option B** for production quality. This lets you verify the entire pipeline works before optimizing rendering.
