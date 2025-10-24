# Split Pane Feature Proposal for Saternal

## Executive Summary

This proposal outlines the implementation of split pane functionality in Saternal, allowing users to divide the terminal window into multiple panes running independent shell sessions. Based on research of established Rust terminal emulators (WezTerm, Zellij, Alacritty+tmux), this document presents a comprehensive implementation strategy tailored to Saternal's architecture.

**Primary Goal**: Enable users to split the terminal window horizontally and vertically using keyboard shortcuts (like Ctrl+D), creating a tiling layout with multiple independent terminal sessions.

## Research Findings

### Industry Standard Implementations

#### 1. WezTerm (Rust Terminal Emulator with Built-in Multiplexing)

WezTerm provides the most comprehensive native split pane implementation:

**Architecture**:
- **Pane Tree Structure**: Hierarchical tree where each node is either a leaf (terminal pane) or a split container
- **Split Directions**: Horizontal (left/right) and Vertical (top/bottom)
- **Size Allocation**: Percentage-based or cell-based sizing with configurable ratios
- **Focus Management**: Active pane tracking with keyboard navigation between panes

**Key Features**:
```rust
// Conceptual WezTerm API structure
pane.split {
    direction: "Left" | "Right" | "Top" | "Bottom",
    size: { Percent: 50 } | { Cells: 80 },
    command: SpawnCommand,
    top_level: bool,  // Split entire tab vs current pane
}
```

**Keyboard Bindings**:
- Split operations: Configurable keys (commonly Ctrl+Shift+D for horizontal, Ctrl+D for vertical)
- Navigation: Ctrl+h/j/k/l (vim-style) or Ctrl+Arrow keys
- Resize: Ctrl+Shift+Arrow keys or mouse dragging

#### 2. Zellij (Modern Terminal Multiplexer in Rust)

Zellij focuses on user-friendly tiling with floating panes:

**Architecture**:
- Layout-based system with predefined and custom layouts
- Supports both tiled and floating panes
- Session persistence and reattachment

**Key Features**:
- Automatic layout adjustment when adding/removing panes
- Visual indicators for pane borders and focus
- Tab support with independent pane layouts per tab
- Simplified keybindings (Ctrl+p for pane mode, then direction keys)

#### 3. Terminal + tmux Pattern

Many terminal emulators (Alacritty, Kitty) rely on tmux for multiplexing:

**Advantages**:
- Proven stability and session persistence
- Extensive features (session management, scripting)

**Disadvantages**:
- External dependency
- Additional complexity for users
- Nested terminal environment can cause confusion

### Common Design Patterns

After analyzing multiple implementations, the following patterns emerge:

1. **Tree-based Layout Management**: All successful implementations use a tree structure for pane organization
2. **50/50 Default Split**: New panes typically take 50% of the parent pane's space
3. **Focus Tracking**: Single active pane receives keyboard input
4. **Recursive Resizing**: When window resizes, all panes proportionally adjust
5. **Split Direction Options**:
   - Split current pane (most common)
   - Split at top-level (creates splits across entire tab)

## Current Saternal Architecture Analysis

### Existing Components (✅ Already Implemented)

Saternal already has a strong foundation for split panes:

#### 1. Pane Tree Structure (`saternal-core/src/pane.rs`)

```rust
pub enum PaneNode {
    Leaf {
        pane: Pane,
    },
    Split {
        direction: SplitDirection,
        children: Vec<PaneNode>,
        ratio: f32,  // 0.0-1.0 for size allocation
    },
}

pub struct Pane {
    pub id: usize,
    pub terminal: Terminal,
    pub focused: bool,
}

pub enum SplitDirection {
    Horizontal,  // Left/right split
    Vertical,    // Top/bottom split
}
```

**Analysis**: This is an excellent foundation! The recursive tree structure matches industry best practices.

**Current Capabilities**:
- ✅ Tree-based pane organization
- ✅ Support for horizontal and vertical splits
- ✅ Focus tracking per pane
- ✅ Ratio-based size allocation
- ✅ Recursive resize operations

**Current Limitations**:
- ⚠️ Split always occurs at root level (see TODO comment: "Split only the focused pane")
- ⚠️ No keyboard shortcuts implemented for splitting
- ⚠️ No pane navigation between siblings
- ⚠️ No visual rendering of multiple panes (renderer assumes single pane)

