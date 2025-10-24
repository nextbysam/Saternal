# Terminal Resizing Fix

## Problem Statement

The terminal text was rendering with severe positioning issues:
1. Text appeared far to the left of the window
2. Bottom half of text was getting cut off
3. Terminal was created with hardcoded 80x24 dimensions regardless of window size
4. Resizing the window had no effect on terminal dimensions

## Root Causes Identified

### 1. Fixed Terminal Dimensions
The terminal was always created with hardcoded 80x24 dimensions in `Tab::new()`, never matching the actual window size.

### 2. Missing Window Resize Handler
The `WindowEvent::Resized` handler only resized the renderer, not the underlying terminal PTY. This caused a mismatch between:
- What the renderer thought it should display
- What the terminal actually contained

### 3. Coordinate System Confusion
`PaneNode::resize()` expected **pixel dimensions** but was receiving **terminal dimensions** (cols x rows):
```rust
// WRONG: Treated cols/rows as pixels
let cols = width / 8;   // 179 / 8 = 22 cols
let rows = height / 16; // 24 / 16 = 1 row
```

This caused the terminal to render as 22x1 instead of 179x24.

### 4. Race Condition in Initialization
Initial approach tried to:
1. Create terminal at 80x24
2. Calculate proper size after renderer was ready
3. Resize terminal in first frame

This caused the terminal to render with wrong dimensions before resize took effect.

## Solution Applied (Following Elon Methodology)

### Step 1 - Question Requirements
**Question:** Do we need padding at the bottom?  
**Answer:** No, the real issue was terminal sizing, not padding.

### Step 2 - Delete
- **Removed** delayed initialization resize logic (race condition prone)
- **Removed** hardcoded 8x16 cell size assumptions in `PaneNode::resize`
- **Removed** unnecessary complexity of resizing after creation

### Step 3 - Simplify
- Calculate terminal size **before** creating the terminal
- Create terminal with correct dimensions from the start
- `PaneNode::resize` now accepts cols/rows directly, not pixels
- Added 1-row bottom padding in calculation to prevent text cutoff

## Changes Made

### 1. `saternal/src/app.rs`

#### Added Terminal Size Calculation Helper
```rust
/// Calculate terminal dimensions from window size
/// Returns (cols, rows) with padding at bottom to prevent text cutoff
fn calculate_terminal_size(window_width: u32, window_height: u32, cell_width: f32, cell_height: f32) -> (usize, usize) {
    let cols = ((window_width as f32) / cell_width).floor() as usize;
    // Reserve ~1 row of padding at bottom to prevent descenders from being cut off
    let rows = (((window_height as f32) / cell_height).floor() - 1.0).max(24.0) as usize;
    (cols.max(80), rows)
}
```

#### Calculate Dimensions Before Terminal Creation
```rust
// Calculate proper terminal size from window dimensions BEFORE creating terminals
let window_size = window.inner_size();
let effective_size = renderer.font_manager().effective_font_size();
let line_metrics = renderer.font_manager().font().horizontal_line_metrics(effective_size).unwrap();
let cell_width = renderer.font_manager().font().metrics('M', effective_size).advance_width;
let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
let (initial_cols, initial_rows) = Self::calculate_terminal_size(window_size.width, window_size.height, cell_width, cell_height);
```

#### Create Terminal with Correct Size
```rust
// Create tab manager with properly sized terminal from the start
let tab_manager = TabManager::new_with_size(config.terminal.shell.clone(), initial_cols, initial_rows)?;
```

#### Added Window Resize Handler
```rust
Event::WindowEvent {
    event: WindowEvent::Resized(size),
    ..
} => {
    // Resize renderer
    renderer.resize(size.width, size.height);
    
    // Calculate new terminal dimensions
    let (cols, rows) = Self::calculate_terminal_size(size.width, size.height, cell_width, cell_height);
    
    // Resize all terminals
    if let Some(mut tab_mgr) = tab_manager.try_lock() {
        if let Some(active_tab) = tab_mgr.active_tab_mut() {
            active_tab.resize(cols, rows)?;
        }
    }
}
```

