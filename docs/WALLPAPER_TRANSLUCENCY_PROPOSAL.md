# Wallpaper and Translucency Implementation Proposal

## Overview

This document outlines the implementation plan for adding wallpaper backgrounds and enhanced translucency features to Saternal terminal. The goal is to allow users to customize the terminal background with images while maintaining excellent text readability through configurable opacity controls.

## Current State Analysis

### Existing Infrastructure
- **Config System**: `AppearanceConfig` already has `opacity: f32` field (default: 0.95) but it's **NOT USED** in rendering
- **Color Palette**: Background color has hardcoded alpha: `[0.09, 0.09, 0.13, 0.95]` (Tokyo Night theme)
- **Text Rendering**: Uses CPU rasterization → GPU texture upload → fullscreen quad rendering
- **Shader**: Simple `text.wgsl` shader that samples terminal texture and outputs directly
- **Render Pipeline**: Uses premultiplied alpha blending
- **Clear Color**: Hardcoded black `(0, 0, 0, 1.0)` in render pass

### Key Architecture Points
```
Text Rasterization (CPU)
    ↓
GPU Texture Upload (terminal content with premultiplied alpha)
    ↓
Render Pass:
    1. Clear to black
    2. Draw fullscreen quad (text texture)
    3. Draw selection highlights
    4. Draw cursor
    ↓
Present Frame
```

## Proposed Solution

### Design Goals
1. **Optional Wallpaper**: Users can set a background image
2. **Configurable Opacity**: Separate controls for wallpaper and overall background
3. **Text Readability**: Ensure text remains readable over any wallpaper
4. **Performance**: No significant impact on rendering performance
5. **Backward Compatibility**: Works without wallpaper (current behavior)

### Architecture Changes

#### 1. Configuration Changes (`saternal-core/src/config.rs`)

Add to `AppearanceConfig`:
```rust
/// Wallpaper image path (optional)
#[serde(default)]
pub wallpaper_path: Option<String>,

/// Wallpaper opacity (0.0-1.0) - lower values make text more readable
#[serde(default = "default_wallpaper_opacity")]
pub wallpaper_opacity: f32,
```

Default wallpaper opacity: **0.3** (30%) - subtle enough to not interfere with text

Keep existing `opacity: f32` field for overall background opacity.

#### 2. Wallpaper Texture Loading (`saternal-core/src/renderer/`)

Create new module: `saternal-core/src/renderer/wallpaper.rs`

```rust
pub struct WallpaperManager {
    texture: Option<wgpu::Texture>,
    view: Option<wgpu::TextureView>,
    sampler: wgpu::Sampler,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl WallpaperManager {
    /// Load wallpaper from file path
    pub fn load_wallpaper(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Result<Self> {
        // Use `image` crate to load PNG/JPG/etc
        // Resize to match window dimensions if needed
        // Upload to GPU texture
    }

    /// Clear wallpaper (return to solid background)
    pub fn clear_wallpaper(&mut self) {
        self.texture = None;
        self.view = None;
        self.bind_group = None;
    }
}
```

**Image Loading Strategy**:
- Use `image` crate (already in dependencies for other purposes)
- Support common formats: PNG, JPG, WEBP
- Resize/scale to match window dimensions for performance
- Convert to RGBA8 format for GPU upload

#### 3. Shader Updates (`saternal-core/src/shaders/text.wgsl`)

**Current Shader**:
```wgsl
@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_texture, t_sampler, input.tex_coords);
    return color;
}
```

**New Shader** (with wallpaper support):
```wgsl
@group(0) @binding(0) var t_texture: texture_2d<f32>;      // Terminal text texture
@group(0) @binding(1) var t_sampler: sampler;
@group(0) @binding(2) var wallpaper_texture: texture_2d<f32>;  // NEW
@group(0) @binding(3) var wallpaper_sampler: sampler;          // NEW

// Uniform buffer for opacity controls
struct Uniforms {
    wallpaper_opacity: f32,
    background_opacity: f32,
    has_wallpaper: u32,  // Boolean flag (0 or 1)
    padding: f32,        // Alignment
}

@group(1) @binding(0) var<uniform> uniforms: Uniforms;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample terminal content (text + background)
    let terminal_color = textureSample(t_texture, t_sampler, input.tex_coords);

    if (uniforms.has_wallpaper == 0u) {
        // No wallpaper - return terminal content as-is
        return terminal_color;
    }

    // Sample wallpaper texture
    let wallpaper_color = textureSample(wallpaper_texture, wallpaper_sampler, input.tex_coords);

    // Apply wallpaper opacity
    let wallpaper_dimmed = vec4<f32>(
        wallpaper_color.rgb * uniforms.wallpaper_opacity,
        uniforms.wallpaper_opacity
    );

    // Blend wallpaper with terminal content using premultiplied alpha
    // Terminal content is already premultiplied from text rasterization
    let bg_blend = wallpaper_dimmed * (1.0 - terminal_color.a) + terminal_color;

    // Apply overall background opacity
    return vec4<f32>(bg_blend.rgb, bg_blend.a * uniforms.background_opacity);
}
```

