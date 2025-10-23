# Terminal Emulator UX/UI Research - Rust Implementations

**Author:** Research conducted for Saternal Terminal
**Date:** October 24, 2025
**Status:** DRAFT

## Executive Summary

This document analyzes UX/UI patterns from leading Rust-based terminal emulators to inform the design of Saternal. Based on research of Alacritty, Warp, Zellij, WezTerm, and Rio, we've identified key design patterns, performance considerations, and user experience best practices that should guide Saternal's UI/UX development.

---

## Table of Contents

1. [Terminal Emulators Analyzed](#terminal-emulators-analyzed)
2. [Core UX/UI Patterns](#core-uxui-patterns)
3. [Performance & Rendering](#performance--rendering)
4. [Cursor Design & Behavior](#cursor-design--behavior)
5. [Color & Theming Systems](#color--theming-systems)
6. [Text Rendering & Typography](#text-rendering--typography)
7. [Window & Pane Management](#window--pane-management)
8. [Scrollback & Navigation](#scrollback--navigation)
9. [Configuration Philosophy](#configuration-philosophy)
10. [Recommendations for Saternal](#recommendations-for-saternal)

---

## Terminal Emulators Analyzed

### 1. **Alacritty**
- **GitHub:** 60.7k stars
- **Focus:** GPU-accelerated, minimal, maximum performance
- **Architecture:** OpenGL/Metal rendering, Rust + VTE parser
- **Philosophy:** Sensible defaults, configuration over features

**Key Strengths:**
- Blazing fast rendering (25-29% CPU vs iTerm's 130-140%)
- Zero input latency through GPU acceleration
- Clean, minimal design
- Cross-platform consistency

### 2. **Warp**
- **Focus:** Modern developer productivity, AI integration
- **Philosophy:** IDE-like features in terminal context
- **Key Innovation:** Command blocks, AI assistance, collaborative features

**Key Strengths:**
- Block-based command output (game changer for UX)
- Modern onboarding and discoverability
- Rich text editing in command line
- Visual workflow organization

### 3. **Zellij**
- **GitHub:** Active development
- **Focus:** Terminal workspace/multiplexer with excellent UX
- **Philosophy:** User-friendly alternative to tmux

**Key Strengths:**
- Intuitive pane management with visual feedback
- Status bar with contextual information
- Plugin system for extensibility
- Floating panes and layouts
- Mouse support as first-class citizen

### 4. **WezTerm**
- **Focus:** Cross-platform consistency, configurability
- **Configuration:** Lua-based config system
- **Philosophy:** Single config for all platforms

**Key Strengths:**
- Unified experience across macOS, Linux, Windows
- Extensive customization via Lua
- Built-in multiplexing
- Hyperlink support
- Dynamic color schemes

### 5. **Rio Terminal**
- **GitHub:** 5.8k stars
- **Focus:** Hardware-accelerated, web and desktop
- **Technology:** WebGPU + Rust + WebAssembly
- **Philosophy:** Modern web and native

**Key Strengths:**
- True color support (16 million colors)
- Images in terminal (Sixel, iTerm2 protocol)
- Font ligatures
- RetroArch shader support
- Cross-platform including web

---

## Core UX/UI Patterns

### 1. **Visual Feedback & Responsiveness**

**Pattern:** Immediate visual feedback for all user actions

**Best Practices:**
- Cursor animations (smooth, not jarring)
- Scroll indicators showing position in history
- Visual state changes (focus, selection, search)
- Hover states for clickable elements

**Examples from Research:**
- **Alacritty:** Smooth cursor implementation with configurable blink
- **Zellij:** Status bar shows current mode, active panes highlighted
- **Warp:** Command blocks provide clear visual boundaries

### 2. **Discoverability**

**Pattern:** Users should discover features without reading docs

**Best Practices:**
- Status bars with hints (Zellij approach)
- Keybinding overlays (similar to Zellij's mode indicators)
- Contextual help
- Smart defaults

**Implementation Ideas for Saternal:**
```
┌─────────────────────────────────────────────────┐
│ Terminal Content                                │
│                                                 │
└─────────────────────────────────────────────────┘
│ ⌘K: Search  ⌘T: New Tab  ⌘+: Zoom  ⌘?: Help    │ ← Status hints
└─────────────────────────────────────────────────┘
```

### 3. **Progressive Disclosure**

**Pattern:** Show simple interface by default, reveal complexity on demand

**Examples:**
- **Warp:** Settings accessible but not overwhelming
- **Alacritty:** Config file-based (power user friendly)
- **Zellij:** Layouts hidden until needed

### 4. **Non-Blocking Interactions**

**Pattern:** UI should never feel laggy or blocking

**Technical Approach:**
- Async rendering pipeline (already implemented in Saternal)
- Separate input handling from rendering
- GPU acceleration for all visual updates
- Try-lock pattern for terminal access (good - already in Saternal)

---

## Performance & Rendering

### GPU Acceleration Best Practices

**Findings from Alacritty:**
- Text rasterization on CPU, upload to GPU texture
- Single fullscreen quad rendering
- Minimal draw calls per frame
- Metal/Vulkan/WebGPU for modern performance

**Saternal's Current Implementation (Good!):**
```rust
// Already following best practices:
- wgpu for GPU rendering ✓
- Texture-based text rendering ✓
- Cursor as overlay pipeline ✓
- Efficient vertex buffer usage ✓
```

**Recommendations:**
- ✅ Continue current GPU architecture
- Consider glyph caching (as seen in glyph-brush implementations)
- Monitor texture atlas size for font management
- Profile render pass overhead

### Frame Pacing

**Pattern:** Consistent frame timing for smooth experience

**Best Practices:**
- Cap at display refresh rate (60/120/144Hz)
- Smart dirty tracking (only render when needed)
- Separate blink timer from render loop

**Saternal Status:**
```rust
// Already implements cursor blink tracking:
let blink_changed = self.cursor_state.update_blink(); ✓
```

---

## Cursor Design & Behavior

### Cursor Animations

**Research Findings:**

1. **Smooth Cursor Movement** (Alacritty fork)
   - Interpolated position changes
   - Easing functions for natural feel
   - 60fps animation target

2. **Cursor Styles**
   - Block (traditional, high visibility)
   - Beam (modern, text-editor like)
   - Underline (minimal)

3. **Blinking Behavior**
   - Configurable on/off
   - Respect terminal escape sequences (DECTCEM)
   - Pause blink on input

**Cursor Rendering Architecture (from research):**
```
Terminal State → Cursor Position → GPU Uniform → Shader → Overlay Render
```

**Saternal Implementation (Excellent!):**
```rust
// saternal-core/src/renderer/cursor/state.rs
- Separate cursor pipeline ✓
- Blink state management ✓
- DECTCEM mode support ✓
- GPU uniform upload ✓
```

**Enhancement Ideas:**
- Smooth cursor movement animations
- Trail effects (inspired by Ghostty/smear-cursor)
- Configurable cursor shapes per mode (insert/normal)

### Cursor Visibility Best Practices

**Pattern:** Clear visual indication of input focus

**Rules:**
1. Hide cursor when scrolled up in history
2. Show cursor on any input
3. Dim when terminal loses focus
4. Respect `CSI ? 25 h/l` escape sequences

**Current Saternal Implementation:**
```rust
// Already handles SHOW_CURSOR mode correctly ✓
let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR);

// Enhancement: Consider scroll offset
if self.scroll_offset > 0 {
    hide_cursor = true; // Don't show cursor in history
}
```

---

## Color & Theming Systems

### Theme Architecture Patterns

**1. Base16 / Base24 Systems**
- 16 colors minimum (ANSI)
- Extended palettes for modern terminals
- Semantic color names

**2. True Color Support** (24-bit)
- Rio: "up to 16 million colors"
- Modern standard for terminal emulators
- RGB color specifications

**3. Dynamic Theme Switching**
- WezTerm: Runtime theme changes
- Alacritty: Config reload on change

### Color Palette Structure

```rust
// Recommended structure (from Alacritty patterns)
pub struct ColorPalette {
    // Primary colors
    background: Color,
    foreground: Color,

    // Cursor
    cursor: Color,
    cursor_text: Color,

    // Selection
    selection_background: Color,
    selection_foreground: Option<Color>,

    // Normal colors (0-7)
    normal: [Color; 8],

    // Bright colors (8-15)
    bright: [Color; 8],

    // Dim colors (optional)
    dim: Option<[Color; 8]>,
}
```

### UI Color Best Practices

**Background:**
- Default: Dark (reduces eye strain)
- Support: Light themes with proper contrast
- Opacity: Optional transparency (with blur on macOS)

**Foreground:**
- High contrast ratio (WCAG AA minimum: 4.5:1)
- Consider color blindness (avoid red/green only indicators)

**Accent Colors:**
- Selection: Distinct but not jarring (typically muted blue)
- Search matches: High visibility (yellow/orange backgrounds)
- Active elements: Clear but not distracting

---

## Text Rendering & Typography

### Font Rendering Pipeline

**From Research (Alacritty, Rio):**

```
Font File → Rasterization (fontdue/swash) → Glyph Cache → GPU Texture → Render
```

**Saternal Current Approach:**
```rust
// Using fontdue ✓
FontManager → TextRasterizer → GPU Texture Upload
```

### Font Features

**Must-Have:**
- [x] Monospace font support (Saternal ✓)
- [x] Font size adjustment (Saternal ✓)
- [ ] Font ligatures (Rio has this)
- [ ] Font fallback for missing glyphs
- [ ] Emoji support (color emoji)

**Font Ligatures:**
Rio and WezTerm support programming ligatures (e.g., `->` → `→`, `==` → `═`). This improves code readability.

**Implementation Path:**
1. Upgrade to swash (has ligature support)
2. Enable OpenType features
3. Cache composite glyphs

### Cell Dimensions & Baseline

**Best Practices:**
```rust
// Saternal already does this correctly ✓
let cell_width = font.metrics('M', font_size).advance_width;
let cell_height = (ascent - descent + line_gap).ceil();
let baseline_offset = ascent.ceil();
```

**Recommendations:**
- Allow slight cell padding for readability (1-2px)
- Configurable line height multiplier
- Bold font weight handling (avoid double-width issues)

---

## Window & Pane Management

### Single Window Model (Alacritty)
- One window = one terminal
- Simple, performant
- Relies on OS/WM for window management

### Multiplexer Model (Zellij, WezTerm)

**Pane Split Patterns:**
```
Horizontal Split:
┌─────────────────┐
│   Pane 1        │
├─────────────────┤
│   Pane 2        │
└─────────────────┘

Vertical Split:
┌────────┬────────┐
│ Pane 1 │ Pane 2 │
│        │        │
└────────┴────────┘

Complex Layout:
┌────────┬────────┐
│        │ Pane 2 │
│ Pane 1 ├────────┤
│        │ Pane 3 │
└────────┴────────┘
```

**Zellij's UX Innovations:**
- Visual split indicators during resize
- Pane borders with focus highlighting
- Floating panes (overlays)
- Predefined layouts

### Tab Management

**Best Practices:**
- Tab bar (top or bottom)
- Current tab highlighting
- Tab titles (customizable)
- Tab switching shortcuts (⌘1-9)
- Tab reordering

**Visual Design:**
```
┌──────┬──────┬──────────┬──────┐
│ Tab1 │ Tab2 │ Tab3 ●   │  +   │  ← Active tab different color
└──────┴──────┴──────────┴──────┘
```

---

## Scrollback & Navigation

### Scrollback Buffer

**Standard Patterns:**
- History size: Configurable (default 10,000-100,000 lines)
- Memory management: Ring buffer
- Search in scrollback: Essential feature

**Saternal Current Implementation:**
```rust
pub fn scroll(&mut self, delta: i32) {
    if delta > 0 {
        // Scroll up into history
        self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
    } else if delta < 0 {
        // Scroll down toward present
        self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
    }
}
```

**Enhancement Ideas:**
1. **Scroll Position Indicator**
   ```
   ┌─────────────────────┐
   │                     │ ▲
   │   Terminal          │ █  ← Visual indicator
   │   Content           │ ║
   │                     │ ▼
   └─────────────────────┘
   ```

2. **Jump to Bottom Button** (when scrolled up)
   ```
   [⬇ Jump to Bottom]  ← Appears when scroll_offset > 0
   ```

3. **Search in Scrollback**
   - Highlight all matches
   - Jump to next/previous
   - Incremental search

### Smooth Scrolling

**Best Practice:** Interpolate scroll position over frames

```rust
// Pseudocode
struct ScrollState {
    target_offset: usize,
    current_offset: f32,
    velocity: f32,
}

// Update each frame
fn update_scroll(&mut self, delta_time: f32) {
    let diff = self.target_offset as f32 - self.current_offset;
    self.velocity += diff * SPRING_CONSTANT;
    self.velocity *= DAMPING;
    self.current_offset += self.velocity * delta_time;
}
```

---

## Configuration Philosophy

### Alacritty Approach: Config File
**Pros:**
- Version controllable
- Shareable across machines
- Power user friendly

**Cons:**
- Steeper learning curve
- No GUI for discovery

**Format:** YAML/TOML
```toml
[font]
size = 14.0
family = "SF Mono"

[colors.primary]
background = "#002b36"
foreground = "#839496"

[cursor]
style = "Block"
blink = true
```

### Warp Approach: GUI + Config
**Pros:**
- Discoverable
- User-friendly
- Still powerful

**Cons:**
- More complex to implement
- Sync issues between GUI and config

### Zellij Approach: Sensible Defaults + Overrides
**Pros:**
- Works great out of box
- Progressive complexity
- Plugin system for extensions

### Recommendation for Saternal

**Hybrid Approach:**
1. **Great defaults** (works immediately)
2. **Config file** for power users
3. **Future:** Simple GUI for common settings
4. **Hot reload** (detect config changes)

**Config Location:**
```
~/.config/saternal/config.toml
```

**Priority Order:**
1. Runtime overrides (CLI flags)
2. User config
3. Default config

---


## Design Principles for Saternal

Based on research, Saternal should follow these principles:

### 1. **Performance First**
- GPU acceleration is non-negotiable
- Zero input latency
- 60fps minimum for all animations
- Efficient memory usage

### 2. **Sensible Defaults**
- Works great out of box
- No configuration required to start
- Beautiful default theme
- Standard keybindings

### 3. **Progressive Disclosure**
- Simple surface, powerful depth
- Features discoverable when needed
- Not overwhelming for beginners

### 4. **Modern UX**
- Smooth animations (where appropriate)
- Visual feedback for all actions
- Mouse support as first-class
- Keyboard-first workflow

### 5. **Respect Standards**
- Full VT100/xterm compatibility
- Standard escape sequences
- ANSI color support
- True color support

### 6. **Native Feel**
- Platform-specific optimizations
- System integration (themes, notifications)
- Follows OS conventions

---

## Visual Design System

### Spacing & Layout

**Grid System:**
```
Base unit: 8px (cell height aligned)
Padding: 8px, 16px, 24px
Margins: 16px, 24px, 32px
```

**Typography:**
```
Terminal text: Monospace, 12-16pt (configurable)
UI text: System font, 11-13pt
Status bar: System font, 10pt
```

### Animation Timing

**Durations:**
- Micro-interactions: 100-150ms (cursor blink, hover)
- Transitions: 200-300ms (tab switch, pane focus)
- Major changes: 300-500ms (theme switch, layout change)

**Easing:**
- Standard: ease-in-out
- Enter: ease-out
- Exit: ease-in
- Bouncy: spring (for playful elements)

### Color Usage

**Semantic Colors:**
```rust
pub struct UIColors {
    // Background
    background: Color,           // Main terminal bg
    background_alt: Color,       // Tab bar, status bar

    // Foreground
    foreground: Color,           // Main text
    foreground_dim: Color,       // Secondary text

    // Accents
    accent: Color,               // Interactive elements
    accent_hover: Color,         // Hover states

    // Status
    success: Color,              // Green
    warning: Color,              // Yellow/Orange
    error: Color,                // Red
    info: Color,                 // Blue

    // UI Elements
    border: Color,               // Pane borders
    border_active: Color,        // Active pane
    selection: Color,            // Text selection
}
```

---

## Implementation Roadmap & Task Tracking

**Progress Overview:**
```
Phase 1: Core UX Polish        [████░░░░░░] 40% (4/10 tasks)
Phase 2: User Interaction      [░░░░░░░░░░]  0% (0/15 tasks)
Phase 3: Window Management     [░░░░░░░░░░]  0% (0/12 tasks)
Phase 4: Advanced Features     [░░░░░░░░░░]  0% (0/8 tasks)
Phase 5: Configuration         [░░░░░░░░░░]  0% (0/10 tasks)

Overall Progress: [██░░░░░░░░] 7% (4/55 tasks)
```

### Phase 1: Core UX Polish (Current) - 2-3 weeks

**Goal:** Polish the core rendering experience with smooth interactions and visual feedback.

#### 1.1 Cursor System Enhancements

- [x] **Task 1.1.1:** Basic cursor rendering (COMPLETED)
  - File: `saternal-core/src/renderer/cursor/state.rs`

- [x] **Task 1.1.2:** Cursor blink support (COMPLETED)
  - File: `saternal-core/src/renderer/cursor/state.rs`

- [ ] **Task 1.1.3:** Hide cursor when scrolled in history
  - File: `saternal-core/src/renderer/mod.rs:161`
  - Time: 30 minutes
  - Code:
    ```rust
    let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR)
                      || self.scroll_offset > 0;
    ```

- [ ] **Task 1.1.4:** Configurable cursor shapes (Block/Beam/Underline)
  - Files: `cursor/config.rs`, `cursor/state.rs`, `cursor/pipeline.rs`
  - Time: 3-4 hours
  - Steps:
    1. Create CursorShape enum
    2. Update CursorConfig struct
    3. Modify shader for different shapes
    4. Update uniform buffer

- [ ] **Task 1.1.5:** Smooth cursor movement animations
  - File: `saternal-core/src/renderer/cursor/state.rs`
  - Time: 2-3 hours
  - Implementation: Interpolate between positions with easing

- [ ] **Task 1.1.6:** Cursor color customization
  - Files: `cursor/config.rs`, `config.rs`
  - Time: 1 hour

#### 1.2 Scrollback Indicators

- [ ] **Task 1.2.1:** Visual scrollbar overlay
  - Files: `renderer/scrollbar/mod.rs` (new), `renderer/mod.rs`
  - Time: 4-5 hours
  - Design: 8px width, semi-transparent, right edge, auto-hide

- [ ] **Task 1.2.2:** Scroll position percentage indicator
  - File: `renderer/scrollbar/mod.rs`
  - Time: 2 hours
  - Shows "X%" or "Line Y of Z", fades after 2s

- [ ] **Task 1.2.3:** "Jump to Bottom" button
  - Files: `renderer/ui/mod.rs` (new), `input.rs`
  - Time: 3-4 hours
  - Appears when `scroll_offset > 100`, click or Shift+G/End

- [ ] **Task 1.2.4:** Smooth scrolling animation
  - File: `renderer/mod.rs`
  - Time: 3 hours
  - Spring-based interpolation, 60fps

#### 1.3 Color & Theme System

- [ ] **Task 1.3.1:** Define ColorPalette structure
  - Files: `theme/mod.rs` (new), `theme/palette.rs` (new)
  - Time: 2 hours
  - Structure: primary, cursor, selection, ANSI colors, UI colors

- [ ] **Task 1.3.2:** Create default theme
  - File: `theme/defaults.rs` (new)
  - Time: 1-2 hours
  - Beautiful dark theme, WCAG AA compliant

- [ ] **Task 1.3.3:** Implement theme loading from TOML
  - Files: `theme/loader.rs` (new), `config.rs`
  - Time: 3 hours
  - Parse TOML, validate colors, fallback on error

- [ ] **Task 1.3.4:** Popular theme presets (5 themes)
  - Files: `themes/solarized-dark.toml`, `dracula.toml`, `nord.toml`, `gruvbox.toml`, `catppuccin.toml`
  - Time: 2-3 hours

- [ ] **Task 1.3.5:** Hot reload theme on config change
  - Files: `theme/loader.rs`, `app.rs`
  - Time: 2 hours
  - Use notify crate to watch file

#### 1.4 Font System Improvements

- [x] **Task 1.4.1:** Basic font rendering (COMPLETED)

- [x] **Task 1.4.2:** Font size adjustment (COMPLETED)

- [ ] **Task 1.4.3:** Font ligature support
  - Files: Update Cargo.toml, `font.rs`, `text_rasterizer.rs`
  - Time: 6-8 hours (complex)
  - Switch from fontdue to swash, enable OpenType features

- [ ] **Task 1.4.4:** Bold/italic font variants
  - File: `font.rs`
  - Time: 4-5 hours
  - Load variants or synthetic bold fallback

- [ ] **Task 1.4.5:** Font fallback chain
  - Files: `font.rs`, `config.rs`
  - Time: 5-6 hours
  - Try multiple fonts for missing glyphs, emoji support

---

### Phase 2: User Interaction - 3-4 weeks

**Goal:** Implement essential user interactions for productivity.

#### 2.1 Text Selection

- [ ] **Task 2.1.1:** Mouse selection (drag to select)
  - Files: `selection/mod.rs` (new), `input.rs`
  - Time: 6-8 hours

- [ ] **Task 2.1.2:** Keyboard selection (Shift+arrows)
  - Files: `selection/mod.rs`, `input.rs`
  - Time: 4-5 hours

- [ ] **Task 2.1.3:** Smart selection (double-click word, URLs, paths)
  - File: `selection/smart.rs` (new)
  - Time: 5-6 hours
  - Patterns: URLs, file paths, IPs

- [ ] **Task 2.1.4:** Render selection highlighting
  - Files: `renderer/selection/mod.rs` (new), `renderer/mod.rs`
  - Time: 4-5 hours

#### 2.2 Clipboard Integration

- [ ] **Task 2.2.1:** Copy to clipboard (Cmd+C)
  - Files: `clipboard.rs` (new), `input.rs`
  - Time: 3-4 hours
  - Dependency: arboard crate

- [ ] **Task 2.2.2:** Paste from clipboard (Cmd+V)
  - Files: `clipboard.rs`, `input.rs`
  - Time: 3-4 hours
  - Multiline paste confirmation, bracket paste mode

- [ ] **Task 2.2.3:** Primary selection (Linux middle-click)
  - File: `clipboard.rs`
  - Time: 2 hours (Linux only)

#### 2.3 Search Functionality

- [ ] **Task 2.3.1:** Basic search UI (Cmd+F)
  - Files: `search/mod.rs` (new), `search/ui.rs` (new)
  - Time: 5-6 hours

- [ ] **Task 2.3.2:** Search in scrollback buffer
  - File: `search/engine.rs` (new)
  - Time: 4-5 hours

- [ ] **Task 2.3.3:** Regex search support
  - File: `search/engine.rs`
  - Time: 3 hours
  - Dependency: regex crate

- [ ] **Task 2.3.4:** Incremental search
  - File: `search/mod.rs`
  - Time: 2-3 hours

- [ ] **Task 2.3.5:** Search match highlighting
  - File: `renderer/search/mod.rs` (new)
  - Time: 3-4 hours

#### 2.4 Keyboard Shortcuts

- [ ] **Task 2.4.1:** Define keyboard shortcut system
  - Files: `keybindings/mod.rs` (new), `keybindings/parser.rs` (new)
  - Time: 4-5 hours

- [ ] **Task 2.4.2:** Implement standard shortcuts
  - File: `keybindings/defaults.rs` (new)
  - Time: 3-4 hours
  - Shortcuts: Cmd+C/V/F/K/T/W, Cmd+Plus/Minus/0

- [ ] **Task 2.4.3:** Configurable key bindings
  - File: `config.rs`
  - Time: 2-3 hours

#### 2.5 Mouse Interactions

- [ ] **Task 2.5.1:** Mouse event handling
  - File: `input.rs`
  - Time: 3-4 hours

- [ ] **Task 2.5.2:** Hover states for UI elements
  - File: `renderer/ui/mod.rs`
  - Time: 2-3 hours

---

### Phase 3: Window Management - 4-5 weeks

**Goal:** Implement tabs and splits for power users.

#### 3.1 Tab System

- [ ] **Task 3.1.1:** Tab data structure
  - File: `tab.rs` (enhance)
  - Time: 2-3 hours

- [ ] **Task 3.1.2:** Tab bar UI
  - File: `renderer/tabbar/mod.rs` (new)
  - Time: 6-8 hours

- [ ] **Task 3.1.3:** Tab switching (Cmd+1-9, Cmd+Tab)
  - File: `app.rs`
  - Time: 2-3 hours

- [ ] **Task 3.1.4:** New tab (Cmd+T)
  - File: `app.rs`
  - Time: 3-4 hours

- [ ] **Task 3.1.5:** Close tab (Cmd+W)
  - File: `app.rs`
  - Time: 2-3 hours

- [ ] **Task 3.1.6:** Tab titles and customization
  - File: `tab.rs`
  - Time: 4-5 hours

#### 3.2 Split Panes

- [ ] **Task 3.2.1:** Pane layout system (tree structure)
  - Files: `pane.rs` (enhance), `layout/mod.rs` (new)
  - Time: 8-10 hours

- [ ] **Task 3.2.2:** Split pane horizontally (Cmd+D)
  - File: `app.rs`
  - Time: 5-6 hours

- [ ] **Task 3.2.3:** Split pane vertically (Cmd+Shift+D)
  - File: `app.rs`
  - Time: 2-3 hours

- [ ] **Task 3.2.4:** Pane focus navigation
  - File: `app.rs`
  - Time: 4-5 hours

- [ ] **Task 3.2.5:** Pane borders and visual focus
  - File: `renderer/pane/mod.rs` (new)
  - Time: 5-6 hours

- [ ] **Task 3.2.6:** Resize panes with mouse/keyboard
  - Files: `app.rs`, `input.rs`
  - Time: 6-8 hours

---

### Phase 4: Advanced Features - 4-6 weeks

**Goal:** Differentiate Saternal with modern features.

#### 4.1 Hyperlink Support

- [ ] **Task 4.1.1:** OSC 8 hyperlink parsing
  - File: `terminal.rs`
  - Time: 4-5 hours

- [ ] **Task 4.1.2:** Hyperlink rendering
  - File: `renderer/text_rasterizer.rs`
  - Time: 3-4 hours

- [ ] **Task 4.1.3:** Click to open links
  - File: `input.rs`
  - Time: 3-4 hours

#### 4.2 Image Support

- [ ] **Task 4.2.1:** Sixel image protocol
  - File: `image/sixel.rs` (new)
  - Time: 12-15 hours

- [ ] **Task 4.2.2:** iTerm2 inline images
  - File: `image/iterm2.rs` (new)
  - Time: 8-10 hours

- [ ] **Task 4.2.3:** Image rendering in terminal
  - File: `renderer/image/mod.rs` (new)
  - Time: 10-12 hours

#### 4.3 Advanced Rendering

- [ ] **Task 4.3.1:** Custom shader support
  - File: `renderer/shaders/custom.rs` (new)
  - Time: 8-10 hours

- [ ] **Task 4.3.2:** Background blur/transparency
  - File: `renderer/mod.rs`
  - Time: 6-8 hours
  - Platform: macOS initially

---

### Phase 5: Configuration & Polish - 3-4 weeks

**Goal:** Make Saternal production-ready.

#### 5.1 Configuration System

- [ ] **Task 5.1.1:** TOML config file structure
  - File: `config.rs`
  - Time: 4-5 hours
  - Location: `~/.config/saternal/config.toml`

- [ ] **Task 5.1.2:** Config validation and error handling
  - File: `config/validation.rs` (new)
  - Time: 3-4 hours

- [ ] **Task 5.1.3:** Hot reload config
  - File: `app.rs`
  - Time: 4-5 hours

- [ ] **Task 5.1.4:** CLI flag overrides
  - File: `main.rs`
  - Time: 3-4 hours
  - Dependency: clap crate

#### 5.2 Performance Optimization

- [ ] **Task 5.2.1:** Glyph cache optimization
  - Files: `font.rs`, `renderer/text_rasterizer.rs`
  - Time: 5-6 hours

- [ ] **Task 5.2.2:** Dirty region tracking
  - File: `renderer/mod.rs`
  - Time: 6-8 hours

- [ ] **Task 5.2.3:** Frame rate profiling
  - File: `profiling.rs` (new)
  - Time: 3-4 hours

#### 5.3 Platform-Specific Polish

- [ ] **Task 5.3.1:** macOS native window decorations
  - File: `saternal-macos/src/window.rs`
  - Time: 5-6 hours

- [ ] **Task 5.3.2:** macOS trackpad gestures
  - File: `saternal-macos/src/window.rs`
  - Time: 6-8 hours

- [ ] **Task 5.3.3:** Linux window manager integration
  - Time: 8-10 hours

#### 5.4 Documentation & Testing

- [ ] **Task 5.4.1:** User documentation
  - Files: `README.md`, `docs/USER_GUIDE.md`, `docs/CONFIGURATION.md`
  - Time: 6-8 hours

- [ ] **Task 5.4.2:** Developer documentation
  - Files: `docs/ARCHITECTURE.md`, `docs/CONTRIBUTING.md`
  - Time: 4-5 hours

- [ ] **Task 5.4.3:** Integration tests
  - Dir: `saternal/tests/` (new)
  - Time: 10-12 hours

- [ ] **Task 5.4.4:** Performance benchmarks
  - Dir: `saternal/benches/` (new)
  - Time: 6-8 hours
  - Dependency: criterion crate

---

## Technical Debt to Avoid

Based on lessons from other terminals:

1. **Don't reinvent everything**
   - Use battle-tested parsers (VTE, alacritty_terminal) ✓
   - Leverage existing font renderers ✓
   - Follow established standards

2. **Plan for configuration early**
   - Don't hardcode values
   - Make things configurable from start
   - Document config options

3. **Test on real hardware**
   - Different GPUs behave differently
   - Test on integrated graphics
   - Profile memory usage

4. **Unicode is hard**
   - Test with emoji
   - Test with CJK characters
   - Test with RTL languages
   - Handle combining characters

5. **Measure everything**
   - Frame times
   - Input latency
   - Memory usage
   - Battery impact

---

## Competitive Analysis Summary

| Feature | Alacritty | Warp | Zellij | WezTerm | Rio | Saternal Goal |
|---------|-----------|------|--------|---------|-----|---------------|
| GPU Acceleration | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ |
| Splits/Panes | ❌ | ✅ | ✅ | ✅ | ✅ | 🎯 |
| Tabs | ❌ | ✅ | ✅ | ✅ | ❌ | 🎯 |
| Ligatures | ✅ | ✅ | ❌ | ✅ | ✅ | 🎯 |
| Images | ❌ | ✅ | ❌ | ✅ | ✅ | 🎯 |
| Config File | ✅ | ✅ | ✅ | ✅ | ✅ | 🎯 |
| GUI Settings | ❌ | ✅ | ❌ | ❌ | ❌ | Future |
| AI Features | ❌ | ✅ | ❌ | ❌ | ❌ | Future? |
| Web Version | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ |

---

## Conclusion

Saternal is already on the right track with its GPU-accelerated architecture using wgpu. The current cursor and rendering implementation follows best practices from Alacritty.

**Next Steps:**
1. Implement scroll indicators for better navigation UX
2. Build out theme/color system
3. Add font ligature support
4. Implement selection and clipboard
5. Design and implement tab/pane management

**Key Differentiator Ideas:**
- **Smoothness**: Animations everywhere (cursor, scroll, transitions)
- **Beauty**: Stunning default themes, modern design
- **Speed**: Alacritty-level performance with better UX
- **Polish**: Attention to detail in every interaction

The research shows that users value:
1. **Performance** (Alacritty's success)
2. **Productivity** (Warp's innovation)
3. **Usability** (Zellij's approach)
4. **Customization** (WezTerm's flexibility)
5. **Modern features** (Rio's ambition)

Saternal can combine the best of all these: Alacritty's performance + Zellij's UX + WezTerm's cross-platform consistency + modern visual design.

---

## References

- [Alacritty GitHub](https://github.com/alacritty/alacritty)
- [Warp Terminal](https://www.warp.dev/)
- [Zellij Terminal](https://zellij.dev/)
- [WezTerm](https://wezfurlong.org/wezterm/)
- [Rio Terminal](https://github.com/raphamorim/rio)
- [GPU Terminal Rendering (Zutty)](https://tomscii.sig7.se/2020/11/How-Zutty-works)
- [Announcing Alacritty](https://jwilm.io/blog/announcing-alacritty/)

---

**Last Updated:** October 24, 2025
**Next Review:** After implementing Phase 1 recommendations