#### 2. Tab Management (`saternal/src/tab.rs`)

```rust
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_tree: PaneNode,  // ✅ Already uses PaneNode!
    next_pane_id: usize,
}

impl Tab {
    pub fn split(&mut self, direction: SplitDirection, shell: Option<String>) -> Result<()> {
        // TODO: Split only the focused pane (currently splits root)
        self.pane_tree.split(direction, pane_id, 80, 24, shell)?;
        Ok(())
    }
}
```

**Analysis**: Tab structure is ready, just needs refined split logic.

#### 3. Terminal Management (`saternal-core/src/terminal.rs`)

Each `Pane` contains an independent `Terminal` instance:
- ✅ Separate PTY per terminal
- ✅ Independent resize capability
- ✅ Isolated input/output handling

**This is perfect for split panes** - each pane can have its own independent shell session.

### Missing Components (❌ Need Implementation)

1. **Keyboard Shortcuts for Splitting**
   - No Ctrl+D handler for vertical split
   - No Ctrl+Shift+D handler for horizontal split

2. **Pane Navigation**
   - No focus movement between panes
   - No visual indication of focused pane

3. **Pane Rendering**
   - Current renderer (`saternal-core/src/renderer/mod.rs`) assumes single terminal
   - Need viewport management for multiple panes

4. **Pane Borders**
   - No visual separator between panes
   - No focus indicator

5. **Pane Lifecycle Management**
   - No pane closing (Ctrl+W)
   - No pane rebalancing after close

## Proposed Implementation

### Phase 1: Core Split Functionality (Minimal Viable Product)

**Goal**: Enable basic splitting with Ctrl+D, render multiple panes side-by-side.

#### 1.1 Fix Focused Pane Splitting (`saternal-core/src/pane.rs`)

**Current Issue**: `Tab::split()` always splits the root node instead of the focused pane.

**Solution**: Implement recursive focused pane discovery and splitting.

```rust
impl PaneNode {
    /// Split the currently focused pane
    pub fn split_focused(
        &mut self,
        direction: SplitDirection,
        new_id: usize,
        shell: Option<String>,
    ) -> Result<bool> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => {
                // Found the focused pane - split it
                let (cols, rows) = pane.terminal.dimensions();

                // Calculate split dimensions based on direction
                let (new_cols, new_rows) = match direction {
                    SplitDirection::Vertical => (cols / 2, rows),
                    SplitDirection::Horizontal => (cols, rows / 2),
                };

                self.split(direction, new_id, new_cols, new_rows, shell)?;

                // Set focus to new pane
                if let PaneNode::Split { children, .. } = self {
                    if let Some(PaneNode::Leaf { pane }) = children.get_mut(1) {
                        children.get_mut(0).unwrap().clear_focus();
                        pane.focused = true;
                    }
                }

                Ok(true)
            }
            PaneNode::Leaf { .. } => Ok(false),
            PaneNode::Split { children, .. } => {
                // Recursively search children for focused pane
                for child in children.iter_mut() {
                    if child.split_focused(direction, new_id, shell.clone())? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Clear focus from all panes in this subtree
    fn clear_focus(&mut self) {
        match self {
            PaneNode::Leaf { pane } => pane.focused = false,
            PaneNode::Split { children, .. } => {
                for child in children {
                    child.clear_focus();
                }
            }
        }
    }
}
```

**Update `Tab::split()`**:
```rust
impl Tab {
    pub fn split(&mut self, direction: SplitDirection, shell: Option<String>) -> Result<()> {
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        if !self.pane_tree.split_focused(direction, pane_id, shell)? {
            log::warn!("No focused pane found to split");
        }

        Ok(())
    }
}
```

#### 1.2 Add Keyboard Shortcuts (`saternal/src/app.rs`)

Add split shortcuts in the `WindowEvent::KeyboardInput` handler:

```rust
// In App::run(), after line 373 (after Cmd+G handler)
KeyCode::KeyD => {
    // Ctrl+D: Split vertically (side-by-side)
    // Ctrl+Shift+D: Split horizontally (top-bottom)
    let ctrl = modifiers_state.state().control_key();
    if ctrl {
        let direction = if shift {
            SplitDirection::Horizontal
        } else {
            SplitDirection::Vertical
        };

        info!("Splitting pane: {:?}", direction);
        if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
            if let Err(e) = active_tab.split(direction, config.terminal.shell.clone()) {
                log::error!("Failed to split pane: {}", e);
            }
        }
        window.request_redraw();
        return;
    }
}
```