**Blending Strategy**:
1. **Wallpaper Layer** (bottom): Dimmed by `wallpaper_opacity` (default 30%)
2. **Terminal Background**: Defined by color palette (e.g., Tokyo Night dark blue)
3. **Text Layer** (top): Full opacity text with premultiplied alpha

This ensures:
- Text always renders on top and remains readable
- Wallpaper shows through transparent areas
- Background color provides a "tint" over the wallpaper for cohesion

#### 4. Renderer Changes (`saternal-core/src/renderer/mod.rs`)

**Add to `Renderer` struct**:
```rust
pub struct Renderer {
    // ... existing fields ...
    wallpaper_manager: Option<WallpaperManager>,
    background_uniforms: wgpu::Buffer,
    background_bind_group: wgpu::BindGroup,
}
```

**Update `Renderer::new()`**:
```rust
pub async fn new(
    window: std::sync::Arc<winit::window::Window>,
    font_family: &str,
    font_size: f32,
    cursor_config: CursorConfig,
    color_palette: ColorPalette,
    wallpaper_path: Option<&str>,           // NEW
    wallpaper_opacity: f32,                  // NEW
    background_opacity: f32,                 // NEW
) -> Result<Self> {
    // ... existing initialization ...

    // Load wallpaper if path provided
    let wallpaper_manager = if let Some(path) = wallpaper_path {
        Some(WallpaperManager::load_wallpaper(&device, &queue, path)?)
    } else {
        None
    };

    // Create uniform buffer for opacity controls
    let background_uniforms = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Background Uniforms"),
        contents: bytemuck::cast_slice(&[
            wallpaper_opacity,
            background_opacity,
            if wallpaper_manager.is_some() { 1u32 } else { 0u32 },
            0.0f32, // padding
        ]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // ... rest of initialization ...
}
```

**Add method to reload wallpaper**:
```rust
pub fn set_wallpaper(&mut self, path: Option<&str>) -> Result<()> {
    if let Some(path) = path {
        self.wallpaper_manager = Some(
            WallpaperManager::load_wallpaper(&self.device, &self.queue, path)?
        );
    } else {
        self.wallpaper_manager = None;
    }
    self.update_background_uniforms();
    Ok(())
}

pub fn set_wallpaper_opacity(&mut self, opacity: f32) {
    // Update uniform buffer
    self.update_background_uniforms();
}
```

#### 5. Render Pipeline Updates

**Update bind group layout** to include wallpaper texture:
```rust
let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    entries: &[
        // Binding 0: Terminal texture
        wgpu::BindGroupLayoutEntry { /* ... */ },
        // Binding 1: Terminal sampler
        wgpu::BindGroupLayoutEntry { /* ... */ },
        // Binding 2: Wallpaper texture (NEW)
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { /* ... */ },
            count: None,
        },
        // Binding 3: Wallpaper sampler (NEW)
        wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
    label: Some("texture_bind_group_layout"),
});
```

**Create additional bind group layout for uniforms** (group 1):
```rust
let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    entries: &[
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
    ],
    label: Some("uniform_bind_group_layout"),
});
```

#### 6. Text Rasterizer Updates (Optional Enhancement)

**Current**: Background is baked into the terminal texture at palette's alpha value

**Enhancement**: Render background separately for better control

Option A (Simple - Recommended):
- Keep current approach
- Use shader blending for wallpaper
- Apply `background_opacity` in shader

Option B (Complex):
- Separate background from text in rasterization
- More control but requires significant refactoring

**Recommendation**: Go with Option A for initial implementation.

## Implementation Plan

### Phase 1: Configuration and Infrastructure
1. ✅ Explore codebase architecture
2. ✅ Create design document
3. Add `wallpaper_path` and `wallpaper_opacity` to `AppearanceConfig`
4. Create `wallpaper.rs` module with `WallpaperManager`
5. Add `image` crate dependency if not already present

### Phase 2: Shader and Rendering
6. Update `text.wgsl` shader with wallpaper blending
7. Create uniform buffer for opacity controls
8. Update bind group layouts in `pipeline.rs`
9. Integrate `WallpaperManager` into `Renderer`

### Phase 3: Integration
10. Update `Renderer::new()` to accept wallpaper parameters
11. Update `saternal/src/app/init.rs` to pass config values to renderer
12. Add methods to change wallpaper at runtime (for future hot-reload)

