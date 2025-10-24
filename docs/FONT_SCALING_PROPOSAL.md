# Font Scaling Implementation Proposal

**Date:** 2025-10-24
**Status:** Draft
**Author:** Claude Code

---

## Executive Summary

This document proposes an implementation strategy for dynamic font scaling in Saternal, based on research into how leading Rust terminal emulators (Alacritty, WezTerm, Rio) handle font sizing and DPI awareness. The goal is to ensure fonts scale appropriately across different displays and window sizes.

---

## Current State Analysis

### Saternal's Current Implementation

**Architecture:**
- **GPU-accelerated rendering** using wgpu (Metal backend on macOS)
- **Font rasterization** via fontdue (CPU-side)
- **Hybrid approach:** CPU rasterizes glyphs → GPU textures → display

**Font Management (font.rs):**
```rust
pub struct FontManager {
    font: Font,
    font_size: f32,  // Points
    glyph_cache: HashMap<(char, u32), (usize, usize, Vec<u8>)>,
}
```

**Current Sizing Logic (renderer/mod.rs:103-109):**
```rust
let line_metrics = font_manager.font()
    .horizontal_line_metrics(font_size).unwrap();
let cell_width = font_manager.font()
    .metrics('M', font_size).advance_width;
let cell_height = (line_metrics.ascent - line_metrics.descent
    + line_metrics.line_gap).ceil();
```

**Window Resizing (renderer/mod.rs:352-366):**
- Surface reconfiguration on window resize
- Texture buffer resizing
- **No automatic font scaling** on DPI change

### Key Issues

1. **No DPI awareness** - Font size is static regardless of display DPI
2. **No per-monitor DPI handling** - Moving window between displays doesn't adjust font
3. **Fixed font size** - No dynamic scaling based on window size or screen resolution
4. **Retina scaling workaround** - Current code uses `(font_size * 2.0) as u32` for cache keys (hardcoded)

---

## Research Findings: Terminal Emulator Best Practices

### 1. Alacritty's Approach

**Key Strategy:** Device Pixel Ratio (DPR) based scaling

**Core Concepts:**
- Detects DPR on window creation and resize events
- Scales font size by DPR: `effective_font_size = configured_size * DPR`
- Handles multi-monitor setups by recalculating on window move

**Common Issues (from GitHub):**
- Font size changes when dragging between monitors with different DPI
- Infinite resize loops when window spans multiple monitors
- Inconsistent behavior across X11/Wayland/macOS

**Lessons Learned:**
- Need to track DPR per window event
- Must handle monitor transitions gracefully
- Cache invalidation crucial when DPR changes

### 2. WezTerm's Approach

**Key Strategy:** Explicit DPI configuration + per-monitor awareness

**Configuration System:**
```lua
-- Global DPI override
config.dpi = 144.0

-- Per-monitor DPI (feature request, not yet implemented)
screens = {
  ['LG HDR WQHD'] = {
    dpi = 109,
    font_size = 11,
  }
}
```

**Platform-specific DPI Defaults:**
| Platform | Standard DPI | High DPI |
|----------|-------------|----------|
| macOS    | 72.0        | 144.0    |
| Windows  | Probed      | Probed   |
| X11      | 96.0 (Xft.dpi) | 96.0  |
| Wayland  | 96.0        | 192.0    |

**Dynamic Features:**
- `window-resized` event for recalculating layout
- `adjust_window_size_when_changing_font_size` config
- `use_cap_height_to_scale_fallback_fonts` for font consistency

**Lessons Learned:**
- Explicit DPI override valuable for edge cases
- Need platform-specific DPI detection
- Window events should trigger recalculation
- Consider both DPI and font size as independent variables

### 3. Rio Terminal's Approach

**Key Strategy:** GPU-accelerated with modern rendering pipeline

**Features:**
- Built with wgpu (same as Saternal!)
- 24-bit true color
- Cross-platform (Windows, macOS, Linux, FreeBSD)
- Retina display support