**Note**: Need to handle conflict with Ctrl+D (EOF/exit signal). Solutions:
- Option A: Require Shift for splits (Ctrl+Shift+D for vertical, Ctrl+Shift+Alt+D for horizontal)
- Option B: Use different key (Ctrl+\ and Ctrl+- like some terminal emulators)
- Option C: Check if shell prompt is active before sending Ctrl+D

**Recommendation**: Option B - Use Ctrl+\ for vertical split, Ctrl+- for horizontal split (common in terminal multiplexers).

#### 1.3 Implement Multi-Pane Rendering (`saternal-core/src/renderer/mod.rs`)

**Current State**: Renderer assumes single full-window terminal.

**Required Changes**:

1. **Add Pane Layout Calculation**:

```rust
/// Viewport for rendering a single pane
#[derive(Debug, Clone)]
pub struct PaneViewport {
    pub pane_id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

impl Renderer {
    /// Calculate viewports for all panes in a tree
    pub fn calculate_pane_viewports(
        &self,
        pane_tree: &PaneNode,
        window_width: u32,
        window_height: u32,
    ) -> Vec<PaneViewport> {
        let mut viewports = Vec::new();
        self.calculate_viewports_recursive(
            pane_tree,
            0, 0,
            window_width, window_height,
            &mut viewports
        );
        viewports
    }

    fn calculate_viewports_recursive(
        &self,
        node: &PaneNode,
        x: u32, y: u32,
        width: u32, height: u32,
        viewports: &mut Vec<PaneViewport>
    ) {
        match node {
            PaneNode::Leaf { pane } => {
                viewports.push(PaneViewport {
                    pane_id: pane.id,
                    x, y, width, height,
                    focused: pane.focused,
                });
            }
            PaneNode::Split { direction, children, ratio } => {
                let border_width = 2; // 2px border between panes

                match direction {
                    SplitDirection::Vertical => {
                        let split_x = (width as f32 * ratio) as u32;

                        if let Some(left) = children.get(0) {
                            self.calculate_viewports_recursive(
                                left,
                                x, y,
                                split_x.saturating_sub(border_width / 2),
                                height,
                                viewports
                            );
                        }

                        if let Some(right) = children.get(1) {
                            self.calculate_viewports_recursive(
                                right,
                                x + split_x + border_width / 2,
                                y,
                                width.saturating_sub(split_x + border_width),
                                height,
                                viewports
                            );
                        }
                    }
                    SplitDirection::Horizontal => {
                        let split_y = (height as f32 * ratio) as u32;

                        if let Some(top) = children.get(0) {
                            self.calculate_viewports_recursive(
                                top,
                                x, y,
                                width,
                                split_y.saturating_sub(border_width / 2),
                                viewports
                            );
                        }

                        if let Some(bottom) = children.get(1) {
                            self.calculate_viewports_recursive(
                                bottom,
                                x,
                                y + split_y + border_width / 2,
                                width,
                                height.saturating_sub(split_y + border_width),
                                viewports
                            );
                        }
                    }
                }
            }
        }
    }
}
```

2. **Update Main Render Loop**:

```rust
impl Renderer {
    /// Render multiple panes
    pub fn render_panes(
        &mut self,
        pane_tree: &PaneNode,
    ) -> Result<()> {
        let window_size = self.surface.get_current_texture()?;
        let view = window_size.texture.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());

        // Calculate viewports for all panes
        let (window_width, window_height) = (self.width, self.height);
        let viewports = self.calculate_pane_viewports(
            pane_tree,
            window_width,
            window_height
        );

        // Clear entire window
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }

        // Render each pane
        for viewport in viewports {
            // Get the terminal for this pane
            if let Some(pane) = self.find_pane(pane_tree, viewport.pane_id) {
                if let Some(term_lock) = pane.terminal.term().try_lock() {
                    self.render_pane_content(
                        &term_lock,
                        &view,
                        &mut encoder,
                        &viewport
                    )?;
                }
            }

            // Draw pane border
            if !viewport.focused {
                self.render_pane_border(&view, &mut encoder, &viewport, false)?;
            } else {
                self.render_pane_border(&view, &mut encoder, &viewport, true)?;
            }
        }

        self.queue.submit(Some(encoder.finish()));
        window_size.present();

        Ok(())
    }

    fn render_pane_content(
        &mut self,
        term: &Term<TermEventListener>,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        viewport: &PaneViewport,
    ) -> Result<()> {
        // Set viewport for scissoring/clipping
        // Render terminal content to this viewport
        // (Use existing render logic but constrained to viewport)

        // TODO: Implement viewport-constrained rendering
        Ok(())
    }

    fn render_pane_border(
        &mut self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        viewport: &PaneViewport,
        focused: bool,
    ) -> Result<()> {
        // Draw border rectangle
        // Use different color for focused pane (e.g., bright blue vs gray)

        // TODO: Implement border rendering
        Ok(())
    }
}
```