### Phase 4: Testing and Polish
13. Test with various image formats (PNG, JPG)
14. Test with different wallpaper opacities
15. Test with and without wallpaper (backward compatibility)
16. Test performance impact
17. Update documentation

## File Checklist

### Files to Modify
- ✅ `saternal-core/src/config.rs` - Add wallpaper fields
- ✅ `saternal-core/src/renderer/mod.rs` - Add WallpaperManager integration
- ✅ `saternal-core/src/renderer/pipeline.rs` - Update bind group layouts
- ✅ `saternal-core/src/shaders/text.wgsl` - Add wallpaper blending
- ✅ `saternal/src/app/init.rs` - Pass wallpaper config to renderer

### Files to Create
- ✅ `saternal-core/src/renderer/wallpaper.rs` - Wallpaper loading and management

### Dependencies
Check if `image` crate is already in `Cargo.toml`, otherwise add:
```toml
image = "0.24"
```

## Example Configuration

After implementation, users can configure wallpapers in `~/.config/saternal/config.toml`:

```toml
[appearance]
opacity = 0.95                                    # Overall background opacity
wallpaper_path = "/Users/sam/Pictures/bg.png"    # Optional wallpaper
wallpaper_opacity = 0.3                           # Wallpaper dimming (30%)
blur = true                                       # macOS vibrancy effect

[appearance.palette]
background = [0.09, 0.09, 0.13, 0.95]  # Tokyo Night with transparency
# ... other colors ...
```

## Performance Considerations

### GPU Memory
- Wallpaper texture: ~8MB for 1920x1080 RGBA8 image
- Negligible impact on modern GPUs

### Rendering Performance
- Additional texture sample in fragment shader: ~0.1ms overhead
- Negligible impact at 60fps target (16.6ms budget)

### Loading Time
- Image decode + GPU upload: ~50-100ms one-time cost at startup
- Non-blocking if loaded asynchronously

## Edge Cases and Considerations

1. **Missing Wallpaper File**: Handle gracefully, fallback to no wallpaper
2. **Invalid Image Format**: Log error, disable wallpaper
3. **Very Large Images**: Resize to window dimensions to avoid excessive memory usage
4. **Window Resize**: Stretch/scale wallpaper (no re-decode needed)
5. **Multi-Monitor**: Wallpaper should work on any monitor (tested in dropdown behavior)

## Future Enhancements

1. **Wallpaper Scaling Modes**:
   - Fill (default)
   - Fit
   - Stretch
   - Tile
   - Center

2. **Live Wallpaper Reload**:
   - Watch file for changes
   - Hot-reload without restart

3. **Built-in Wallpapers**:
   - Ship with curated set of terminal-friendly backgrounds

4. **Blur/Effects**:
   - Apply Gaussian blur to wallpaper
   - Integrate with macOS vibrancy effects

5. **Per-Pane Wallpapers**:
   - Different wallpapers for split panes

## Testing Strategy

### Manual Testing
1. No wallpaper (current behavior) ✓
2. With wallpaper at various opacities (0.1, 0.3, 0.5, 0.8) ✓
3. Different image formats (PNG, JPG) ✓
4. Invalid/missing files ✓
5. Window resize with wallpaper ✓
6. Multiple panes with wallpaper ✓

### Visual Verification
- Text remains crisp and readable
- Wallpaper doesn't overpower terminal content
- Smooth blending between layers
- No visual artifacts or tearing

## Success Criteria

- ✅ Users can set a wallpaper via config file
- ✅ Wallpaper opacity is configurable (0.0-1.0)
- ✅ Text remains fully readable over any wallpaper
- ✅ No significant performance impact (<1ms rendering overhead)
- ✅ Works without wallpaper (backward compatible)
- ✅ Handles errors gracefully (missing/invalid files)

## Industry Research: How Other Terminals Handle Wallpapers

### WezTerm (Most Feature-Rich)
**Approach**: Multi-layer background system with extensive controls

```lua
config.background = {
  {
    source = { File = '/path/to/wallpaper.jpg' },
    hsb = { brightness = 0.3 },  -- Dim to 30% for readability
    repeat_x = 'NoRepeat',
    width = '100%',
    height = '100%',
  },
  {
    source = { Color = 'rgba(28, 33, 39, 0.95)' },  -- Semi-transparent overlay
    width = '100%',
    height = '100%',
  }
}
config.window_background_opacity = 0.9
```

**Key Features**:
- Multiple background layers (images, gradients, colors)
- HSB adjustments (hue, saturation, brightness)
- Separate wallpaper opacity and window opacity
- Parallax scrolling effects
- Tiling/scaling modes

