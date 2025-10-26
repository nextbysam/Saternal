# GPU-Accelerated Blur Implementation

**Status:** ⚠️ DISABLED - Interfering with terminal text rendering
**Date:** 2025-10-26 (Updated)
**Priority:** Critical Bug - Blur Disabled

---

## Overview

Implementation of GPU-accelerated Gaussian blur for Saternal's wallpaper rendering **AND** macOS backdrop blur for content behind the window. The system consists of two complementary blur mechanisms:

1. **GPU Wallpaper Blur**: Two-pass Gaussian blur on the wallpaper texture (runs on GPU via wgpu/Metal)
2. **Backdrop Blur**: macOS NSVisualEffectView blur of content behind the window (desktop, other apps)

---

## Implementation Summary

### ✅ Completed Features

#### 1. GPU Wallpaper Blur

**Configuration System**
- Added `blur_strength` field to `AppearanceConfig` (default: 2.0)
- Range: 0.0 (disabled) to 10.0+ (maximum blur)
- Location: `saternal-core/src/config.rs`

**Runtime Commands**
- `blur-strength <value>` command parser
- `wallpaper-opacity <value>` - affects backdrop blur intensity
- `background-opacity <value>` - affects backdrop blur intensity
- Location: `saternal/src/app/commands.rs`

**Blur Infrastructure**
- `BlurRenderer` integrated into main `Renderer`
- Blur intermediate textures created and managed
- Proper texture format matching (Bgra8UnormSrgb)
- Getter methods: `blur_strength()`, `wallpaper_opacity()`, `background_opacity()`
- Location: `saternal-core/src/renderer/mod.rs`, `saternal-core/src/renderer/blur.rs`

**Wallpaper Texture Management**
- Blurred wallpaper texture support
- Bind group management for blurred textures
- Dynamic format support
- Location: `saternal-core/src/renderer/wallpaper.rs`

**Shader Implementation**
- Two-pass Gaussian blur (horizontal → vertical)
- 9-tap kernel for quality/performance balance
- Manual loop unrolling to avoid WGSL dynamic array indexing
- Location: `saternal-core/src/shaders/blur.wgsl`

**Render Pipeline Integration**
- Conditional blur application based on `blur_enabled` flag
- Separate render passes for blur (horizontal, vertical)
- Proper bind group selection (blurred vs regular wallpaper)
- Location: `saternal-core/src/renderer/mod.rs`

#### 2. Backdrop Blur (macOS NSVisualEffectView)

**Dependencies Added**
- `window-vibrancy = "0.6"` - macOS backdrop blur integration
- `raw-window-handle = "0.6"` - Required by window-vibrancy
- Location: `Cargo.toml`

**Window Management**
- `DropdownWindow::set_backdrop_blur(ns_window, radius)` - Dynamically update blur radius
- `apply_backdrop_blur_internal()` - Wrapper for NSVisualEffectView integration
- Uses `window_vibrancy::apply_vibrancy()` with HudWindow material
- Location: `saternal-macos/src/window.rs`

**Dynamic Blur Updates**
- `update_backdrop_blur()` helper function calculates radius based on:
  - `blur-strength`: Higher values = more backdrop blur
  - `wallpaper-opacity`: Lower opacity = more backdrop blur
  - `background-opacity`: Lower opacity = more backdrop blur
  - Formula: `radius = blur_strength * (1 - min(wallpaper_opacity, background_opacity)) * 20`
  - Range: 0-200 points (macOS blur radius)
- Location: `saternal/src/app/input.rs`

**Command Integration**
- `WallpaperOpacity`, `BackgroundOpacity`, `BlurStrength` commands trigger backdrop blur updates
- Initial backdrop blur set during app initialization
- Location: `saternal/src/app/input.rs`, `saternal/src/app/init.rs`

---

## Architecture

### Rendering Pipeline

```
Complete Flow:
1. macOS Desktop/Other Windows (BEHIND window)
   ↓
2. NSVisualEffectView Backdrop Blur (macOS compositor)
   ↓
3. Terminal background (opaque, with palette.background color)
   ↓
4. Wallpaper texture → GPU Blur (horizontal) → Temp texture
   ↓
5. Temp texture → GPU Blur (vertical) → Blurred wallpaper texture
   ↓
6. Fragment shader: Blurred wallpaper → Terminal texture → Screen
```

### Dual Blur System

#### GPU Wallpaper Blur
- **What it blurs**: The wallpaper image loaded via `wallpaper <path>` command
- **Where it runs**: GPU (Metal) via wgpu
- **Control**: `blur-strength` command (0.0-10.0+)
- **Purpose**: Artistic effect on the wallpaper behind terminal content

#### Backdrop Blur
- **What it blurs**: Content BEHIND the window (desktop wallpaper, other apps)
- **Where it runs**: macOS compositor (NSVisualEffectView)
- **Control**: Automatically calculated from `blur-strength + transparency`
- **Purpose**: Frosted glass effect, improves terminal readability

---

## How Backdrop Blur Works

### NSVisualEffectView Integration

The backdrop blur uses macOS's built-in `NSVisualEffectView` which provides a frosted glass effect. The implementation:

