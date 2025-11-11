# Cursor Visibility Fix Proposal

## Problem Statement

When launching CLI tools (e.g., `claude`, `vim`, `htop`) within the Saternal terminal emulator, the cursor becomes invisible and stops blinking. This creates a poor user experience as users cannot see where they are typing.

**Observed Behavior:**
- Cursor is visible and blinking in the shell prompt
- Cursor disappears when CLI applications start
- Cursor does not blink even when visible in some applications

**Screenshot Reference:** See image showing Claude CLI with missing cursor at the command input line.

## Root Cause Analysis

### Technical Background

Terminal applications control cursor visibility through **DECTCEM** (DEC Text Cursor Enable Mode), using ANSI escape sequences:
- `CSI ? 25 h` (or `\x1b[?25h`) - Show cursor
- `CSI ? 25 l` (or `\x1b[?25l`) - Hide cursor

The Alacritty terminal library (which Saternal uses) tracks this state in `TermMode::SHOW_CURSOR`.

### Current Implementation

**Location:** `saternal-core/src/renderer/mod.rs:474-509`

```rust
fn update_cursor_position<T>(&mut self, term: &Term<T>) {
    let cursor_pos = term.grid().cursor.point;

    // Cursor visibility is managed by the terminal's DECTCEM mode (CSI ? 25 h/l)
    // SHOW_CURSOR flag present = visible, absent = hidden
    // Also hide cursor when scrolled in history
    let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR)
                      || self.scroll_offset > 0.01;

    log::debug!("Cursor: pos=({}, {}), SHOW_CURSOR={}, hide={}",
               cursor_pos.column.0, cursor_pos.line.0,
               term.mode().contains(TermMode::SHOW_CURSOR), hide_cursor);

    // ... cursor rendering logic ...
}
```

**Cursor State Management:** `saternal-core/src/renderer/cursor/state.rs:128-136`

```rust
pub fn update_position(
    &mut self,
    cursor_pos: Point,
    cell_width: f32,
    cell_height: f32,
    window_width: u32,
    window_height: u32,
    scroll_offset: usize,
    hide_cursor: bool,
) {
    // Hide cursor if scrolled or terminal mode requests it
    // Unless force_show is enabled (overrides application hide requests)
    let should_hide = scroll_offset > 0 || (hide_cursor && !self.config.force_show);
    // ...
}
```

### Identified Issues

1. **VTE Parser State:** The Alacritty VTE (Virtual Terminal Emulator) parser in `saternal-core/src/terminal.rs:117-142` correctly processes escape sequences and updates `TermMode::SHOW_CURSOR`, but there may be timing issues or initialization problems.

2. **Missing Initial State:** When a PTY is created, the cursor visibility mode may not be explicitly set, leading to an undefined state.

3. **Application Behavior:** Some CLI applications (like Claude Code, vim) hide the cursor during initialization and expect the terminal to respect the DECTCEM sequences, but the cursor state may not be updating properly.

4. **Rendering Pipeline:** The cursor state is checked once per render frame. If the terminal mode changes between frames, the cursor visibility may lag.

## Research Findings

From extensive research into terminal emulator implementations and ANSI standards:

### Standard Behavior (from xterm, alacritty, ghostty)

1. **Default State:** Cursor should be visible on PTY creation
2. **DECTCEM Compliance:** Terminal MUST respect `CSI ? 25 h/l` sequences
3. **Application Control:** Applications can hide cursor during:
   - Full-screen mode (TUI apps)
   - Input processing
   - Background operations
4. **Restoration:** Applications SHOULD restore cursor on exit

### Common Pitfalls in Terminal Emulators

1. **Missing Alternate Screen State:** When switching to alternate screen buffer, cursor state should be preserved
2. **Signal Handling:** SIGWINCH (window resize) can cause cursor state to reset
3. **PTY Initialization:** Some shells/apps send cursor hide sequences early in startup
4. **Race Conditions:** Escape sequences arriving faster than rendering updates

## Proposed Solutions

### Solution 1: Force Initial Cursor Visibility (Simple Fix)

**Approach:** Set cursor to visible state when PTY is created and ensure it's respected in rendering.

**Implementation:**

**File:** `saternal-core/src/terminal.rs:54-70`