**Implementation Details (limited documentation):**
- Leverages wgpu's surface configuration for DPI
- Focus on performance through GPU acceleration
- Minimal configuration approach

**Lessons Learned:**
- wgpu provides native DPI scaling capabilities
- Surface reconfiguration handles DPI changes
- Keep configuration minimal for better UX

---

## Proposed Implementation Strategy

### Phase 1: DPI Detection & Management (Foundation)

**1.1 Add DPI State Tracking**

Create a new module: `saternal-core/src/dpi.rs`

```rust
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy)]
pub struct DpiContext {
    /// Device pixel ratio (e.g., 2.0 for Retina)
    pub scale_factor: f64,

    /// Effective DPI (typically 72 * scale_factor on macOS)
    pub dpi: f32,

    /// Logical size in points
    pub logical_size: (u32, u32),

    /// Physical size in pixels
    pub physical_size: PhysicalSize<u32>,
}

impl DpiContext {
    pub fn new(scale_factor: f64, physical_size: PhysicalSize<u32>) -> Self {
        let dpi = Self::calculate_effective_dpi(scale_factor);
        let logical_size = (
            (physical_size.width as f64 / scale_factor) as u32,
            (physical_size.height as f64 / scale_factor) as u32,
        );

        Self {
            scale_factor,
            dpi,
            logical_size,
            physical_size,
        }
    }

    fn calculate_effective_dpi(scale_factor: f64) -> f32 {
        #[cfg(target_os = "macos")]
        return (72.0 * scale_factor) as f32;

        #[cfg(target_os = "windows")]
        return (96.0 * scale_factor) as f32;

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        return (96.0 * scale_factor) as f32;
    }

    /// Scale font size by DPR for physical pixel calculations
    pub fn scale_font_size(&self, logical_font_size: f32) -> f32 {
        logical_font_size * self.scale_factor as f32
    }
}
```

**1.2 Update FontManager to be DPI-aware**

Modify `saternal-core/src/font.rs`:

```rust
pub struct FontManager {
    font: Font,
    configured_font_size: f32,  // Logical size from config
    effective_font_size: f32,   // Physical size for rendering
    dpi_context: DpiContext,
    glyph_cache: HashMap<(char, u32), (usize, usize, Vec<u8>)>,
}

impl FontManager {
    pub fn new(
        font_family: &str,
        font_size: f32,
        dpi_context: DpiContext,
    ) -> Result<Self> {
        let effective_font_size = dpi_context.scale_font_size(font_size);

        Ok(Self {
            font: Self::load_font(font_family)?,
            configured_font_size: font_size,
            effective_font_size,
            dpi_context,
            glyph_cache: HashMap::new(),
        })
    }

    /// Update DPI context and recalculate font metrics
    pub fn update_dpi(&mut self, new_dpi_context: DpiContext) {
        // Only clear cache if DPI actually changed
        if (self.dpi_context.scale_factor - new_dpi_context.scale_factor).abs() > 0.001 {
            self.dpi_context = new_dpi_context;
            self.effective_font_size = new_dpi_context.scale_font_size(
                self.configured_font_size
            );
            self.glyph_cache.clear();
        }
    }

    /// Get glyph using effective (scaled) font size
    pub fn get_glyph(&mut self, ch: char) -> Result<&(usize, usize, Vec<u8>)> {
        let size_key = self.effective_font_size.round() as u32;
        let cache_key = (ch, size_key);

        if !self.glyph_cache.contains_key(&cache_key) {
            let (metrics, bitmap) = self.rasterize(ch);
            // ... cache the glyph
        }

        Ok(self.glyph_cache.get(&cache_key).unwrap())
    }

    /// Cell dimensions in physical pixels
    pub fn cell_dimensions(&mut self) -> (usize, usize) {
        let line_metrics = self.font
            .horizontal_line_metrics(self.effective_font_size)
            .unwrap();
        let cell_width = self.font
            .metrics('M', self.effective_font_size)
            .advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent
            + line_metrics.line_gap).ceil();

        (cell_width as usize, cell_height as usize)
    }
}
```