1. **Window Wrapper**: Creates a `WindowWrapper` struct that implements `HasWindowHandle` for compatibility with the `window-vibrancy` crate
2. **Vibrancy Application**: Calls `apply_vibrancy()` with:
   - Material: `NSVisualEffectMaterial::HudWindow` (dark, subtle effect)
   - State: `NSVisualEffectState::Active` (always on)
   - Radius: Dynamically calculated based on blur-strength and transparency

### Blur Radius Calculation

```rust
// Backdrop blur should be stronger when:
// 1. blur-strength is higher
// 2. window is more transparent (lower opacity)

let transparency_factor = 1.0 - wallpaper_opacity.min(background_opacity);
let radius = (blur_strength * transparency_factor * 20.0) as f64;
```

**Examples:**
- `blur-strength 5.0`, `wallpaper-opacity 0.3`: radius ≈ 70 points
- `blur-strength 10.0`, `wallpaper-opacity 0.1`: radius ≈ 180 points
- `blur-strength 0.0`: radius = 0 (no backdrop blur)
- `wallpaper-opacity 1.0` or `background-opacity 1.0`: radius = 0 (fully opaque)

---

## Testing

### Test Commands

```bash
# In Saternal terminal:

# GPU Wallpaper Blur
wallpaper ~/path/to/image.jpg
blur-strength 5.0   # Medium blur on wallpaper
blur-strength 10.0  # Maximum blur on wallpaper
blur-strength 0.0   # Disable blur

# Backdrop Blur (automatic)
blur-strength 5.0
wallpaper-opacity 0.3  # More transparency = more backdrop blur
background-opacity 0.8 # Combined with wallpaper-opacity

# Frosted glass effect
blur-strength 8.0
wallpaper-opacity 0.2  # Very transparent
# Result: Desktop behind window is heavily blurred
```

### Expected Behavior

**GPU Wallpaper Blur:**
- ✅ Wallpaper shows visible Gaussian blur
- ✅ Higher strength = more blur on wallpaper
- ✅ Strength 0.0 = crisp wallpaper

**Backdrop Blur:**
- ✅ Content behind window (desktop, other apps) is blurred
- ✅ More blur with higher blur-strength
- ✅ More blur with lower opacity (more transparent window)
- ✅ Frosted glass effect improves readability
- ✅ Smooth performance (60fps+)

---

## Implementation Details

### Files Modified/Added

#### 1. Dependencies
- `Cargo.toml` - Added `window-vibrancy`, `raw-window-handle`
- `saternal-macos/Cargo.toml` - Added workspace dependencies

#### 2. macOS Window Management
- `saternal-macos/src/window.rs`
  - `set_backdrop_blur()` - Public API to update blur radius
  - `apply_backdrop_blur_internal()` - NSVisualEffectView integration
  - `enable_vibrancy()` - Initialize with default radius (0.0)
  - `WindowWrapper` struct implementing `HasWindowHandle`

#### 3. Command Handling
- `saternal/src/app/input.rs`
  - `update_backdrop_blur()` - Calculate and apply backdrop blur
  - Updated `execute_command()` to call `update_backdrop_blur()` for:
    - `WallpaperOpacity` command
    - `BackgroundOpacity` command
    - `BlurStrength` command
  - Updated `handle_keyboard_input()` signature to accept `dropdown`
  - Updated `handle_terminal_input()` signature to accept `dropdown`

#### 4. Event Loop
- `saternal/src/app/event_loop.rs`
  - Pass `dropdown` to `handle_keyboard_input()`

#### 5. Initialization
- `saternal/src/app/init.rs`
  - Calculate and set initial backdrop blur on app startup
  - Uses config values for blur-strength, wallpaper-opacity, background-opacity

#### 6. Renderer Getters
- `saternal-core/src/renderer/mod.rs`
  - `blur_strength()` - Get current blur strength
  - `wallpaper_opacity()` - Get current wallpaper opacity
  - `background_opacity()` - Get current background opacity
- `saternal-core/src/renderer/blur.rs`
  - `strength()` - Get current blur strength from BlurRenderer

---

## GPU Resources

**Wallpaper Blur (GPU):**
- Blurred wallpaper texture (window size, surface format)
- Blur intermediate texture (for two-pass blur)
- Blur horizontal pass bind group
- Blur vertical pass bind group
- Blurred wallpaper bind group (for final rendering)

**Backdrop Blur (macOS):**
- NSVisualEffectView (managed by macOS compositor)
- Zero additional GPU memory from our app
- Blur radius stored as f64 in DropdownWindow

**Render Passes:**
1. Backdrop blur: macOS compositor (behind window)
2. GPU horizontal blur: wallpaper → temp texture
3. GPU vertical blur: temp texture → blurred wallpaper
4. Main render: blurred wallpaper → terminal → screen

---

## Performance

### GPU Wallpaper Blur
- Two-pass Gaussian blur runs on Metal
- 9-tap kernel (18 texture samples per pixel)
- ~8MB memory for 1920x1080 textures
- 60fps+ on modern GPUs

