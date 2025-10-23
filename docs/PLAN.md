# Saternal - Blazing Fast Dropdown Terminal Implementation Plan

## Architecture Overview
**Base**: Fork Alacritty (proven terminal core with `alacritty_terminal` crate)
**Target**: macOS-native dropdown terminal with global hotkey, tabs, and splits
**Performance**: Metal GPU acceleration, minimal latency

---

## Phase 1: Foundation & Project Setup

### 1.1 Initialize Rust Workspace
```
saternal/
├── Cargo.toml (workspace)
├── saternal/          # Main app
├── saternal-core/     # Terminal logic (wraps alacritty_terminal)
└── saternal-macos/    # macOS-specific native code
```

**Dependencies:**
- `alacritty_terminal = "0.25"` - Terminal emulation core
- `wgpu = "0.18"` - GPU rendering (Metal backend)
- `winit = "0.29"` - Window creation
- `global-hotkey = "0.5"` - System-wide Cmd+` binding
- `cocoa = "0.25"` - macOS native APIs
- `objc = "0.2"` - Objective-C bridging
- `tokio = "1.0"` - Async runtime for PTY I/O

### 1.2 macOS Bundle Setup
- Create Info.plist with accessibility permissions
- Configure entitlements for global hotkeys
- Setup .app bundle structure

---

## Phase 2: Core Terminal Engine

### 2.1 Integrate Alacritty Terminal Core
- Wrap `alacritty_terminal::Term` for PTY management
- Implement event loop using `alacritty_terminal::event_loop`
- Setup VTE parser for terminal escape sequences
- Handle terminal grid updates and scrollback

### 2.2 GPU-Accelerated Renderer
- Use wgpu with Metal backend (not OpenGL - faster on macOS)
- Implement glyph atlas for text rendering
- Font rasterization with `fontdue` or `rustybuzz`
- Render pipeline: clear → draw glyphs → present
- Target 60+ FPS at 4K resolution

---

## Phase 3: Dropdown Window & Hotkey System

### 3.1 Global Hotkey Registration
```rust
use global_hotkey::{GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

let manager = GlobalHotKeyManager::new()?;
let hotkey = HotKey::new(Some(Modifiers::META), Code::Backquote); // Cmd+`
manager.register(hotkey)?;
```

### 3.2 Native macOS Window Behavior
Using `cocoa` crate for precise control:
- Borderless window (NSWindowStyleMask::Borderless)
- Always on top: `window.setLevel_(CGShieldingWindowLevel() as i64)`
- Full screen width, ~50% height
- Positioned at top of screen (x=0, y=0)

### 3.3 Dropdown Animation
- Smooth slide down/up with Core Animation
- Animation duration: 150-200ms (feels instant)
- Use native `NSAnimationContext` for 60fps animations
- Track visibility state for toggle behavior

---

## Phase 4: Tabs & Pane Splits

### 4.1 Tab Management
- Tab bar at top (minimal design, 24px height)
- Each tab = separate `alacritty_terminal::Term` + PTY
- Keyboard shortcuts:
  - `Cmd+T` - new tab
  - `Cmd+W` - close tab
  - `Cmd+1-9` - switch to tab N
  - `Cmd+Shift+[/]` - prev/next tab

### 4.2 Split Panes (inspired by Zellij/tmux)
- Recursive pane tree structure
- Horizontal/vertical splits
- Each pane = independent `Term` instance
- Shortcuts:
  - `Cmd+D` - vertical split
  - `Cmd+Shift+D` - horizontal split
  - `Cmd+H/J/K/L` - navigate panes (vim-style)
  - `Cmd+Ctrl+H/J/K/L` - resize panes

**Implementation approach:**
```rust
enum PaneNode {
    Leaf { term: Term, pty: Pty },
    Split { direction: SplitDir, children: Vec<PaneNode>, ratio: f32 }
}
```

---

## Phase 5: Configuration & Theming

### 5.1 TOML Configuration
Location: `~/.config/saternal/config.toml`
```toml
[window]
width_percentage = 100
height_percentage = 50
animation_duration_ms = 180

[hotkey]
toggle = "cmd+`"

[appearance]
theme = "tokyo-night"
font_family = "JetBrains Mono"
font_size = 14
```

### 5.2 Hot Reload
- Watch config file with `notify` crate
- Apply changes without restart (font, theme, etc.)

---

## Phase 6: Performance Optimization

### 6.1 Fast Startup
- Lazy initialization of hidden tabs/panes
- Preload font atlas on app launch
- Keep window hidden but ready (don't destroy/recreate)

### 6.2 Rendering Optimizations
- Only redraw dirty regions (terminal damage tracking)
- Vsync with Metal's CAMetalLayer
- GPU-resident glyph cache
- Target: <1ms render time for typical updates

### 6.3 Memory Efficiency
- Limit scrollback per pane (default 10k lines)
- Shared font atlas across all panes
- Efficient grid storage (copy-on-write for history)

---

## Phase 7: Polish & UX

### 7.1 Visual Polish
- Background blur effect (vibrancy on macOS)
- Smooth animations for tab/pane operations
- Visual feedback for pane focus
- Minimal but clear UI chrome

### 7.2 Integration Testing
- Test with Claude Code CLI
- Verify vim/emacs/tmux work correctly
- Test Unicode rendering (emoji, CJK, etc.)
- Performance benchmark vs Alacritty/Ghostty

---

## Implementation Timeline

**Week 1-2**: Foundation (Phases 1-2)
- Setup project, integrate Alacritty core, basic rendering

**Week 3**: Dropdown + Hotkey (Phase 3)
- Global hotkey, window animations, toggle behavior

**Week 4**: Tabs (Phase 4.1)
- Multi-tab support, tab UI, keyboard shortcuts

**Week 5**: Splits (Phase 4.2)
- Pane splitting, navigation, resizing

**Week 6**: Config + Polish (Phases 5-7)
- Configuration system, optimization, testing

---

## Key Technical Challenges

1. **Window focus management** - Ensure Cmd+` works from any app
2. **Animation smoothness** - 60fps dropdown without jank
3. **PTY multiplexing** - Efficient I/O for many panes
4. **Font rendering** - Fast, crisp text at all sizes
5. **Memory usage** - Keep lean with many tabs/panes

## Success Metrics
- Startup time: <100ms (cold start)
- Toggle latency: <200ms (hotkey press → visible)
- Frame rate: 60fps sustained during scrolling
- Memory: <100MB with 5 tabs, 10 panes total
