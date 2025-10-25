# Split Pane Implementation - October 25, 2025

## Overview

Successfully implemented split pane functionality for Saternal terminal, enabling users to divide the terminal window into multiple independent shell sessions with keyboard-driven navigation and management.

## Features Implemented

### 1. Pane Splitting
- **Vertical splits** (left/right, side-by-side) via **Cmd+D**
- Splits the currently focused pane (not root)
- New pane automatically gets focus
- 50/50 space allocation between split panes
- Each pane runs an independent shell session with separate PTY
- **All panes are rendered simultaneously in their viewports**
- **Scroll enabled for focused pane** (non-focused panes show live view)

### 2. Focus Navigation
- **Mouse Click**: Click on any pane to focus it
- **Cmd+Shift+]**: Move focus to next pane (circular)
- **Cmd+Shift+[**: Move focus to previous pane (circular)
- Focused pane receives all keyboard input
- Visual distinction planned (border colors)

### 3. Pane Lifecycle
- **Ctrl+W**: Close focused pane
- Cannot close the last remaining pane (safety check)
- Tree automatically rebalances when pane is closed
- Focus moves to next available pane after close

## Keyboard Shortcuts Reference

| Shortcut | Action | Description |
|----------|--------|-------------|
| **Cmd+D** | Split Vertical | Creates left/right split (side-by-side) |
| **Mouse Click** | Focus Pane | Click on any pane to focus it |
| **Cmd+Shift+]** | Next Pane | Cycles focus forward |
| **Cmd+Shift+[** | Previous Pane | Cycles focus backward |
| **Ctrl+W** | Close Pane | Closes focused pane (if not last) |

## Architecture Changes

### Core Components Modified

#### 1. **saternal-core/src/pane.rs**
Added new methods to `PaneNode`:
- `split_focused()` - Splits the currently focused pane instead of root
- `clear_focus()` - Helper to clear focus from all panes in subtree
- `focus_next()` - Circular forward navigation through panes
- `focus_prev()` - Circular backward navigation through panes
- `close_focused()` - Closes focused pane and rebalances tree

**Key Implementation Details:**
```rust
// Recursively finds and splits the focused pane
pub fn split_focused(
    &mut self,
    direction: SplitDirection,
    new_id: usize,
    shell: Option<String>,
) -> Result<bool>

// Circular navigation through flat list of pane IDs
pub fn focus_next(&mut self) -> bool
pub fn focus_prev(&mut self) -> bool

// Removes focused pane and collapses parent split
pub fn close_focused(&mut self) -> Result<bool>
```

#### 2. **saternal/src/tab.rs**
Updated `Tab` struct:
- Modified `split()` to use `split_focused()` instead of root split
- Added `close_focused_pane()` with safety check (won't close last pane)
- Focus automatically transfers to next pane after close

#### 3. **saternal/src/app.rs**
Added keyboard shortcut handlers:
- Ctrl+D → `tab.split(SplitDirection::Horizontal, shell)`
- Ctrl+Tab/Shift+Tab → `tab.pane_tree.focus_next()` / `focus_prev()`
- Ctrl+W → `tab.close_focused_pane()`
- Updated render path to use `render_with_panes(&tab.pane_tree)`

**Lifetime Fix:**
```rust
// Holds both locks during rendering to avoid dangling reference
if let (Some(mut renderer), Some(tab_mgr)) = 
    (renderer.try_lock(), tab_manager.try_lock()) 
{
    // Render while holding locks
    renderer.render_with_panes(&tab.pane_tree)?;
}
```

#### 4. **saternal-core/src/renderer/mod.rs**
Multi-pane rendering infrastructure:
- `render_with_panes(&PaneNode)` - Main entry point for multi-pane rendering
- `copy_buffer_to_region()` - Copies individual pane buffers to combined buffer
- `update_cursor_position_with_viewport()` - Updates cursor with viewport offset
- `execute_render_pass_with_borders()` - Render pass with border support
- `render_pane_borders()` - Placeholder for border rendering (logs viewports)

**Implementation:**
1. Calculates viewports for all panes
2. Creates a combined buffer (window_width × window_height × 4 bytes)
3. For each pane:
   - Renders terminal to viewport-sized buffer
   - Copies buffer to correct region of combined buffer
4. Uploads combined buffer to GPU texture
5. Renders all panes simultaneously

#### 5. **saternal-core/src/selection/renderer.rs**
Added viewport calculation:
- `PaneViewport` struct - Stores pane position, size, and focus state
- `calculate_pane_viewports()` - Public function to compute all viewports
- `calculate_viewports_recursive()` - Recursive tree traversal for layout

**Viewport Calculation:**
```rust
pub struct PaneViewport {
    pub pane_id: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub focused: bool,
}

// Recursively calculates viewport rectangles with 2px borders
const BORDER_WIDTH: u32 = 2;
```

## How It Works

### Pane Tree Structure

The pane system uses a recursive tree structure:

```
PaneNode
├── Leaf { pane: Pane }              // Single terminal
└── Split {
      direction: Horizontal/Vertical,
      children: Vec<PaneNode>,
      ratio: f32 (0.0-1.0)            // Split ratio
    }
```

### Example Pane Layout

```
Initial:
┌─────────────────┐
│   Pane 0 (*)    │  (* = focused)
└─────────────────┘

After Ctrl+D:
┌─────────────────┐
│   Pane 0        │
├─────────────────┤
│   Pane 1 (*)    │
└─────────────────┘

After Ctrl+D again in Pane 1:
┌─────────────────┐
│   Pane 0        │
├─────────────────┤
│   Pane 1        │
├─────────────────┤
│   Pane 2 (*)    │
└─────────────────┘
```

### Focus Management Flow

1. **Split Operation:**
   - Find focused pane in tree
   - Replace it with Split node containing [old_pane, new_pane]
   - Clear focus from old pane
   - Set focus on new pane

2. **Navigation:**
   - Get flat list of all pane IDs
   - Find current focused pane index
   - Move to next/prev index (wrapping)
   - Update focus flags

3. **Close Operation:**
   - Find focused pane
   - Remove it from parent's children
   - Replace parent Split with remaining sibling
   - Focus first available pane

### Rendering Pipeline

Current implementation (MVP):
1. Calculate all pane viewports from tree
2. Render focused pane's terminal content (full screen)
3. Log viewport positions for debugging
4. **Future:** Render visible borders showing pane layout

## Technical Decisions

### Why Only Horizontal Splits?
Following **Elon's Step 1: Question Requirements** and **Step 2: Delete**:
- Start with minimum viable feature (horizontal only)
- Validate user workflow before adding complexity
- Vertical splits can be added incrementally if needed

### Why Circular Navigation?
Simple and predictable:
- No need for spatial awareness (up/down/left/right)
- Works with any pane layout
- Easy to implement and test

### Why 50/50 Split Ratio?
Follows **Step 3: Simplify**:
- Most common use case
- No UI needed for ratio adjustment
- Can add custom ratios later if requested

### Why Render Only Focused Pane?
Incremental implementation:
- MVP: Get core functionality working first
- Phase 2: Add simultaneous multi-pane rendering
- Avoids complexity of viewport clipping and multiple terminal renders

## Known Limitations & Future Work

### Current Limitations
1. **Visual Borders**: Viewports calculated but not rendered (logs only) - NEED TO IMPLEMENT
2. ~~**Single Pane Rendering**: Only focused pane visible (others exist but not shown)~~ ✅ **FIXED: All panes now render simultaneously**
3. **No Vertical Splits**: Only horizontal splits implemented
4. **Fixed 50/50 Ratio**: Cannot adjust split sizes
5. **No Mouse Support**: Keyboard-only navigation

### Planned Enhancements (Future)

#### Phase 2: Visual Polish
- [ ] Render pane borders using GPU shader
- [ ] Colored borders (blue=focused, gray=unfocused)
- [ ] Show all panes simultaneously (viewport clipping)

#### Phase 3: Advanced Features
- [ ] Vertical splits (Ctrl+Shift+D)
- [ ] Adjustable split ratios (mouse drag or keyboard)
- [ ] Directional navigation (Ctrl+Alt+hjkl)
- [ ] Mouse click to focus pane

#### Phase 4: Power User Features
- [ ] Pane swap/reorder
- [ ] Save/restore pane layouts
- [ ] Named layouts (2-column, 3-row, etc.)
- [ ] Pane synchronization (send input to multiple panes)

## Testing

### Manual Test Cases

1. **Basic Split:**
   - Start terminal
   - Press Ctrl+D
   - Verify: 2 panes exist, bottom one focused
   - Type in bottom pane, verify input goes there

2. **Multiple Splits:**
   - Press Ctrl+D three times
   - Verify: 4 panes exist
   - Each pane runs independent shell

3. **Navigation:**
   - With 3+ panes open
   - Press Ctrl+Tab repeatedly
   - Verify: Focus cycles through all panes

4. **Close Pane:**
   - With 3 panes open
   - Press Ctrl+W
   - Verify: Focused pane closes, tree rebalances
   - Focus moves to next pane

5. **Cannot Close Last:**
   - With 1 pane remaining
   - Press Ctrl+W
   - Verify: Pane remains (log message: "Cannot close last pane")

6. **Window Resize:**
   - With multiple panes open
   - Resize terminal window
   - Verify: All panes resize proportionally

## Code Quality Metrics

- **Lines Added:** ~350 lines
- **Files Modified:** 7 files
- **New Public APIs:** 5 methods
- **Build Status:** ✅ Successful (warnings only, no errors)
- **Test Coverage:** Manual testing (automated tests TODO)

## Migration Notes

### Breaking Changes
None. This is a pure feature addition.

### API Changes
New public exports in `saternal-core`:
```rust
pub use pane::SplitDirection;
pub use selection::{PaneViewport, calculate_pane_viewports};
```

### Configuration Changes
None required. Feature works out of the box.

## Troubleshooting

### Issue: Ctrl+D doesn't split
- Check if shell is capturing Ctrl+D (EOF signal)
- Solution: Shell processes may intercept Ctrl+D if no input buffer

### Issue: Pane closes when pressing Ctrl+W
- Expected behavior - Ctrl+W closes focused pane
- To prevent accidental close, consider Ctrl+Shift+W instead

### Issue: Cannot see other panes
- Current limitation - only focused pane renders
- Viewports calculated but borders not yet drawn
- Future enhancement: simultaneous multi-pane rendering

## References

- Original Proposal: `docs/SPLIT_PANE_PROPOSAL.md`
- Implementation Methodology: `.claude/commands/elon.md`
- Architecture Rules: `.claude/commands/rules.md`

## Credits

- **Implementation Date:** October 25, 2025
- **Methodology:** 5-Step Elon Process (Question, Delete, Simplify, Accelerate, Automate)
- **Architecture:** Tree-based pane layout (inspired by WezTerm, Zellij)

---

## Update: October 25, 2025 (Later) - Multi-Pane Rendering

### Changes Made

1. **Keyboard Shortcut Change**: Changed from `Ctrl+D` to `Cmd+D` for split pane operation
   - Moved `KeyD` handler from `handle_ctrl_shortcuts()` to `handle_cmd_shortcuts()` in `/saternal/src/app/input.rs`
   - More intuitive on macOS, follows platform conventions

2. **Split Direction Fixed**: Changed from top/bottom to left/right (side-by-side)
   - Changed `SplitDirection::Horizontal` to `SplitDirection::Vertical` in input.rs
   - Panes now appear side-by-side (left/right) instead of stacked (top/bottom)
   - Note: Naming is counterintuitive - "Vertical" split = vertical dividing line = horizontal panes

3. **Scroll Enabled for Focused Pane**: Each pane can scroll independently
   - Focused pane uses `self.scroll_offset` from renderer
   - Non-focused panes show live view (scroll_offset = 0)
   - Clamped to available history size

4. **Multi-Pane Rendering Implemented**: All panes now render simultaneously in their viewports
   - Added `all_panes()` and `find_pane()` methods to `PaneNode` in `saternal-core/src/pane.rs`
   - Completely rewrote `render_with_panes()` in `saternal-core/src/renderer/mod.rs`:
     - Creates combined buffer for entire window
     - Renders each pane's terminal to viewport-sized buffer
     - Composites all pane buffers into combined buffer
     - Uploads combined buffer to GPU as single texture
   - Added `copy_buffer_to_region()` helper for buffer composition
   - Added `update_cursor_position_with_viewport()` for cursor positioning in split views

### Technical Details

**Buffer Composition Algorithm:**
```
1. Create black buffer: window_width × window_height × 4 bytes (RGBA)
2. For each pane + viewport:
   a. Render pane's terminal → viewport_width × viewport_height buffer
   b. Copy buffer row-by-row to combined buffer at (viewport.x, viewport.y)
3. Upload combined buffer to GPU texture
4. Render single quad with combined texture
```

**Memory Impact:**
- Before: Single terminal buffer (full window size)
- After: Combined buffer + N viewport buffers (allocated temporarily per frame)
- Memory overhead is acceptable for typical 2-4 pane layouts

### Testing Results

- ✅ Compilation successful (release build)
- ✅ All panes render simultaneously
- ✅ Each pane shows independent shell session
- ✅ Cursor appears in focused pane
- ✅ 50/50 split displays correctly (left/right, side-by-side)
- ✅ Scroll enabled for focused pane
- ✅ Non-focused panes show live terminal output

### Bug Fix: Split Direction & Scroll (Post-Screenshot Feedback)

**Issue Reported**: After initial implementation, user screenshot showed:
1. Panes stacked top/bottom instead of side-by-side (left/right)
2. Scroll not working in panes

**Root Cause**: 
- Used `SplitDirection::Horizontal` which creates horizontal dividing line (top/bottom panes)
- Scroll offset hardcoded to 0 for all panes

**Solution Applied**:
1. Changed to `SplitDirection::Vertical` for vertical dividing line (left/right panes)
2. Enabled scroll for focused pane by using `self.scroll_offset` conditionally
3. Non-focused panes remain at scroll_offset=0 (live view)

**Result**: Panes now correctly display side-by-side with independent scroll for focused pane.

### Additional Improvements: Mouse Focus & Better Keyboard Shortcuts

**User Feedback**: 
1. Need mouse click to focus panes (more intuitive)
2. Ctrl+Tab conflicts with system shortcuts, need better keys

**Changes Made**:
1. **Mouse Click Focus** (`saternal/src/app/mouse.rs`):
   - Added viewport hit-testing in `handle_mouse_press()`
   - Calculates which pane viewport contains mouse click
   - Automatically focuses clicked pane
   - Requests redraw to show updated focus

2. **Improved Keyboard Shortcuts** (`saternal/src/app/input.rs`):
   - Removed Ctrl+Tab pane navigation (conflicts with system)
   - Added **Cmd+Shift+[** for previous pane (like browser tab switching)
   - Added **Cmd+Shift+]** for next pane
   - More intuitive and follows macOS conventions

**Implementation Details**:
- Mouse click converts cell position to pixel coordinates
- Compares against all viewport bounds
- Only changes focus if clicking different pane (prevents unnecessary redraws)
- Keyboard shortcuts now use BracketLeft/BracketRight with Cmd+Shift modifiers

---

**Status:** ✅ Fully Functional (Multi-Pane Rendering Complete)  
**Next Milestone:** Visual border rendering (Phase 2)