### Backdrop Blur
- Handled by macOS compositor (zero GPU overhead from our app)
- No performance impact on rendering pipeline
- Native macOS effect (same as used by Terminal.app, iTerm2)

---

## Usage Examples

### Subtle Frosted Glass
```bash
wallpaper ~/Pictures/mountains.jpg
blur-strength 3.0
wallpaper-opacity 0.4
background-opacity 0.9
```
Result: Slight blur on desktop behind window, medium blur on wallpaper

### Heavy Frosted Glass
```bash
blur-strength 10.0
wallpaper-opacity 0.15
background-opacity 0.85
```
Result: Desktop heavily blurred behind window, strong blur on wallpaper

### No Backdrop Blur
```bash
blur-strength 0.0
# or
wallpaper-opacity 1.0
# or
background-opacity 1.0
```
Result: No backdrop blur, but wallpaper can still be blurred if blur-strength > 0

---

## Technical Notes

### WGSL Shader Constraints

**Issue:** Cannot use dynamic array indexing in WGSL
```wgsl
// ❌ This doesn't work:
for (var i = 0; i < 9; i++) {
    result += sample * KERNEL_WEIGHTS[i];  // Error: dynamic index
}
```

**Solution:** Manual loop unrolling
```wgsl
// ✅ This works:
result += textureSample(...) * 0.05;
result += textureSample(...) * 0.09;
// ... (9 samples total)
```

### Texture Format Matching

Critical: Blur pipeline format **must match** surface format
- Surface: `Bgra8UnormSrgb` (typical on macOS)
- Blur textures: Must use same format
- Mismatch causes validation error

### NSVisualEffectView Material

We use `NSVisualEffectMaterial::HudWindow` for the backdrop blur because:
- Dark, subtle effect suitable for terminals
- Better contrast for text readability
- Matches the aesthetic of other developer tools

Other materials available:
- `AppearanceBased` - Adapts to system light/dark mode
- `Sidebar` - Lighter effect
- `Menu` - Menu-like appearance
- See `window-vibrancy` docs for full list

---

## References

- [window-vibrancy crate](https://docs.rs/window-vibrancy/latest/window_vibrancy/)
- [WezTerm backdrop blur implementation](https://wezfurlong.org/wezterm/config/lua/config/macos_window_background_blur.html)
- [NSVisualEffectView documentation](https://developer.apple.com/documentation/appkit/nsvisualeffectview)
- WGSL Specification: Array indexing constraints
- wgpu Validation: Texture format compatibility
- Gaussian Blur: Two-pass separable filter optimization

---

## ⚠️ CRITICAL ISSUE - GPU Blur Disabled (2025-10-26)

### Problem
The GPU wallpaper blur shader was interfering with terminal text rendering, making the terminal completely invisible even when commands were working. The blur was rendering on top of the terminal content instead of behind it.

### Root Cause Analysis
1. **Shader Compositing**: The text shader (saternal-core/src/shaders/text.wgsl) was incorrectly applying `background_opacity` to the final output, making all content (including text) transparent
2. **Render Pass Order**: While the blur rendered to an intermediate texture correctly, the compositing formula made text invisible
3. **Premultiplied Alpha Issues**: Confusion between premultiplied alpha blending and opacity multipliers

### Attempted Fixes
1. ✅ Fixed shader to remove `background_opacity` multiplication on line 75 of text.wgsl
2. ✅ Changed blur.rs vertical pass from `LoadOp::Load` to `LoadOp::Clear`
3. ❌ Text still not visible - blur shader still interfering

### Temporary Solution (CURRENT STATE)
**GPU Wallpaper Blur DISABLED** in saternal-core/src/renderer/mod.rs:
- Lines 488-502: Blur render passes commented out
- Lines 529-535: Always use regular wallpaper bind group (not blurred)
- Backdrop blur (NSVisualEffectView) remains functional

### Files Modified
- `saternal-core/src/renderer/mod.rs`: Disabled blur shader rendering
- `saternal-core/src/shaders/text.wgsl`: Fixed opacity multiplication
- `saternal-core/src/renderer/blur.rs`: Changed LoadOp to Clear

### Next Steps to Re-enable Blur
1. **Debug render pass ordering**: Verify blur writes to intermediate texture, not screen
2. **Test without wallpaper**: Ensure terminal text renders correctly with no wallpaper
3. **Test with wallpaper**: Ensure wallpaper shows behind terminal text
4. **Re-enable blur gradually**: First test blur rendering, then test compositing
5. **Fix shader blending**: Ensure terminal text always has alpha=1.0 for visibility

### Current Behavior
- ✅ Terminal text is visible
- ✅ Terminal commands work
- ✅ Backdrop blur (macOS NSVisualEffectView) works
- ❌ GPU wallpaper blur disabled
- ⚠️ Wallpaper may or may not be visible (needs testing)

---

## Original Conclusion (Now Outdated)

Saternal ~~now has~~ **attempted** a complete dual-blur system. However, due to critical rendering bugs, the GPU wallpaper blur has been temporarily disabled. Only the backdrop blur (NSVisualEffectView) is currently functional.