**1.3 Update Configuration System**

Modify `saternal-core/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    pub font_family: String,
    pub font_size: f32,  // Logical size in points

    // Optional DPI override (None = auto-detect)
    #[serde(default)]
    pub dpi_override: Option<f32>,

    // Existing fields...
    pub opacity: f32,
    pub blur: bool,
    pub palette: ColorPalette,
}
```

### Phase 2: Window Event Handling

**2.1 Integrate with winit Events**

The main application event loop needs to handle:

```rust
use winit::event::WindowEvent;

match event {
    WindowEvent::ScaleFactorChanged {
        scale_factor,
        inner_size_writer
    } => {
        // Update DPI context
        let new_dpi_context = DpiContext::new(
            scale_factor,
            window.inner_size(),
        );

        // Update font manager
        font_manager.update_dpi(new_dpi_context);

        // Recalculate cell dimensions
        let (cell_width, cell_height) = font_manager.cell_dimensions();

        // Update renderer surface
        renderer.handle_dpi_change(new_dpi_context);
    }

    WindowEvent::Resized(new_size) => {
        // Check if DPI might have changed (monitor switch)
        let current_scale = window.scale_factor();
        let new_dpi_context = DpiContext::new(current_scale, new_size);

        renderer.resize(new_size.width, new_size.height);
        font_manager.update_dpi(new_dpi_context);
    }

    // ... other events
}
```

**2.2 Update Renderer to Track DPI**

Modify `saternal-core/src/renderer/mod.rs`:

```rust
pub struct Renderer {
    // ... existing fields
    dpi_context: DpiContext,
}

impl Renderer {
    pub fn handle_dpi_change(&mut self, new_dpi_context: DpiContext) {
        self.dpi_context = new_dpi_context;

        // Reconfigure surface with new scale
        self.surface.configure(&self.device, &self.surface_config);

        // Resize texture buffer
        self.texture_manager.resize(
            new_dpi_context.physical_size.width,
            new_dpi_context.physical_size.height,
        );
    }
}
```

### Phase 3: Advanced Features (Optional)

**3.1 Per-Monitor DPI Profiles**

Allow users to configure different font sizes per monitor:

```toml
# config.toml
[appearance]
font_family = "JetBrains Mono"
font_size = 14.0

[[appearance.monitor_overrides]]
name = "LG HDR WQHD"
font_size = 11.0
dpi = 109.0

[[appearance.monitor_overrides]]
name = "Built-in Retina Display"
font_size = 14.0
```

**3.2 Dynamic Font Size Adjustment**

Add keybindings for runtime font size changes:

```rust
// cmd/ctrl + '+' to increase font size
// cmd/ctrl + '-' to decrease font size
// cmd/ctrl + '0' to reset to configured size

pub fn adjust_font_size(&mut self, delta: f32) {
    self.font_manager.configured_font_size += delta;
    self.font_manager.effective_font_size =
        self.dpi_context.scale_font_size(
            self.font_manager.configured_font_size
        );
    self.font_manager.glyph_cache.clear();
}
```

**3.3 Smooth Font Size Transitions**

Animate font size changes for better UX:

```rust
pub struct FontSizeAnimation {
    start_size: f32,
    target_size: f32,
    progress: f32,  // 0.0 to 1.0
    duration_ms: f32,
}

impl FontSizeAnimation {
    pub fn update(&mut self, delta_time_ms: f32) -> f32 {
        self.progress = (self.progress + delta_time_ms / self.duration_ms).min(1.0);

        // Ease-out cubic interpolation
        let t = self.progress;
        let ease = 1.0 - (1.0 - t).powi(3);

        self.start_size + (self.target_size - self.start_size) * ease
    }
}
```

---

## Implementation Checklist