3. **Update App Render Call** (`saternal/src/app.rs:697`):

```rust
// Replace single terminal render with pane tree render
if let Err(e) = renderer.render_panes(&tab_mgr.active_tab().unwrap().pane_tree) {
    log::error!("Render error: {}", e);
}
```

### Phase 2: Pane Navigation

**Goal**: Allow users to move focus between panes with keyboard shortcuts.

#### 2.1 Implement Focus Navigation (`saternal-core/src/pane.rs`)

```rust
#[derive(Debug, Clone, Copy)]
pub enum NavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

impl PaneNode {
    /// Move focus in the specified direction
    pub fn navigate_focus(&mut self, direction: NavigationDirection) -> bool {
        // Find currently focused pane and its position
        // Find adjacent pane in the specified direction
        // Transfer focus to that pane

        // This requires spatial awareness of pane positions
        // Implementation can be simplified by tracking pane coordinates

        // TODO: Implement directional navigation
        false
    }

    /// Get the next pane in circular order (for Tab navigation)
    pub fn focus_next(&mut self) -> bool {
        let pane_ids = self.pane_ids();
        if let Some(current_idx) = self.focused_pane().map(|p| p.id) {
            let current_pos = pane_ids.iter().position(|&id| id == current_idx).unwrap();
            let next_id = pane_ids[(current_pos + 1) % pane_ids.len()];
            return self.set_focus(next_id);
        }
        false
    }

    /// Get the previous pane in circular order (for Shift+Tab navigation)
    pub fn focus_prev(&mut self) -> bool {
        let pane_ids = self.pane_ids();
        if let Some(current_idx) = self.focused_pane().map(|p| p.id) {
            let current_pos = pane_ids.iter().position(|&id| id == current_idx).unwrap();
            let prev_pos = if current_pos == 0 {
                pane_ids.len() - 1
            } else {
                current_pos - 1
            };
            let prev_id = pane_ids[prev_pos];
            return self.set_focus(prev_id);
        }
        false
    }
}
```

#### 2.2 Add Navigation Shortcuts (`saternal/src/app.rs`)

```rust
// In keyboard input handler, check for Ctrl+Alt+Arrow or Ctrl+hjkl
let ctrl = modifiers_state.state().control_key();
let alt = modifiers_state.state().alt_key();

if ctrl && alt {
    if let PhysicalKey::Code(keycode) = event.physical_key {
        let nav_dir = match keycode {
            KeyCode::ArrowLeft | KeyCode::KeyH => Some(NavigationDirection::Left),
            KeyCode::ArrowRight | KeyCode::KeyL => Some(NavigationDirection::Right),
            KeyCode::ArrowUp | KeyCode::KeyK => Some(NavigationDirection::Up),
            KeyCode::ArrowDown | KeyCode::KeyJ => Some(NavigationDirection::Down),
            _ => None,
        };

        if let Some(dir) = nav_dir {
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                if active_tab.pane_tree.navigate_focus(dir) {
                    window.request_redraw();
                }
            }
            return;
        }
    }
}

// Simple cycling with Ctrl+Tab / Ctrl+Shift+Tab
if ctrl && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
    if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
        if shift {
            active_tab.pane_tree.focus_prev();
        } else {
            active_tab.pane_tree.focus_next();
        }
        window.request_redraw();
    }
    return;
}
```

### Phase 3: Pane Lifecycle Management

**Goal**: Allow closing panes and rebalancing space.