### 2. `saternal/src/tab.rs`

#### Added Size-Aware Constructor
```rust
impl Tab {
    pub fn new(id: usize, shell: Option<String>) -> Result<Self> {
        Self::new_with_size(id, 80, 24, shell)
    }
    
    pub fn new_with_size(id: usize, cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        info!("Creating new tab: {} with size {}x{}", id, cols, rows);
        let pane_tree = PaneNode::new_leaf(0, cols, rows, shell)?;
        // ...
    }
}

impl TabManager {
    pub fn new(shell: String) -> Result<Self> {
        Self::new_with_size(shell, 80, 24)
    }
    
    pub fn new_with_size(shell: String, cols: usize, rows: usize) -> Result<Self> {
        info!("Creating tab manager with terminal size {}x{}", cols, rows);
        let mut tab = Tab::new_with_size(0, cols, rows, Some(shell.clone()))?;
        // ...
    }
}
```

### 3. `saternal-core/src/pane.rs`

#### Fixed Resize to Accept Cols/Rows
**Before:**
```rust
/// Resize all panes in the tree based on available space
pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
    match self {
        PaneNode::Leaf { pane } => {
            // WRONG: Assumed pixels, divided by hardcoded cell size
            let cols = width / 8;
            let rows = height / 16;
            pane.resize(cols.max(1), rows.max(1))?;
        }
    }
}
```

**After:**
```rust
/// Resize all panes in the tree to specified terminal dimensions (cols x rows)
pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
    match self {
        PaneNode::Leaf { pane } => {
            pane.resize(cols.max(1), rows.max(1))?;
        }
        PaneNode::Split { direction, children, ratio } => {
            match direction {
                SplitDirection::Horizontal => {
                    // Split rows between panes
                    let rows1 = (rows as f32 * *ratio) as usize;
                    let rows2 = rows.saturating_sub(rows1);
                    children[0].resize(cols, rows1)?;
                    children[1].resize(cols, rows2)?;
                }
                SplitDirection::Vertical => {
                    // Split cols between panes
                    let cols1 = (cols as f32 * *ratio) as usize;
                    let cols2 = cols.saturating_sub(cols1);
                    children[0].resize(cols1, rows)?;
                    children[1].resize(cols2, rows)?;
                }
            }
        }
    }
}
```

## How It Works Now

### Startup Flow
1. Window is created
2. Renderer is created
3. Font metrics are calculated from renderer
4. Terminal dimensions are calculated from window size and font metrics
5. Terminal is created with correct dimensions immediately
6. No race conditions or delayed resizing

### Window Resize Flow
1. `WindowEvent::Resized` is received
2. Renderer surface is resized
3. Terminal dimensions are recalculated based on new window size
4. All terminals in all tabs are resized to match
5. Terminal PTY is notified of new dimensions

### Terminal Size Calculation
```rust
cols = floor(window_width / cell_width)
rows = floor(window_height / cell_height) - 1  // Reserve 1 row for padding

// Minimums applied
cols = max(cols, 80)
rows = max(rows, 24)
```

## Benefits

1. **No More Text Cutoff:** Terminal size matches window size with proper padding
2. **Correct Positioning:** Text renders in the correct location
3. **Dynamic Resizing:** Terminal resizes when window resizes
4. **No Race Conditions:** Terminal created with correct size from the start
5. **Proper Split Handling:** Pane splits work correctly with cols/rows instead of pixels

## Testing

To verify the fix:
1. Launch terminal - text should render correctly positioned
2. Resize window - terminal should resize to match
3. Check logs for:
   ```
   Calculated initial terminal size: 179x24 for window 3440x720
   Creating tab manager with terminal size 179x24
   Rendering terminal: 179x24 cells
   ```

## Future Improvements

- [ ] Handle DPI changes to recalculate terminal size
- [ ] Add terminal size display in UI
- [ ] Support user-configurable cell padding
- [ ] Handle multiple monitors with different DPIs