```rust
let pty = tty::new(&pty_config, window_size, 0)?;

// Create terminal with TermSize
let event_listener = TermEventListener::new();
let size = TermSize::new(cols, rows);
let mut term = Term::new(TermConfig::default(), &size, event_listener);

// ADDITION: Ensure cursor is visible by default
// This sets the DECTCEM mode to enabled (cursor visible)
term.mode_mut().insert(TermMode::SHOW_CURSOR);

let term = Arc::new(Mutex::new(term));
```

**Pros:**
- Simple, 1-line fix
- Ensures deterministic initial state
- Aligns with terminal standard behavior

**Cons:**
- May not address applications that explicitly hide cursor
- Doesn't fix blinking issues if those exist

### Solution 2: Enhanced Cursor State Tracking (Comprehensive Fix)

**Approach:** Add explicit cursor state tracking and logging to diagnose the full issue.

**Implementation:**

**File:** `saternal-core/src/terminal.rs` - Add tracking method:

```rust
impl Terminal {
    /// Get current cursor visibility state for debugging
    pub fn cursor_visible(&self) -> bool {
        let term = self.term.lock();
        term.mode().contains(TermMode::SHOW_CURSOR)
    }

    /// Force cursor visibility (for debugging/recovery)
    pub fn force_show_cursor(&mut self) {
        let mut term = self.term.lock();
        term.mode_mut().insert(TermMode::SHOW_CURSOR);
        log::info!("Forced cursor visibility");
    }
}
```

**File:** `saternal-core/src/renderer/mod.rs:474` - Enhanced logging:

```rust
fn update_cursor_position<T>(&mut self, term: &Term<T>) {
    let cursor_pos = term.grid().cursor.point;
    let term_mode = term.mode();
    let show_cursor_flag = term_mode.contains(TermMode::SHOW_CURSOR);

    // Log full terminal mode for debugging
    if !show_cursor_flag {
        log::warn!("Cursor hidden by terminal mode - SHOW_CURSOR flag not set");
        log::debug!("Terminal mode flags: {:?}", term_mode);
    }

    let hide_cursor = !show_cursor_flag || self.scroll_offset > 0.01;

    // ... rest of implementation
}
```

**Pros:**
- Provides diagnostic information
- Allows runtime debugging
- Can recover from bad state

**Cons:**
- More complex
- Requires testing with multiple applications

### Solution 3: Application-Specific Cursor Handling (Robust Fix)

**Approach:** Track cursor state changes and ensure proper restoration, especially around application launches.

**Implementation:**

**File:** `saternal-core/src/renderer/cursor/state.rs` - Add state tracking:

```rust
pub struct CursorState {
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    blink_state: BlinkState,
    pub config: CursorConfig,
    current_uniforms: CursorUniforms,

    // ADDITION: Track cursor state history
    last_visibility_state: bool,
    visibility_changed_at: std::time::Instant,
}

impl CursorState {
    pub fn update_position(
        &mut self,
        cursor_pos: Point,
        cell_width: f32,
        cell_height: f32,
        window_width: u32,
        window_height: u32,
        scroll_offset: usize,
        hide_cursor: bool,
    ) {
        let should_hide = scroll_offset > 0 || (hide_cursor && !self.config.force_show);

        // ADDITION: Log state changes
        if should_hide != self.last_visibility_state {
            log::info!("Cursor visibility changed: {} -> {} (hide_cursor={}, scroll={}, force_show={})",
                      !self.last_visibility_state, !should_hide,
                      hide_cursor, scroll_offset, self.config.force_show);
            self.last_visibility_state = should_hide;
            self.visibility_changed_at = std::time::Instant::now();
        }

        // ... rest of implementation
    }
}
```

**Pros:**
- Detailed visibility tracking
- Helps identify when/why cursor disappears
- Can detect and log anomalies

**Cons:**
- Most complex solution
- Higher memory/CPU overhead

## Recommended Solution

**Hybrid Approach: Solution 1 + Enhanced Logging**

1. **Implement Solution 1** to fix the immediate problem:
   - Add `term.mode_mut().insert(TermMode::SHOW_CURSOR);` after terminal creation

2. **Add minimal logging** (from Solution 2):
   - Log when SHOW_CURSOR flag transitions from true → false
   - Help diagnose applications that misbehave

**Implementation Steps:**

### Step 1: Fix Initial State
**File:** `saternal-core/src/terminal.rs:59`

```rust
let mut term = Term::new(TermConfig::default(), &size, event_listener);

// Ensure cursor starts visible (DECTCEM default behavior)
term.mode_mut().insert(TermMode::SHOW_CURSOR);

let term = Arc::new(Mutex::new(term));
```