#### 3.1 Implement Pane Closing

```rust
impl Tab {
    /// Close the currently focused pane
    pub fn close_focused_pane(&mut self) -> Result<()> {
        // Don't close if it's the last pane
        if self.pane_tree.pane_ids().len() <= 1 {
            return Ok(());
        }

        self.pane_tree.close_focused()?;

        // Focus the next available pane
        if let Some(first_id) = self.pane_tree.pane_ids().first() {
            self.pane_tree.set_focus(*first_id);
        }

        Ok(())
    }
}

impl PaneNode {
    /// Close the focused pane and rebalance the tree
    pub fn close_focused(&mut self) -> Result<bool> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => {
                // Can't close from leaf level - parent must handle
                Ok(true)
            }
            PaneNode::Leaf { .. } => Ok(false),
            PaneNode::Split { children, .. } => {
                // Check if a child is focused (leaf) and should be closed
                for i in 0..children.len() {
                    if let PaneNode::Leaf { pane } = &children[i] {
                        if pane.focused {
                            // Remove this child and collapse the split
                            if children.len() == 2 {
                                // Replace this split with the other child
                                let other_idx = 1 - i;
                                let other_child = children.remove(other_idx);
                                *self = other_child;
                                return Ok(true);
                            }
                        }
                    }

                    // Recursively check children
                    if children[i].close_focused()? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}
```

#### 3.2 Add Close Shortcut

```rust
// In keyboard handler
if ctrl {
    if let PhysicalKey::Code(KeyCode::KeyW) = event.physical_key {
        info!("Closing focused pane");
        if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
            if let Err(e) = active_tab.close_focused_pane() {
                log::error!("Failed to close pane: {}", e);
            }
        }
        window.request_redraw();
        return;
    }
}
```

### Phase 4: Pane Resizing (Future Enhancement)

- Mouse drag on borders to resize
- Keyboard shortcuts to adjust split ratio (Ctrl+Shift+< / >)
- Equalize all panes (Ctrl+Shift+E)

## Recommended Keyboard Shortcuts

Based on research and avoiding conflicts:

| Action | Shortcut | Alternative | Notes |
|--------|----------|-------------|-------|
| Split Vertical (side-by-side) | Ctrl+\\ | Ctrl+Shift+V | Creates left/right split |
| Split Horizontal (top-bottom) | Ctrl+- | Ctrl+Shift+H | Creates top/bottom split |
| Navigate Left | Ctrl+Alt+H | Ctrl+Alt+← | Vim-style preferred |
| Navigate Right | Ctrl+Alt+L | Ctrl+Alt+→ | Vim-style preferred |
| Navigate Up | Ctrl+Alt+K | Ctrl+Alt+↑ | Vim-style preferred |
| Navigate Down | Ctrl+Alt+J | Ctrl+Alt+↓ | Vim-style preferred |
| Cycle Panes Forward | Ctrl+Tab | - | Circular through panes |
| Cycle Panes Backward | Ctrl+Shift+Tab | - | Circular (reverse) |
| Close Focused Pane | Ctrl+W | Ctrl+Shift+W | Common in browsers |
| Zoom Pane (toggle fullscreen) | Ctrl+Z | - | Phase 4 feature |

## Implementation Roadmap

### Week 1: Core Split (Phase 1)
- Day 1-2: Fix focused pane splitting logic
- Day 3-4: Add keyboard shortcuts for splitting
- Day 5-7: Implement multi-pane rendering with borders

**Deliverable**: Users can split panes and see multiple terminals side-by-side.

### Week 2: Navigation & Lifecycle (Phases 2-3)
- Day 1-3: Implement pane navigation (Ctrl+Alt+arrows)
- Day 4-5: Implement pane closing (Ctrl+W)
- Day 6-7: Polish and bug fixes

**Deliverable**: Full pane management workflow.

### Week 3: Polish & Advanced Features (Phase 4)
- Day 1-3: Mouse-based pane resizing
- Day 4-5: Pane zoom/maximize toggle
- Day 6-7: Configuration options and documentation

**Deliverable**: Production-ready split pane feature.

## Testing Strategy

1. **Unit Tests**:
   - Pane tree manipulation (split, close, navigate)
   - Focus management correctness
   - Resize calculations

2. **Integration Tests**:
   - Keyboard shortcut triggering
   - Multi-pane rendering
   - Terminal I/O isolation between panes