### Must-Have (Phase 1)
- [ ] Create `dpi.rs` module with `DpiContext` struct
- [ ] Update `FontManager` to track configured vs. effective font size
- [ ] Add DPI context parameter to `FontManager::new()`
- [ ] Implement `FontManager::update_dpi()` method
- [ ] Update glyph cache to use effective font size
- [ ] Add `dpi_override` to `AppearanceConfig`
- [ ] Modify `Renderer` to track DPI context
- [ ] Handle `WindowEvent::ScaleFactorChanged`
- [ ] Update surface configuration on DPI change

### Should-Have (Phase 2)
- [ ] Detect monitor changes and update DPI
- [ ] Add keybindings for font size adjustment (Cmd +/-)
- [ ] Persist font size changes to config
- [ ] Add logging for DPI changes (debug mode)
- [ ] Test on multiple displays with different DPI

### Nice-to-Have (Phase 3)
- [ ] Per-monitor font size configuration
- [ ] Smooth font size transition animations
- [ ] Auto-adjust font size based on window size
- [ ] Font size presets (small/medium/large)

---

## Testing Strategy

### Test Cases

1. **Single Display - Standard DPI**
   - Start app on 1080p display
   - Verify font renders at configured size
   - Resize window → font size stays consistent

2. **Single Display - HiDPI (Retina)**
   - Start app on Retina display (2x scale)
   - Verify font is 2x physical size
   - Verify visual appearance matches intended point size

3. **Multi-Monitor - Mixed DPI**
   - Start on Retina MacBook display (2x)
   - Drag to external 1080p monitor (1x)
   - Verify font size adjusts smoothly
   - Verify no infinite resize loops

4. **Runtime Font Size Changes**
   - Press Cmd+Plus → font increases
   - Press Cmd+Minus → font decreases
   - Press Cmd+0 → resets to config value
   - Verify glyph cache clears appropriately

5. **Configuration Persistence**
   - Change font size at runtime
   - Restart application
   - Verify new font size is used

### Performance Metrics

- **Glyph cache hit rate:** >95% after warm-up
- **DPI change latency:** <16ms (single frame)
- **Cache clear time:** <5ms
- **Memory usage:** Cache size should stabilize at ~256 glyphs

---

## Migration Path

### Step 1: Add DPI Support (Non-Breaking)
```rust
// Old code still works
let font_manager = FontManager::new("JetBrains Mono", 14.0)?;

// New code with DPI awareness
let dpi_context = DpiContext::new(window.scale_factor(), window.inner_size());
let font_manager = FontManager::new_with_dpi("JetBrains Mono", 14.0, dpi_context)?;
```

### Step 2: Deprecate Old Constructor
Add deprecation warning:
```rust
#[deprecated(since = "0.2.0", note = "Use new_with_dpi instead")]
pub fn new(font_family: &str, font_size: f32) -> Result<Self> {
    // Default to 1.0 scale factor for backward compatibility
    let dpi_context = DpiContext::new(1.0, PhysicalSize::new(1920, 1080));
    Self::new_with_dpi(font_family, font_size, dpi_context)
}
```

### Step 3: Update All Call Sites
Use IDE refactoring to update all instantiations.

---

## Risk Mitigation

### Potential Issues

1. **Cache Thrashing on Monitor Switch**
   - **Risk:** Frequent cache clears degrade performance
   - **Mitigation:** Keep separate caches per DPI level (max 2-3)

2. **Incorrect DPI Detection**
   - **Risk:** winit reports wrong scale factor
   - **Mitigation:** Allow `dpi_override` in config

3. **Font Size Feels Wrong**
   - **Risk:** User expectations vary by platform
   - **Mitigation:** Platform-specific defaults + easy adjustment

4. **Breaking Changes**
   - **Risk:** Changing FontManager API breaks downstream code
   - **Mitigation:** Gradual deprecation path (see Migration)

---

## Success Criteria

1. **Font size visually consistent** across displays with same DPI
2. **Smooth transitions** when moving windows between monitors
3. **No performance degradation** (maintain >60 FPS)
4. **User can override** DPI detection if needed
5. **Configuration persists** across restarts
6. **Zero crashes** related to font scaling

---

## References

