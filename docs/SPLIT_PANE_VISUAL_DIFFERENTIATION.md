# Split Pane Visual Differentiation - Design Proposal

**Author:** Research based on WezTerm, tmux, and other terminal emulators
**Date:** 2025-10-25
**Status:** Proposal
**Related Files:**
- `saternal/src/app/window.rs`
- `saternal-core/src/renderer/mod.rs`
- `saternal-core/src/pane.rs`
- `saternal-core/src/selection/renderer.rs`

---

## Table of Contents

1. [Problem Statement](#problem-statement)
2. [Current State](#current-state)
3. [Research: Industry Approaches](#research-industry-approaches)
4. [Proposed Design Options](#proposed-design-options)
5. [Recommended Implementation](#recommended-implementation)
6. [Technical Implementation Details](#technical-implementation-details)
7. [Configuration API](#configuration-api)
8. [Implementation Roadmap](#implementation-roadmap)

---

## Problem Statement

When using split panes in Saternal, users currently have no visual indication of:
1. **Which pane is currently active/focused** - Users cannot easily tell where keyboard input will go
2. **Boundaries between panes** - While 2-pixel gaps exist, they're not visually rendered
3. **Inactive panes** - No differentiation between panes that are in focus vs background

This creates usability issues, especially when working with 3+ panes simultaneously.

**User Requirements:**
- Instantly identify the active pane without reading content
- Clear visual separation between panes
- Non-intrusive design that doesn't distract from terminal content
- Consistent with modern terminal emulator UX patterns

---

## Current State

### What Exists

**Infrastructure (✅ Implemented):**
```rust
// PaneViewport tracks focused state
pub struct PaneViewport {
    pub pane_id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub focused: bool,  // ✅ Already tracked!
}

// Pane tree tracks focus
pub struct Pane {
    pub id: usize,
    pub terminal: Terminal,
    pub focused: bool,  // ✅ Already tracked!
}
```

**Viewport Calculation (`saternal-core/src/selection/renderer.rs`):**
- ✅ 2-pixel gaps reserved between panes
- ✅ Viewports calculated with positions and dimensions
- ✅ Focused state passed through viewport metadata

**Rendering Pipeline (`saternal-core/src/renderer/mod.rs`):**
- ✅ `render_with_panes()` orchestrates multi-pane rendering
- ✅ Parallel rendering to separate buffers
- ✅ GPU shader infrastructure (wgpu) ready

### What's Missing (❌ Not Implemented)

**Border Rendering (`saternal-core/src/renderer/mod.rs:render_pane_borders()`):**
```rust
fn render_pane_borders(&self, render_pass: &mut wgpu::RenderPass, viewports: &[PaneViewport]) {
    // ❌ Currently just logging - no actual rendering!
    for viewport in viewports {
        let color = if viewport.focused { "blue" } else { "gray" };
        log::trace!("Pane {} - {}", viewport.pane_id, color);
    }
}
```

**Missing Components:**
1. ❌ Border shader (similar to `selection.wgsl`)
2. ❌ Border geometry generation (vertex buffers)
3. ❌ Active/inactive color configuration
4. ❌ Dimming/opacity for inactive panes
5. ❌ Border style options (solid, gradient, thickness)

---

## Research: Industry Approaches

### 1. **WezTerm** (Rust + OpenGL/Metal)

**Primary Approach: Pane Dimming**
```lua
-- WezTerm Configuration
config.inactive_pane_hsb = {
    hue = 1.0,        -- No hue shift
    saturation = 0.8, -- Slightly desaturate (0.0 = grayscale)
    brightness = 0.7, -- Dim to 70% brightness
}
```

**How It Works:**
- Applies HSB (Hue, Saturation, Brightness) transformation to inactive panes during rendering
- Changes entire pane content appearance, not just borders
- GPU shader applies color transformation in real-time

**Border Support:**
- ❌ **No per-pane border colors** currently supported
- ✅ Window-level borders only
- Community has requested this feature (see GitHub #3337, #297)

**Pros:**
- Very clear visual distinction
- Subtle and non-intrusive
- Easy to implement with fragment shaders

**Cons:**
- Cannot disable dimming if user prefers only border indication
- May reduce readability of background panes
- No explicit border highlighting

---

### 2. **tmux** (Terminal-based)

**Primary Approach: Colored Borders + Optional Dimming**

```bash
# Border Colors
set -g pane-border-style 'fg=colour238'
set -g pane-active-border-style 'fg=blue'

# Optional: Background Dimming
set -g window-style 'fg=colour247,bg=colour236'
set -g window-active-style 'fg=default,bg=colour234'
```

**How It Works:**
- Draws borders using Unicode box-drawing characters in terminal cells
- Active pane gets bright colored border (e.g., blue)
- Inactive panes get dimmer gray borders
- Can optionally dim entire inactive pane backgrounds

**Border Rendering Strategy:**
- Uses actual terminal cells for borders (not GPU overlays)
- For vertical splits: colors top/bottom halves differently
- For horizontal splits: colors left/right segments differently

**Pros:**
- ✅ Very clear active pane indication
- ✅ Highly customizable colors
- ✅ Works in any terminal (text-based)
- ✅ Can combine border colors + dimming

**Cons:**
- Consumes terminal cell space for borders
- Limited to monochrome borders (one color per line segment)

---

### 3. **Kitty** (GPU-accelerated)

**Primary Approach: Inactive Text Alpha**

```conf
# Kitty Configuration
inactive_text_alpha 0.7
active_border_color #00ff00
inactive_border_color #555555
```

**How It Works:**
- Applies alpha transparency to inactive pane text (0.0-1.0)
- GPU shader multiplies text color alpha during rendering
- Borders rendered as GPU rectangles with configurable colors

**Pros:**
- Clear visual hierarchy
- Preserves terminal content visibility
- GPU-efficient

**Cons:**
- Transparency may reduce text contrast on some backgrounds

---

### 4. **Windows Terminal**

**Approach: Border Colors Only**

```json
{
    "unfocusedAppearance": {
        "colorScheme": "Dimmed"
    }
}
```

**How It Works:**
- Each pane can have different color scheme when unfocused
- Uses GPU to render colored borders around panes

**Pros:**
- No content modification (borders only)
- Very clear active indication

---

## Proposed Design Options

Based on research and Saternal's GPU rendering architecture, here are **three design approaches** (can be combined):

### **Option A: Border Color Differentiation** (Primary Recommendation)

Render GPU-based borders with different colors for active vs inactive panes.

**Visual Example:**
```
┌─────────────────────┬─────────────────────┐
│                     │                     │
│   ACTIVE PANE       │   INACTIVE PANE     │
│   (Blue Border)     │   (Gray Border)     │
│                     │                     │
└─────────────────────┴─────────────────────┘
```

**Implementation:**
- Use wgpu shader to render colored rectangles
- Similar to `selection.wgsl` but for borders
- Render borders AFTER terminal content but BEFORE cursor

**Configuration:**
```toml
[panes.borders]
enabled = true
thickness = 2  # pixels
active_color = "#4A90E2"     # Blue
inactive_color = "#3C3C3C"   # Dark gray
style = "solid"  # or "gradient"
```

**Pros:**
- ✅ Clear, instant visual feedback
- ✅ Doesn't modify terminal content
- ✅ Highly customizable
- ✅ Industry standard approach (tmux, Windows Terminal)
- ✅ GPU-efficient (minimal overdraw)

**Cons:**
- Requires shader implementation
- May clip 1-2 pixels of terminal content at edges

---

### **Option B: Inactive Pane Dimming** (WezTerm-style)

Apply HSB/brightness transformation to inactive panes.

**Visual Example:**
```
┌──────────────────────┬──────────────────────┐
│                      │                      │
│   ACTIVE PANE        │   INACTIVE PANE      │
│   ■ Full Brightness  │   ■ 70% Brightness   │
│                      │   (Dimmed)           │
└──────────────────────┴──────────────────────┘
```

**Implementation:**
- Modify fragment shader in text rasterizer
- Multiply final color by brightness factor for inactive panes
- Pass `focused` flag through to shader via uniform

**Configuration:**
```toml
[panes.inactive]
enabled = true
brightness = 0.7      # 0.0 = black, 1.0 = no change
saturation = 0.85     # 0.0 = grayscale, 1.0 = no change
opacity = 1.0         # Optional alpha blending
```

**Shader Pseudocode:**
```wgsl
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = sample_text_texture(in.tex_coords);

    if (!is_focused_pane) {
        // Apply dimming
        color.rgb *= config.inactive_brightness;

        // Optional: desaturate
        let gray = dot(color.rgb, vec3(0.299, 0.587, 0.114));
        color.rgb = mix(vec3(gray), color.rgb, config.inactive_saturation);
    }

    return color;
}
```

**Pros:**
- ✅ Entire pane visually distinct
- ✅ Proven approach (WezTerm)
- ✅ Subtle and professional

**Cons:**
- ❌ May reduce readability of background panes
- ❌ Some users prefer content to remain unchanged
- ❌ Harder to debug if you need to read inactive pane output

---

### **Option C: Combined Approach** (Best of Both Worlds)

Use **both** border colors AND optional dimming, with user configuration.

**Configuration:**
```toml
[panes]
# Border Configuration
border_enabled = true
border_thickness = 2
border_active_color = "#4A90E2"
border_inactive_color = "#3C3C3C"

# Optional Dimming
dim_inactive = true
inactive_brightness = 0.8
inactive_saturation = 0.9
```

**Pros:**
- ✅ Maximum flexibility
- ✅ Users can choose borders only, dimming only, or both
- ✅ Strongest visual differentiation

**Cons:**
- More configuration complexity
- More implementation work

---

## Recommended Implementation

**Phase 1: Border Color Differentiation (Option A)**

Start with GPU-rendered colored borders as the foundation.

**Why:**
1. ✅ Non-destructive (doesn't modify content)
2. ✅ Clear, universally understood pattern
3. ✅ Can add dimming later if desired
4. ✅ Easier to implement than dimming (no shader modifications to text rendering)

**Phase 2: Add Dimming Support (Option B)**

Once borders work, add optional inactive pane dimming.

**Phase 3: Refinements**

- Gradient borders
- Border animations on focus change
- Configurable border thickness
- Per-pane custom colors

---

## Technical Implementation Details

### Architecture Changes

```
┌─────────────────────────────────────────────────────────┐
│                  Render Pipeline                        │
├─────────────────────────────────────────────────────────┤
│  1. Calculate Pane Viewports (existing)                 │
│  2. Render Each Pane to Buffer (existing)               │
│  3. Composite Buffers to Window (existing)              │
│  4. ✨ NEW: Render Pane Borders (with colors)           │
│  5. Render Selection Highlights (existing)              │
│  6. Render Cursor (existing)                            │
└─────────────────────────────────────────────────────────┘
```

### Step-by-Step Implementation

#### **Step 1: Create Border Shader**

**File:** `saternal-core/src/shaders/border.wgsl`

```wgsl
// Vertex shader for border rectangles
struct BorderVertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(vertex: BorderVertex) -> VertexOutput {
    var out: VertexOutput;
    // Convert pixel coordinates to clip space (-1 to 1)
    out.clip_position = vec4<f32>(
        (vertex.position.x / screen_width) * 2.0 - 1.0,
        1.0 - (vertex.position.y / screen_height) * 2.0,
        0.0,
        1.0
    );
    out.color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
```

#### **Step 2: Generate Border Geometry**

**File:** `saternal-core/src/renderer/borders.rs` (new file)

```rust
use crate::selection::renderer::PaneViewport;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BorderVertex {
    position: [f32; 2],
    color: [f32; 4],
}

pub struct BorderConfig {
    pub thickness: u32,
    pub active_color: [f32; 4],    // RGBA
    pub inactive_color: [f32; 4],
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            thickness: 2,
            active_color: [0.29, 0.56, 0.89, 1.0],  // #4A90E2 blue
            inactive_color: [0.24, 0.24, 0.24, 1.0], // #3C3C3C gray
        }
    }
}

/// Generate border vertices for a single pane
pub fn generate_border_vertices(
    viewport: &PaneViewport,
    config: &BorderConfig,
) -> Vec<BorderVertex> {
    let color = if viewport.focused {
        config.active_color
    } else {
        config.inactive_color
    };

    let thickness = config.thickness as f32;
    let x = viewport.x as f32;
    let y = viewport.y as f32;
    let w = viewport.width as f32;
    let h = viewport.height as f32;

    let mut vertices = Vec::new();

    // Top border (rectangle)
    vertices.extend_from_slice(&[
        BorderVertex { position: [x, y], color },
        BorderVertex { position: [x + w, y], color },
        BorderVertex { position: [x, y + thickness], color },
        BorderVertex { position: [x + w, y + thickness], color },
    ]);

    // Bottom border
    vertices.extend_from_slice(&[
        BorderVertex { position: [x, y + h - thickness], color },
        BorderVertex { position: [x + w, y + h - thickness], color },
        BorderVertex { position: [x, y + h], color },
        BorderVertex { position: [x + w, y + h], color },
    ]);

    // Left border
    vertices.extend_from_slice(&[
        BorderVertex { position: [x, y], color },
        BorderVertex { position: [x + thickness, y], color },
        BorderVertex { position: [x, y + h], color },
        BorderVertex { position: [x + thickness, y + h], color },
    ]);

    // Right border
    vertices.extend_from_slice(&[
        BorderVertex { position: [x + w - thickness, y], color },
        BorderVertex { position: [x + w, y], color },
        BorderVertex { position: [x + w - thickness, y + h], color },
        BorderVertex { position: [x + w, y + h], color },
    ]);

    vertices
}

/// Generate vertices for all pane borders
pub fn generate_all_border_vertices(
    viewports: &[PaneViewport],
    config: &BorderConfig,
) -> Vec<BorderVertex> {
    viewports
        .iter()
        .flat_map(|vp| generate_border_vertices(vp, config))
        .collect()
}
```

#### **Step 3: Render Borders in GPU Pass**

**File:** `saternal-core/src/renderer/mod.rs`

Update the `render_pane_borders` implementation:

```rust
fn render_pane_borders(
    &self,
    render_pass: &mut wgpu::RenderPass,
    viewports: &[PaneViewport],
) -> Result<()> {
    use crate::renderer::borders::{generate_all_border_vertices, BorderConfig};

    let config = BorderConfig::default(); // TODO: Load from user config
    let vertices = generate_all_border_vertices(viewports, &config);

    if vertices.is_empty() {
        return Ok(());
    }

    // Create vertex buffer (or reuse if cached)
    let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Pane Border Vertex Buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });

    // Set pipeline and draw
    render_pass.set_pipeline(&self.border_pipeline);
    render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    render_pass.draw(0..vertices.len() as u32, 0..1);

    Ok(())
}
```

#### **Step 4: Initialize Border Pipeline**

**File:** `saternal-core/src/renderer/mod.rs`

Add to `Renderer` struct initialization:

```rust
impl Renderer {
    pub async fn new(config: Config) -> Result<Self> {
        // ... existing initialization ...

        // Load border shader
        let border_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Border Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/border.wgsl").into()),
        });

        // Create border render pipeline
        let border_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Border Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &border_shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BorderVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // Color
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &border_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Store in Renderer struct
        Ok(Self {
            // ... existing fields ...
            border_pipeline,
        })
    }
}
```

---

## Configuration API

### TOML Configuration

**File:** `~/.config/saternal/config.toml`

```toml
[panes.borders]
# Enable/disable pane borders
enabled = true

# Border thickness in pixels (1-10)
thickness = 2

# Active pane border color (hex or named)
active_color = "#4A90E2"     # Blue
# Or use named colors: "blue", "green", "red", etc.

# Inactive pane border color
inactive_color = "#3C3C3C"   # Dark gray

# Border style
style = "solid"  # Options: "solid", "gradient" (future), "dashed" (future)

# Optional: Gradient configuration (Phase 3)
# gradient_direction = "horizontal"  # or "vertical"
# gradient_start_color = "#4A90E2"
# gradient_end_color = "#9B59B6"

[panes.inactive]
# Enable dimming of inactive panes (Phase 2)
dim_enabled = false

# Brightness multiplier for inactive panes (0.0-1.0)
# 1.0 = no dimming, 0.0 = black
brightness = 0.8

# Saturation multiplier (0.0-1.0)
# 1.0 = no change, 0.0 = grayscale
saturation = 0.9

# Optional opacity (0.0-1.0)
opacity = 1.0
```

### Rust Configuration Struct

**File:** `saternal-core/src/config.rs`

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaneBorderConfig {
    pub enabled: bool,
    pub thickness: u32,
    pub active_color: String,
    pub inactive_color: String,
    pub style: BorderStyle,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum BorderStyle {
    Solid,
    Gradient,  // Future
    Dashed,    // Future
}

impl Default for PaneBorderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thickness: 2,
            active_color: "#4A90E2".to_string(),
            inactive_color: "#3C3C3C".to_string(),
            style: BorderStyle::Solid,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaneInactiveConfig {
    pub dim_enabled: bool,
    pub brightness: f32,
    pub saturation: f32,
    pub opacity: f32,
}

impl Default for PaneInactiveConfig {
    fn default() -> Self {
        Self {
            dim_enabled: false,
            brightness: 0.8,
            saturation: 0.9,
            opacity: 1.0,
        }
    }
}
```

---

## Implementation Roadmap

### Phase 1: Basic Border Rendering (Week 1-2)

**Goals:**
- ✅ Render solid colored borders around panes
- ✅ Different colors for active vs inactive panes
- ✅ Basic configuration support

**Tasks:**
1. Create `border.wgsl` shader
2. Implement `borders.rs` module with vertex generation
3. Initialize border pipeline in renderer
4. Update `render_pane_borders()` to actually render
5. Add configuration struct
6. Test with 2, 3, 4+ pane layouts

**Success Criteria:**
- Active pane has blue border
- Inactive panes have gray borders
- Borders render without gaps or overlaps
- No performance degradation

---

### Phase 2: Inactive Pane Dimming (Week 3-4)

**Goals:**
- ✅ Add HSB/brightness adjustment for inactive panes
- ✅ User-configurable dimming strength
- ✅ Optional feature (can disable)

**Tasks:**
1. Modify text fragment shader to accept focused flag
2. Implement HSB color transformation
3. Pass focused state through shader uniforms
4. Add configuration for brightness/saturation
5. Test readability at different dimming levels

**Success Criteria:**
- Inactive panes visibly dimmed
- Text remains readable
- No flickering on focus change
- Configurable dimming intensity

---

### Phase 3: Advanced Features (Week 5+)

**Optional Enhancements:**
1. **Gradient Borders**
   - Smooth color transitions on borders
   - Configurable gradient direction

2. **Border Animations**
   - Subtle fade/pulse on focus change
   - Configurable animation duration

3. **Custom Border Thickness**
   - Per-pane thickness override
   - Adaptive thickness based on window size

4. **Border Styles**
   - Dashed borders
   - Dotted borders
   - Double-line borders

5. **Accessibility**
   - High contrast mode
   - Alternative focus indicators (beyond color)
   - Screen reader hints

---

## Visual Mockups

### Single Split (2 Panes)

```
┌────────────────────────────────────────────────────┐
│                  Saternal                          │
├────────────────────────┬───────────────────────────┤
│ ┏━━━━━━━━━━━━━━━━━━━━┓ │ ┌─────────────────────┐ │
│ ┃ $ vim src/main.rs  ┃ │ │ $ cargo build       │ │
│ ┃                    ┃ │ │                     │ │
│ ┃ fn main() {        ┃ │ │ Compiling saternal  │ │
│ ┃     println!();    ┃ │ │ v0.1.0              │ │
│ ┃ }                  ┃ │ │                     │ │
│ ┃                    ┃ │ │ Finished in 2.3s    │ │
│ ┗━━━━━━━━━━━━━━━━━━━━┛ │ └─────────────────────┘ │
│    ACTIVE (Blue)        │   INACTIVE (Gray)       │
└────────────────────────┴───────────────────────────┘
```

### Triple Split (with Dimming)

```
┌──────────────────────────────────────────────────────┐
│                    Saternal                          │
├──────────────────────┬───────────────────────────────┤
│ ┏━━━━━━━━━━━━━━━━━━┓ │ ┌──────────────────────┐    │
│ ┃ $ vim main.rs    ┃ │ │ $ cargo test         │    │
│ ┃                  ┃ │ │ (70% brightness)     │    │
│ ┃ fn main() {      ┃ │ │ Running tests...     │    │
│ ┃     // code      ┃ │ └──────────────────────┘    │
│ ┃ }                ┃ ├─────────────────────────────┤
│ ┃                  ┃ │ ┌──────────────────────┐    │
│ ┃ ■ Cursor here    ┃ │ │ $ htop               │    │
│ ┗━━━━━━━━━━━━━━━━━━┛ │ │ (70% brightness)     │    │
│   ACTIVE              │ └──────────────────────┘    │
│   (Blue + Full)       │   INACTIVE (Gray + Dimmed)  │
└──────────────────────┴───────────────────────────────┘
```

---

## References

### Terminal Emulators Studied

1. **WezTerm** (Rust + GPU)
   - Config: `inactive_pane_hsb`
   - Approach: Pane content dimming
   - Docs: https://wezfurlong.org/wezterm/config/appearance.html

2. **tmux**
   - Config: `pane-active-border-fg`, `window-active-style`
   - Approach: Border colors + optional dimming
   - Implementation: Text-based borders

3. **Kitty**
   - Config: `inactive_text_alpha`, `active_border_color`
   - Approach: Text transparency + GPU borders

4. **Windows Terminal**
   - Config: `unfocusedAppearance`
   - Approach: Per-pane color schemes

### Key GitHub Issues/Discussions

- WezTerm #3337: Change border color for active pane
- WezTerm #297: Add pane-related styling options
- WezTerm #2842: Support configuration for inactive pane font opacity

---

## Conclusion

This proposal recommends a **phased implementation** starting with **GPU-rendered colored borders** (Phase 1), followed by **optional inactive pane dimming** (Phase 2).

**Why This Approach:**
1. ✅ **Non-intrusive** - Borders don't modify content
2. ✅ **Clear visual feedback** - Industry standard pattern
3. ✅ **Flexible** - Users can enable borders only, dimming only, or both
4. ✅ **GPU-efficient** - Minimal performance impact
5. ✅ **Extensible** - Can add gradients, animations, styles later

**Next Steps:**
1. Review and approve this design proposal
2. Begin Phase 1 implementation (border shader)
3. Test with real-world multi-pane workflows
4. Gather user feedback
5. Iterate on Phase 2 (dimming) based on feedback

---

**Questions or Feedback?**

Please review this proposal and provide feedback on:
- Design direction (borders vs dimming vs both)
- Default colors and thickness
- Configuration API
- Implementation priority