3. **Manual Testing Scenarios**:
   - Create 4-pane layout (2x2 grid)
   - Close middle pane and verify rebalancing
   - Resize window with multiple panes
   - Navigate between panes with keyboard
   - Run different commands in each pane simultaneously

## Configuration Options

Add to `Config` structure:

```rust
pub struct PaneConfig {
    /// Enable split pane feature
    pub enabled: bool,

    /// Border width in pixels
    pub border_width: u32,

    /// Border color for unfocused panes
    pub border_color_unfocused: [u8; 3],

    /// Border color for focused pane
    pub border_color_focused: [u8; 3],

    /// Initial split ratio (0.0-1.0)
    pub default_split_ratio: f32,
}

impl Default for PaneConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            border_width: 2,
            border_color_unfocused: [60, 60, 60],   // Gray
            border_color_focused: [0, 120, 255],    // Blue
            default_split_ratio: 0.5,
        }
    }
}
```

## Potential Challenges & Solutions

### Challenge 1: Rendering Performance with Many Panes

**Risk**: Rendering 4+ panes simultaneously may impact performance.

**Mitigation**:
- Use GPU instancing to render all panes in single draw call
- Implement viewport culling for off-screen panes
- Share texture atlas across all panes
- Profile early and optimize hot paths

### Challenge 2: Focus Management Complexity

**Risk**: Complex pane trees make focus navigation unintuitive.

**Mitigation**:
- Start with simple cycling (Tab/Shift+Tab)
- Add directional navigation as enhancement
- Visual focus indicator (colored border)
- Log focus changes for debugging

### Challenge 3: Terminal Resizing with Multiple Panes

**Risk**: Complex resize calculations when window size changes.

**Mitigation**:
- The existing `PaneNode::resize()` method already handles recursive resizing
- Ensure minimum pane sizes (e.g., 20 cols x 10 rows)
- Test edge cases (very small windows, extreme aspect ratios)

### Challenge 4: Mouse Event Routing

**Risk**: Determining which pane receives mouse events.

**Mitigation**:
- Implement viewport hit-testing in mouse event handler
- Route events only to pane under cursor
- Update focus on mouse click within pane

## Success Metrics

1. **Functional**: Users can create, navigate, and close panes without crashes
2. **Performance**: 60 FPS rendering with up to 4 panes on standard hardware
3. **Usability**: Average user can learn split workflow in < 5 minutes
4. **Stability**: No regressions in single-pane mode
5. **Code Quality**: 80%+ test coverage for pane management code

## Future Enhancements (Post-MVP)

1. **Layout Persistence**: Save/restore pane layouts across sessions
2. **Named Layouts**: Predefined layouts (2-column, 3-row, etc.)
3. **Pane Tabs**: Multiple tabs per pane for advanced workflows
4. **Pane Swapping**: Drag-and-drop to reorder panes
5. **Floating Panes**: Overlay panes that can be moved/resized freely
6. **Pane Synchronization**: Send input to multiple panes simultaneously
7. **Session Management**: tmux-like session attach/detach

## Conclusion

Saternal already has excellent infrastructure for split panes through its `PaneNode` tree structure and independent `Terminal` instances per pane. The primary work involves:

1. **Refinement**: Fix focused pane splitting (currently splits root)
2. **Integration**: Connect keyboard shortcuts to split operations
3. **Rendering**: Extend renderer to handle multiple viewports with borders
4. **Polish**: Add navigation, closing, and resize capabilities

The architecture is sound and well-positioned for this feature. With focused effort over 2-3 weeks, Saternal can have production-ready split pane functionality that rivals established terminal emulators.

## References

- [WezTerm Split Pane Documentation](https://wezfurlong.org/wezterm/config/lua/keyassignment/SplitPane.html)
- [WezTerm CLI Split Pane](https://wezfurlong.org/wezterm/cli/cli/split-pane.html)
- [Zellij Layout System](https://zellij.dev/)
- [tmux Pane Management](https://github.com/tmux/tmux/wiki)
- [Terminal Emulator Comparison](https://www.tecmint.com/linux-terminal-emulators/)

---

**Document Version**: 1.0
**Date**: October 25, 2025
**Author**: Claude (Research & Analysis)
**Status**: Draft for Review