**Lessons for Saternal**:
- ✅ Separate wallpaper opacity from overall background opacity
- ✅ Support HSB/brightness adjustments for better text contrast
- ✅ Layer composition: wallpaper → color overlay → text
- ⚠️ Start simple, advanced features (parallax, gradients) can come later

### Windows Terminal
**Approach**: Per-profile background images with alignment options

```json
{
  "backgroundImage": "C:\\path\\to\\image.png",
  "backgroundImageOpacity": 0.1,
  "backgroundImageAlignment": "center",
  "backgroundImageStretchMode": "uniformToFill"
}
```

**Key Features**:
- Very low default opacity (0.1 = 10%) for subtle backgrounds
- Multiple stretch modes: none, fill, uniform, uniformToFill
- Alignment options: center, left, top, right, bottom, topLeft, etc.
- Live opacity adjustment with keyboard shortcuts

**Lessons for Saternal**:
- ✅ Default wallpaper opacity should be VERY low (0.1-0.3)
- ✅ Stretch modes are important for different image aspect ratios
- 💡 Future: keyboard shortcuts for live opacity adjustment

### URxvt (rxvt-unicode)
**Approach**: Perl-based background expressions

```
URxvt.background.expr: scale keep { load "/path/to/mybg.png" }
```

**Key Features**:
- Dynamic background calculations
- Pseudo-transparency (root pixmap)
- Blur effects
- Image transformations

**Lessons for Saternal**:
- ✅ Support blur/effects on wallpaper
- ⚠️ Perl expressions are powerful but complex - TOML config is simpler

### Rio Terminal
**Approach**: Dedicated background image configuration block

```toml
[window.background-image]
path = "/path/to/image.png"
width = 1200
height = 800
opacity = 0.5
x = 0.0
y = 0.0
```

**Lessons for Saternal**:
- ✅ Position control (x, y offsets) for tiling
- ✅ Explicit size control
- ✅ Clean TOML structure

### Contour Terminal
**Approach**: Integrated into color schemes

```yaml
color_schemes:
  default:
    background_image:
      path: '/path/to/image.png'
      opacity: 0.5
      blur: false
```

**Lessons for Saternal**:
- ✅ Blur toggle is simple and effective
- ✅ Part of appearance/theme configuration

### Industry Best Practices Summary

1. **Opacity Defaults**:
   - Wallpaper: 0.1 - 0.3 (very subtle)
   - Background: 0.85 - 0.95 (semi-transparent)
   - **Recommendation**: Default wallpaper opacity to **0.3** (30%)

2. **Image Adjustments**:
   - Brightness/darkness control (critical for readability)
   - Blur (helps text stand out)
   - Contrast/saturation (advanced)
   - **Recommendation**: Start with brightness only, add blur later

3. **Scaling Modes**:
   - Fill (stretch to fit)
   - Fit (maintain aspect ratio, may have black bars)
   - Tile (repeat pattern)
   - Center (no scaling)
   - **Recommendation**: Start with Fill, add others later

4. **Layer Composition**:
   ```
   Wallpaper (dimmed)
   ↓
   Color overlay (terminal background color with alpha)
   ↓
   Text (full opacity)
   ```

5. **Configuration Location**:
   - Usually part of `appearance` or `window` config
   - Separate opacity controls for wallpaper vs window
   - **Recommendation**: Add to `AppearanceConfig` as proposed

## Updated Implementation Strategy

Based on industry research, here's the refined implementation:

### Phase 1: Basic Wallpaper Support
1. Add `wallpaper_path: Option<String>` to config
2. Add `wallpaper_opacity: f32` (default: **0.3**)
3. Load image and upload to GPU
4. Simple shader blending: wallpaper → background color → text
5. Default scaling: fill window

### Phase 2: Enhanced Controls (Future)
6. Add `wallpaper_brightness: f32` for dimming control
7. Add `wallpaper_blur: bool` for text readability
8. Add scaling modes: fill, fit, center, tile
9. Add position controls: x/y offsets

### Phase 3: Advanced Features (Future)
10. Live opacity adjustment with keyboard shortcuts
11. Multiple wallpaper layers
12. Dynamic wallpaper changes based on time/theme
13. Parallax scrolling (like WezTerm)

## References

- Current architecture: see exploration results above
- WezTerm background docs: https://wezterm.org/config/lua/config/background.html
- Windows Terminal background: https://docs.microsoft.com/en-us/windows/terminal/customize-settings/profile-appearance
- Rio terminal config: https://raphamorim.io/rio/docs/config/
- Premultiplied alpha blending: https://developer.nvidia.com/content/alpha-blending-pre-or-not-pre
- WGSL spec: https://www.w3.org/TR/WGSL/
- `image` crate: https://docs.rs/image/latest/image/