### Step 2: Add Debug Logging
**File:** `saternal-core/src/terminal.rs:129-131`

```rust
pub fn process_output(&mut self) -> Result<usize> {
    use std::io::Read;

    let mut buf = [0u8; 4096];
    let mut total_bytes = 0;
    loop {
        match self.pty.reader().read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_bytes += n;
                debug!("Read {} bytes from PTY: {:?}", n, String::from_utf8_lossy(&buf[..n]));

                // ADDITION: Track cursor visibility before/after processing
                let was_visible = {
                    let term = self.term.lock();
                    term.mode().contains(TermMode::SHOW_CURSOR)
                };

                let mut term = self.term.lock();
                self.processor.advance(&mut *term, &buf[..n]);

                let is_visible = term.mode().contains(TermMode::SHOW_CURSOR);
                if was_visible != is_visible {
                    log::info!("Cursor visibility changed: {} -> {} (bytes processed: {})",
                              was_visible, is_visible, n);
                    log::debug!("Escape sequences: {:?}", String::from_utf8_lossy(&buf[..n]));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => {
                debug!("PTY read error: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(total_bytes)
}
```

### Step 3: Verify Cursor Blinking
**File:** `saternal-core/src/renderer/cursor/state.rs:111-117`

Ensure blinking is active:

```rust
pub fn update_blink(&mut self) -> bool {
    if self.config.blink {
        let changed = self.blink_state.update();
        if changed {
            log::trace!("Cursor blink toggled: visible={}", self.blink_state.visible);
        }
        changed
    } else {
        false
    }
}
```

### Step 4: Test Cases

Create test scenarios:
1. **Shell prompt** - cursor should be visible and blinking
2. **Launch vim** - cursor should remain visible, may change shape
3. **Launch htop** - cursor may be hidden (expected)
4. **Launch Claude CLI** - cursor should be visible at input
5. **Exit application** - cursor should restore to shell state

## Alternative: Configuration Override

If applications misbehave persistently, add a config option:

**File:** `saternal-core/src/renderer/cursor/config.rs:5-10`

```rust
pub struct CursorConfig {
    pub color: [f32; 4],
    pub style: CursorStyle,
    pub blink: bool,
    pub blink_interval_ms: u64,

    // ADDITION: Force cursor to always be visible
    pub force_show: bool,  // Already exists! Just needs documentation
}
```

**File:** `saternal-core/src/config.rs` - Add to user config:

```rust
[cursor]
force_show = false  # Set to true to ignore application hide requests
```

## Testing Plan

1. **Unit Tests:** Verify terminal mode is set on initialization
2. **Integration Tests:** Launch various CLI apps and verify cursor behavior
3. **Visual Tests:** Manual verification of cursor blinking and visibility
4. **Performance Tests:** Ensure cursor state tracking doesn't impact render performance

## Success Criteria

- ✅ Cursor is visible and blinking in shell prompt
- ✅ Cursor remains visible when launching CLI tools (claude, vim, etc.)
- ✅ Cursor blinks consistently at configured interval
- ✅ Cursor properly hides when scrolling through history
- ✅ Cursor respects application hide/show requests (DECTCEM)
- ✅ No performance regression in rendering pipeline

## References

- **XTerm Control Sequences:** https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
- **ANSI Escape Codes:** https://en.wikipedia.org/wiki/ANSI_escape_code#CSI_(Control_Sequence_Introducer)_sequences
- **Alacritty Terminal Docs:** https://docs.rs/alacritty_terminal/latest/alacritty_terminal/
- **DECTCEM Specification:** DEC VT510 Manual, Section 5.152

## Implementation Timeline

1. **Phase 1 (Immediate):** Implement Solution 1 - Initial state fix (1 hour)
2. **Phase 2 (Short-term):** Add debug logging (2 hours)
3. **Phase 3 (Testing):** Test with various CLI applications (2 hours)
4. **Phase 4 (Optional):** Enhanced state tracking if issues persist (4 hours)

**Total Estimated Time:** 5-9 hours

## Conclusion

The cursor visibility issue in Saternal is likely caused by missing initial state configuration and/or improper handling of DECTCEM escape sequences during application startup. The recommended hybrid solution combines a simple fix with diagnostic logging to ensure both immediate resolution and long-term maintainability.

The fix is minimal, low-risk, and aligns with standard terminal emulator behavior. If issues persist after the initial fix, the enhanced logging will provide clear diagnostic information to guide further refinement.