### Documentation
- [Alacritty Configuration](https://alacritty.org/config-alacritty.html)
- [WezTerm DPI Documentation](https://wezfurlong.org/wezterm/config/lua/config/dpi.html)
- [Rio Terminal](https://rioterm.com/)
- [fontdue Documentation](https://docs.rs/fontdue/latest/fontdue/)
- [winit DPI Handling](https://docs.rs/winit/latest/winit/dpi/index.html)

### GitHub Issues Reviewed
- [Alacritty #4523](https://github.com/alacritty/alacritty/issues/4523) - Infinite window resizing
- [Alacritty #3465](https://github.com/alacritty/alacritty/issues/3465) - Different font size on multiple displays
- [WezTerm #4096](https://github.com/wezterm/wezterm/issues/4096) - Per-screen DPI override
- [WezTerm #5453](https://github.com/wezterm/wezterm/issues/5453) - Custom DPI ignored after resize

### Code Examples
- Alacritty font rendering pipeline
- WezTerm `window-resized` event handling
- WezTerm DPI configuration system

---

## Appendix A: Platform-Specific Considerations

### macOS
- **Default DPI:** 72pt (standard), 144pt (Retina)
- **Detection:** `NSScreen.backingScaleFactor` via winit
- **Behavior:** Automatic DPI switching when dragging between displays
- **Issue:** Font may appear too small/large during transition

### Windows
- **Default DPI:** 96 (100% scaling), varies with user settings
- **Detection:** `GetDpiForMonitor()` via winit
- **Behavior:** Per-monitor DPI awareness (Windows 8.1+)
- **Issue:** Mixed DPI setups common (laptop + external monitor)

### Linux (X11)
- **Default DPI:** 96, often overridden in `~/.Xresources` (`Xft.dpi`)
- **Detection:** Query `Xft.dpi` property
- **Behavior:** Global DPI setting (no per-monitor)
- **Issue:** HiDPI often requires manual configuration

### Linux (Wayland)
- **Default DPI:** 96 (standard), 192 (HiDPI)
- **Detection:** Compositor reports scale factor
- **Behavior:** Per-output scaling
- **Issue:** Fractional scaling can cause blurry rendering

---

## Appendix B: Alternative Approaches Considered

### Approach 1: Fixed Physical Font Size
**Concept:** Always render at a fixed physical pixel size (e.g., 28px), regardless of DPI.

**Pros:**
- Simple implementation
- No DPI detection needed

**Cons:**
- Font appears smaller on HiDPI displays
- Poor user experience
- **Rejected**

### Approach 2: Window-Size-Based Scaling
**Concept:** Scale font size as percentage of window height.

**Pros:**
- Responsive to window resizing
- No DPI detection needed

**Cons:**
- Font size changes when resizing window (bad UX)
- Terminal grid size becomes unpredictable
- **Rejected**

### Approach 3: Dual Font Manager (Recommended)
**Concept:** Maintain separate font managers for different DPI levels.

**Pros:**
- Faster DPI transitions (no cache clearing)
- Supports quick monitor switching

**Cons:**
- Higher memory usage
- More complex state management
- **Worth exploring in Phase 3**

---

## Appendix C: Fontdue API Reference

Key methods for DPI-aware rendering:

```rust
// Get font metrics at specific size
pub fn horizontal_line_metrics(&self, px: f32) -> Option<LineMetrics>

// Rasterize glyph at specific size
pub fn rasterize(&self, character: char, px: f32) -> (Metrics, Vec<u8>)

// Get glyph advance width
pub fn metrics(&self, character: char, px: f32) -> Metrics
```

**Note:** `px` parameter is in **physical pixels**, not points. Must multiply logical font size by scale factor before passing to fontdue.

---

## Next Steps

1. **Review this proposal** with the team
2. **Validate approach** with a prototype (Phase 1, minimal)
3. **Implement Phase 1** (DPI detection + basic scaling)
4. **Test on multiple displays**
5. **Iterate based on feedback**
6. **Consider Phase 2/3** based on user needs

---

**End of Proposal**
