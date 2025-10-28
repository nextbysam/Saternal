# Saternal UI Flows Documentation

## Overview

This document describes all user interface flows in Saternal, including event handling, input processing, rendering pipelines, and interactive features. It serves as a comprehensive reference for understanding how user interactions are processed from input to visual output.

---

## Table of Contents

1. [Application Architecture](#application-architecture)
2. [Event Loop Flow](#event-loop-flow)
3. [Input Flows](#input-flows)
4. [Pane Management](#pane-management)
5. [Tab Management](#tab-management)
6. [Selection Flow](#selection-flow)
7. [Search Flow](#search-flow)
8. [Natural Language Commands](#natural-language-commands)
9. [Rendering Pipeline](#rendering-pipeline)
10. [Scrolling and Navigation](#scrolling-and-navigation)
11. [Clipboard Operations](#clipboard-operations)
12. [Window Management](#window-management)

---

## Application Architecture

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                         Main Thread                          │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐       ┌──────────────┐                   │
│  │ Winit Window │◄──────┤  Event Loop  │                   │
│  └──────┬───────┘       └──────┬───────┘                   │
│         │                      │                            │
│         │                      ▼                            │
│         │              ┌──────────────┐                     │
│         │              │Event Handlers│                     │
│         │              └──────┬───────┘                     │
│         │                     │                             │
│         │      ┌──────────────┼──────────────┐             │
│         │      ▼              ▼              ▼             │
│         │  ┌────────┐   ┌──────────┐  ┌──────────┐        │
│         │  │ Input  │   │  Mouse   │  │  Window  │        │
│         │  │Handler │   │ Handler  │  │ Handler  │        │
│         │  └───┬────┘   └─────┬────┘  └─────┬────┘        │
│         │      │              │             │              │
│         │      └──────┬───────┴─────────────┘              │
│         │             ▼                                     │
│         │      ┌──────────────┐                            │
│         │      │ Tab Manager  │                            │
│         │      └──────┬───────┘                            │
│         │             │                                     │
│         │             ▼                                     │
│         │      ┌──────────────┐                            │
│         │      │  Pane Tree   │                            │
│         │      └──────┬───────┘                            │
│         │             │                                     │
│         │             ▼                                     │
│         │      ┌──────────────┐                            │
│         │      │  PTY/Shell   │                            │
│         │      └──────────────┘                            │
│         │                                                   │
│         ▼                                                   │
│  ┌──────────────┐                                          │
│  │ GPU Renderer │                                          │
│  │   (WGPU)     │                                          │
│  └──────────────┘                                          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      Tokio Runtime                           │
│                    (Async Background)                        │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐       ┌──────────────┐                   │
│  │  LLM Client  │──────►│   Channel    │──►Main Thread     │
│  │  (Async)     │       │  (mpsc)      │                   │
│  └──────────────┘       └──────────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

### State Management

**Global State (Thread-Safe via Arc<Mutex>):**
- `Renderer` - GPU rendering state, font manager, scroll offset
- `TabManager` - All tabs, active tab index
- `DropdownWindow` - macOS dropdown overlay
- `LLMClient` - Natural language processing client

**Per-Tab State:**
- `PaneNode` - Tree structure of terminal panes
- `UIMessage` - Active UI overlay (generating/suggestion/error)
- `pending_nl_commands` - Commands awaiting confirmation
- `nl_confirmation_mode` - Whether awaiting user y/n input
- `confirmation_input` - Buffer for confirmation input

**Per-Pane State:**
- `Terminal` - PTY connection and terminal emulator
- `focused` - Whether this pane has keyboard focus

---

## Event Loop Flow

### Main Event Loop (Synchronous)

```rust
// saternal/src/app/event_loop.rs

event_loop.run(move |event, elwt| {
    match event {
        Event::WindowEvent { event, .. } => { /* Handle window events */ }
        Event::AboutToWait => { 
            // Check async messages
            // Process PTY output
            // Request redraws if needed
        }
        _ => {}
    }
})
```

### Event Processing Flow

```
User Action
    │
    ▼
┌─────────────────┐
│ Winit Event     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Event Handler   │ (input.rs, mouse.rs, window.rs)
└────────┬────────┘
         │
         ├──► Modify State (TabManager, Renderer, etc.)
         │
         ├──► Send to PTY (if terminal input)
         │
         ├──► Spawn Async Task (if NL command)
         │
         └──► Request Redraw
                   │
                   ▼
           ┌─────────────────┐
           │ AboutToWait     │
           ├─────────────────┤
           │ • Check async   │
           │ • Process PTY   │
           │ • Request redraw│
           └────────┬────────┘
                    │
                    ▼
           ┌─────────────────┐
           │ RedrawRequested │
           └────────┬────────┘
                    │
                    ▼
           ┌─────────────────┐
           │ Render Frame    │
           └─────────────────┘
```

### AboutToWait (Idle Processing)

```rust
Event::AboutToWait => {
    // 1. Check for async messages (NL commands)
    if let Ok(msg) = nl_rx.try_recv() {
        handle_nl_message(msg, &tab_manager);
        window.request_redraw();
    }
    
    // 2. Process PTY output from all panes
    if let Some(active_tab) = tab_mgr.active_tab_mut() {
        if active_tab.process_output()? > 0 {
            window.request_redraw();
        }
    }
}
```

**Critical Design Points:**
- Non-blocking: Uses `try_lock()` and `try_recv()` to avoid deadlocks
- Continuous polling: Runs on every idle event to process PTY output
- Selective redraws: Only requests redraw if data changed

---

## Input Flows

### Keyboard Input Flow

```
Keyboard Press
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ WindowEvent::KeyboardInput                              │
│ • KeyEvent (physical & logical key)                     │
│ • ElementState (pressed/released)                       │
│ • ModifiersState (Cmd, Shift, Ctrl, Alt)              │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│ handle_keyboard_input()                                 │
│ saternal/src/app/input.rs                              │
└────────────────────┬────────────────────────────────────┘
                     │
         ┌───────────┴───────────┐
         │                       │
         ▼                       ▼
┌──────────────────┐    ┌──────────────────┐
│ Cmd Shortcuts    │    │ Terminal Input   │
│ • Cmd+C (copy)   │    │ • Regular keys   │
│ • Cmd+V (paste)  │    │ • Control chars  │
│ • Cmd+F (search) │    │ • Escape seqs    │
│ • Cmd+D (split)  │    │ • Text input     │
└──────────────────┘    └─────────┬────────┘
                                  │
                                  ▼
                        ┌──────────────────┐
                        │ Enter Key?       │
                        └────┬────────┬────┘
                             │        │
                          NO │        │ YES
                             │        │
                             │        ▼
                             │  ┌──────────────────┐
                             │  │ In Confirmation  │
                             │  │     Mode?        │
                             │  └────┬────────┬────┘
                             │       │        │
                             │    YES│        │NO
                             │       │        │
                             │       ▼        ▼
                             │  ┌─────────┐  ┌──────────────┐
                             │  │ Handle  │  │ Read current │
                             │  │ y/n/yes │  │ line from    │
                             │  │ /no     │  │ grid         │
                             │  └─────────┘  └──────┬───────┘
                             │                      │
                             │                      ▼
                             │              ┌──────────────┐
                             │              │ Strip shell  │
                             │              │ prompt       │
                             │              └──────┬───────┘
                             │                     │
                             │                     ▼
                             │              ┌──────────────┐
                             │              │ Detect NL?   │
                             │              └────┬────┬────┘
                             │                   │    │
                             │                YES│    │NO
                             │                   │    │
                             │                   ▼    ▼
                             │              ┌────────────┐
                             │              │ Spawn LLM  │
                             │              │ async task │
                             │              └────────────┘
                             │
                             ▼
                    ┌──────────────────┐
                    │ key_to_bytes()   │
                    │ Convert to ANSI  │
                    └─────────┬────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │ tab.write_input()│
                    │ Send to PTY      │
                    └──────────────────┘
```

### Key Categories

**1. Command Shortcuts (Cmd+Key)**
- `Cmd+C` - Copy selection to clipboard
- `Cmd+V` - Paste from clipboard
- `Cmd+F` - Activate search mode
- `Cmd+G` / `Cmd+Shift+G` - Next/previous search result
- `Cmd+D` - Split pane vertically (side-by-side)
- `Cmd+Shift+D` - Split pane horizontally (top/bottom)
- `Cmd+W` - Close focused pane
- `Cmd+Shift+[` - Focus previous pane
- `Cmd+Shift+]` - Focus next pane
- `Cmd++` - Increase font size
- `Cmd+-` - Decrease font size
- `Cmd+0` - Reset font size

**2. Control Characters (Ctrl+Key)**
- `Ctrl+C` - SIGINT (0x03)
- `Ctrl+D` - EOF (0x04)
- `Ctrl+Z` - SIGTSTP (0x1A)
- `Ctrl+A` - Beginning of line (0x01)
- `Ctrl+E` - End of line (0x05)
- `Ctrl+K` - Kill to end of line (0x0B)
- `Ctrl+L` - Clear screen (0x0C)
- `Ctrl+R` - Reverse search (0x12)
- `Ctrl+U` - Kill line backward (0x15)
- `Ctrl+W` - Kill word backward (0x17)

**3. Special Keys**
- `Escape` - ESC (0x1B) or cancel search/selection
- `Enter` - CR (0x0D) or handle NL detection
- `Backspace` - DEL (0x7F)
- `Tab` - HT (0x09)
- `Shift+Tab` - Backtab (ESC[Z)
- Arrow keys - ESC[A/B/C/D (with modifier support)
- Function keys - ESC[OP through ESC[24~

**4. Jump to Bottom**
- `Shift+G` - Jump to bottom (vim-style)
- `Shift+End` - Jump to bottom (traditional)

### Text Input Flow

```
Character Input
    │
    ▼
┌─────────────────┐
│ Key::Character  │
└────────┬────────┘
         │
         ├──► If in confirmation mode:
         │    • Append to confirmation_input buffer
         │    • Echo to terminal
         │
         └──► Normal mode:
              • Send to PTY stdin
              • Shell echoes back
              • Appears in terminal grid
```

### Natural Language Detection (Enter Key)

```rust
// When Enter is pressed:

1. Check confirmation mode
   ├─ YES → Handle y/n/yes/no from buffer
   └─ NO  → Continue to detection

2. Read current line from grid
   • Use cursor position to find line
   • Read visible characters from terminal

3. Strip shell prompt
   • Remove "user@host path % "
   • Remove "user@host:path$ "
   • Keep only actual command text

4. Detect natural language
   • Check for question words (show, list, find, tell)
   • Check for articles (the, a, an, my, all)
   • Check word count (> 5 words)
   • Not starting with shell commands

5. If detected:
   • Send newline to terminal (move to next line)
   • Display "Generating..." UI overlay
   • Spawn async LLM task
   • Store in pending commands on response
```

---

## Mouse Input Flow

### Mouse Click Flow

```
Mouse Click
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ WindowEvent::MouseInput                                 │
│ • ElementState (pressed/released)                       │
│ • MouseButton (left/right/middle)                       │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│ handle_mouse_input()                                    │
│ saternal/src/app/mouse.rs                              │
└────────────────────┬────────────────────────────────────┘
                     │
         ┌───────────┴───────────┐
         │                       │
         ▼                       ▼
┌──────────────────┐    ┌──────────────────┐
│ Button Pressed   │    │ Button Released  │
└────────┬─────────┘    └────────┬─────────┘
         │                       │
         ▼                       │
┌──────────────────┐             │
│ Left Click?      │             │
└────┬────────┬────┘             │
     │        │                  │
  YES│        │NO                │
     │        │                  │
     ▼        └──────────────────┘
┌──────────────────┐             │
│ Check Pane Focus │             │
│ • Calc viewports │             │
│ • Find clicked   │             │
│ • Set focus      │             │
└────────┬─────────┘             │
         │                       │
         ▼                       │
┌──────────────────┐             │
│ Click Count?     │             │
└────┬────┬────┬───┘             │
     │    │    │                 │
   1 │  2 │  3 │                 │
     │    │    │                 │
     ▼    ▼    ▼                 ▼
┌─────┐ ┌──┐ ┌──┐     ┌──────────────────┐
│Start│ │Dbl│ │Tri│    │ End Selection    │
│Sel. │ │Clk│ │Clk│    │ • Finalize range │
└─────┘ └──┘ └──┘     │ • Update renderer│
         │    │        └──────────────────┘
         │    │
         ▼    ▼
    ┌──────────────┐
    │ Expand Word/ │
    │ Line         │
    └──────────────┘
```

### Mouse Drag Flow (Selection)

```
Drag Started (Button Pressed)
    │
    ▼
┌─────────────────┐
│ Start Selection │
│ • Store start   │
│ • Set mode      │
└────────┬────────┘
         │
    (Mouse Moves)
         │
         ▼
┌─────────────────┐
│ CursorMoved     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Update position │
│ in MouseState   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Dragging?       │
└────┬───────┬────┘
     │       │
  YES│       │NO
     │       │
     ▼       └─── (ignore)
┌─────────────────┐
│ Update Selection│
│ • Extend range  │
│ • Update GPU    │
└─────────────────┘
```

### Mouse Wheel Flow (Scrolling)

```
Mouse Wheel Event
    │
    ▼
┌─────────────────────────────────────┐
│ WindowEvent::MouseWheel             │
│ • MouseScrollDelta                  │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ handle_mouse_wheel()                │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ Convert delta to scroll lines       │
│ • LineDelta: y * 3.0                │
│ • PixelDelta: y / 20.0              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ renderer.scroll(delta)              │
│ • Update scroll_offset              │
│ • Clamp to history bounds           │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ Update window title                 │
│ "Saternal [↑ 42%]"                  │
└────────────────┬────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

---

## Pane Management

### Pane Tree Structure

```rust
pub enum PaneNode {
    Leaf { pane: Pane },
    Split {
        direction: SplitDirection,  // Horizontal or Vertical
        children: Vec<PaneNode>,
        ratio: f32,  // Split position (0.0-1.0)
    },
}
```

### Split Pane Flow

```
User: Cmd+D (vertical) or Cmd+Shift+D (horizontal)
    │
    ▼
┌─────────────────────────────────────┐
│ handle_cmd_shortcuts()              │
│ KeyCode::KeyD detected              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ tab.split(direction, shell)         │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ pane_tree.split_focused()           │
│ • Find focused leaf                 │
│ • Create new pane with PTY          │
│ • Replace leaf with Split node      │
│ • Set children [old, new]           │
│ • Set ratio 0.5 (50/50 split)       │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ calculate_pane_viewports()          │
│ • Traverse tree recursively         │
│ • Calculate pixel bounds per pane   │
│ • Account for border width (2px)    │
└────────────────┬────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Example Split Tree Evolution

```
Initial State:
┌─────────────────┐
│   Pane 0        │
│   (Focused)     │
└─────────────────┘

After Vertical Split (Cmd+D):
┌────────┬────────┐
│ Pane 0 │ Pane 1 │
│        │(Focused)│
└────────┴────────┘

Tree:
Split(Vertical, ratio=0.5)
├─ Leaf(Pane 0)
└─ Leaf(Pane 1)

After Horizontal Split on Pane 1 (Cmd+Shift+D):
┌────────┬────────┐
│ Pane 0 │ Pane 1 │
│        ├────────┤
│        │ Pane 2 │
│        │(Focused)│
└────────┴────────┘

Tree:
Split(Vertical, ratio=0.5)
├─ Leaf(Pane 0)
└─ Split(Horizontal, ratio=0.5)
    ├─ Leaf(Pane 1)
    └─ Leaf(Pane 2)
```

### Pane Navigation Flow

```
Cmd+Shift+[ or Cmd+Shift+]
    │
    ▼
┌─────────────────────────────────────┐
│ handle_pane_navigation()            │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ pane_tree.focus_prev/next()         │
│ • Get all pane IDs                  │
│ • Find current focused index        │
│ • Move to prev/next (wrap around)   │
│ • Set focus on new pane             │
└────────────────┬────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Close Pane Flow

```
Cmd+W
    │
    ▼
┌─────────────────────────────────────┐
│ tab.close_focused_pane()            │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ Check if last pane?                 │
└────┬───────────────────┬────────────┘
     │                   │
  YES│                   │NO
     │                   │
     ▼                   ▼
┌─────────┐    ┌──────────────────────┐
│ Refuse  │    │ pane_tree.close()    │
│ (warn)  │    │ • Remove pane        │
└─────────┘    │ • Drop PTY           │
               │ • Simplify tree      │
               │ • Focus next pane    │
               └──────────┬───────────┘
                          │
                          ▼
                    window.request_redraw()
```

---

## Tab Management

### Tab State

```rust
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_tree: PaneNode,
    pub pending_nl_commands: Option<Vec<String>>,
    pub nl_confirmation_mode: bool,
    pub confirmation_input: String,
    pub ui_message: Option<UIMessage>,
}
```

### Create New Tab Flow

```
User Action (Hotkey or Menu)
    │
    ▼
┌─────────────────────────────────────┐
│ tab_manager.new_tab()               │
│ • Generate new tab ID               │
│ • Create Tab with single pane       │
│ • Spawn shell PTY                   │
│ • Set as active tab                 │
└────────────────┬────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Switch Tab Flow

```
Tab Switch Command
    │
    ▼
┌─────────────────────────────────────┐
│ tab_manager.switch_to_tab(index)    │
│ • Validate index                    │
│ • Update active_tab                 │
└────────────────┬────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Close Tab Flow

```
User Action
    │
    ▼
┌─────────────────────────────────────┐
│ tab_manager.close_tab(id)           │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│ Check if last tab?                  │
└────┬───────────────────┬────────────┘
     │                   │
  YES│                   │NO
     │                   │
     ▼                   ▼
┌─────────┐    ┌──────────────────────┐
│ Refuse  │    │ Remove tab           │
│ (keep 1)│    │ • Drop all panes     │
└─────────┘    │ • Close all PTYs     │
               │ • Adjust active idx  │
               └──────────┬───────────┘
                          │
                          ▼
                    window.request_redraw()
```

---

## Selection Flow

### Selection States

```rust
pub enum SelectionMode {
    Normal,  // Character-by-character
    Word,    // Word boundaries
    Line,    // Full lines
}

pub struct SelectionManager {
    range: Option<SelectionRange>,
    mode: SelectionMode,
}
```

### Selection Lifecycle

```
┌─────────────────────────────────────────────────────────┐
│ 1. START SELECTION                                      │
│    • Single click (Normal mode)                         │
│    • Double click (Word mode)                           │
│    • Triple click (Line mode)                           │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 2. EXTEND SELECTION (Optional)                          │
│    • Mouse drag updates end point                       │
│    • Mode determines expansion logic                    │
│    • GPU renderer updates highlight spans               │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 3. FINALIZE SELECTION                                   │
│    • Mouse release                                      │
│    • Range stored in SelectionManager                   │
│    • Highlight remains visible                          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 4. COPY OR CLEAR                                        │
│    • Cmd+C copies to clipboard                          │
│    • Escape clears selection                            │
│    • New click starts new selection                     │
└─────────────────────────────────────────────────────────┘
```

### Selection Expansion Logic

**Normal Mode:**
- Direct character selection from start to end point
- Follows mouse cursor exactly

**Word Mode:**
- Expands to word boundaries using regex: `\b\w+\b`
- Includes alphanumeric and underscore

**Line Mode:**
- Selects entire lines from start line to end line
- Includes full line width (all columns)

### Selection to Spans (GPU Rendering)

```rust
// Selection range in grid coordinates
SelectionRange {
    start: Point(line: 5, column: 10),
    end: Point(line: 7, column: 25),
}

// Converted to NDC spans for GPU
Span 1: Line 5, cols 10-79 (rest of first line)
Span 2: Line 6, cols 0-79  (full middle line)
Span 3: Line 7, cols 0-25  (start of last line)

// Each span rendered as semi-transparent rectangle
// Color: rgba(0.3, 0.5, 0.8, 0.3) - semi-transparent blue
```

---

## Search Flow

### Search States

```rust
pub struct SearchState {
    active: bool,
    query: String,
    current_match: Option<Point>,
    all_matches: Vec<Point>,
}
```

### Search Lifecycle

```
┌─────────────────────────────────────────────────────────┐
│ 1. ACTIVATE SEARCH (Cmd+F)                              │
│    • search_state.activate()                            │
│    • User types query in macOS dropdown overlay         │
│    • Query stored in shared buffer                      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 2. EXECUTE SEARCH                                       │
│    • search_state.search(grid, query)                   │
│    • Scan all visible lines + scrollback                │
│    • Build vector of match positions                    │
│    • Highlight first match                              │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 3. NAVIGATE MATCHES                                     │
│    • Cmd+G (next)                                       │
│    • Cmd+Shift+G (previous)                             │
│    • Wraps around at boundaries                         │
│    • Auto-scrolls to match if offscreen                 │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 4. DEACTIVATE (Escape)                                  │
│    • search_state.deactivate()                          │
│    • Clear matches and highlights                       │
│    • Return focus to terminal                           │
└─────────────────────────────────────────────────────────┘
```

### Search Algorithm

```rust
pub fn search(&mut self, grid: &Grid, query: &str) {
    self.all_matches.clear();
    
    for line_idx in (-history_size as i32)..screen_lines {
        let line = Line(line_idx);
        let text = grid_line_to_string(grid, line);
        
        // Case-insensitive search
        let lower_text = text.to_lowercase();
        let lower_query = query.to_lowercase();
        
        // Find all occurrences in this line
        for (col, _) in lower_text.match_indices(&lower_query) {
            self.all_matches.push(Point {
                line,
                column: Column(col),
            });
        }
    }
    
    self.current_match = self.all_matches.first().cloned();
}
```

---

## Natural Language Commands

### Flow Overview

```
User types natural language + Enter
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ 1. DETECTION                                            │
│    • Read line from grid                                │
│    • Strip shell prompt                                 │
│    • nl_detector.is_natural_language()                  │
│    • Heuristics: question words, articles, length       │
└────────────────┬────────────────────────────────────────┘
                 │
              YES│ (Natural Language)
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 2. VISUAL FEEDBACK                                      │
│    • Send newline to terminal (move cursor down)        │
│    • tab.ui_message = Generating { query }             │
│    • Render "Generating..." UI overlay                  │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 3. ASYNC API CALL                                       │
│    • tokio_handle.spawn(async task)                     │
│    • llm_client.generate_command(nl, context)           │
│    • context: shell, cwd, os                            │
│    • Anannas AI with Claude model                       │
│    • Non-blocking (main thread continues)               │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 4. RESPONSE HANDLING                                    │
│    • Parse commands from LLM response                   │
│    • Detect dangerous patterns (rm -rf, sudo, etc.)     │
│    • Calculate ConfirmationLevel (Standard/Sudo/Danger) │
│    • Send via channel: nl_tx.send(Suggestion)           │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 5. UI SUGGESTION (Main Thread)                          │
│    • AboutToWait receives message via nl_rx             │
│    • tab.ui_message = Suggestion { cmds, safety }      │
│    • tab.pending_nl_commands = Some(cmds)              │
│    • tab.nl_confirmation_mode = true                    │
│    • Render suggestion UI box (color-coded border)      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 6. USER CONFIRMATION                                    │
│    • User types: y, yes, n, no                          │
│    • Stored in tab.confirmation_input buffer            │
│    • On Enter: check buffer                             │
│    │                                                     │
│    ├─ "y" or "yes" → Execute commands                   │
│    │   • Clear confirmation text with backspaces        │
│    │   • Send newline                                   │
│    │   • Write each command to PTY                      │
│    │                                                     │
│    └─ "n" or "no" → Cancel                              │
│        • Clear confirmation text with backspaces        │
│        • Send newline                                   │
│        • Clear pending commands                         │
└─────────────────────────────────────────────────────────┘
```

### UI Overlay Positioning (Pane-Aware & Scroll-Aware)

```rust
// In render_with_panes_and_ui():

1. Find focused viewport
2. Lock terminal to get cursor position and scroll offset
3. Calculate cursor position in viewport:
   cursor_line_in_viewport = cursor.line + scroll_offset
4. Position UI box:
   ui_row = cursor_line_in_viewport + 1  // 1 line below cursor
   ui_col = 2                             // 2 columns indent
5. Clamp to viewport bounds
6. Convert grid to pixels within viewport:
   pixel_x_in_vp = ui_col * cell_width + PADDING_LEFT
   pixel_y_in_vp = ui_row * cell_height + PADDING_TOP
7. Add viewport offset:
   pixel_x = viewport.x + pixel_x_in_vp
   pixel_y = viewport.y + pixel_y_in_vp
8. Render cells at pixel coordinates
```

**Critical Features:**
- **Pane-aware**: Appears in focused pane, not globally
- **Scroll-aware**: Moves with terminal content when scrolling
- **Viewport-relative**: Respects split-screen boundaries

---

## Rendering Pipeline

### Frame Rendering Flow

```
window.request_redraw()
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ Event::WindowEvent::RedrawRequested                     │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ handle_redraw()                                         │
│ saternal/src/app/window.rs                             │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ renderer.render_with_panes_and_ui()                     │
│ saternal-core/src/renderer/mod.rs                      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 1. CALCULATE VIEWPORTS                                  │
│    • Traverse pane tree                                 │
│    • Calculate pixel bounds for each pane               │
│    • Account for 2px borders between panes              │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 2. PARALLEL PANE RENDERING                              │
│    • Use Rayon for CPU parallelism                      │
│    • Each pane renders to separate buffer               │
│    • text_rasterizer.render_to_buffer()                 │
│    • Rasterize glyphs with fontdue                      │
│    • Apply colors from ColorPalette                     │
│    • Account for scroll offset (focused pane)           │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 3. COMBINE BUFFERS                                      │
│    • Create full-window black buffer                    │
│    • Copy each pane buffer to viewport position         │
│    • Blit with proper alignment                         │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 4. OVERLAY UI (if present)                              │
│    • Generate UI box cells                              │
│    • Calculate viewport-relative position               │
│    • Overlay with alpha blending                        │
│    • Semi-transparent background (0.8 opacity)          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 5. UPDATE CURSOR                                        │
│    • Get cursor position from focused pane              │
│    • Account for viewport offset                        │
│    • Convert to NDC coordinates                         │
│    • Upload cursor uniforms to GPU                      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 6. GPU UPLOAD                                           │
│    • queue.write_texture(combined_buffer)               │
│    • Upload to texture manager                          │
│    • Ready for GPU render pass                          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ 7. GPU RENDER PASS                                      │
│    • Clear framebuffer (transparent black)              │
│    • Draw wallpaper (if present)                        │
│    • Draw terminal texture (full-screen quad)           │
│    • Draw selection highlights (instanced quads)        │
│    • Draw cursor overlay (quad with blink)              │
│    • Draw pane borders (instanced rectangles)           │
│    • Present frame                                      │
└─────────────────────────────────────────────────────────┘
```

### Rendering Layers (Bottom to Top)

```
┌────────────────────────────────────────┐
│ 7. Pane Borders (Yellow lines)         │ ← Top layer
├────────────────────────────────────────┤
│ 6. Cursor (Blinking block/beam/line)   │
├────────────────────────────────────────┤
│ 5. Selection Highlights (Blue overlay) │
├────────────────────────────────────────┤
│ 4. UI Overlays (NL command boxes)      │
├────────────────────────────────────────┤
│ 3. Terminal Text (All panes combined)  │
├────────────────────────────────────────┤
│ 2. Wallpaper (Optional background)     │
├────────────────────────────────────────┤
│ 1. Clear Color (Transparent black)     │ ← Bottom layer
└────────────────────────────────────────┘
```

### GPU Shader Pipeline

**Background/Wallpaper:**
- Vertex shader: Full-screen quad
- Fragment shader: Sample wallpaper texture with opacity

**Terminal Content:**
- Vertex shader: Full-screen quad
- Fragment shader: Sample terminal texture (RGBA premultiplied)

**Selection Highlights:**
- Vertex shader: Instanced quads (one per span)
- Fragment shader: Semi-transparent blue overlay
- Blend mode: Alpha blending

**Cursor:**
- Vertex shader: Single quad at cursor position
- Fragment shader: Solid color or blinking based on time
- Blend mode: Alpha blending

**Pane Borders:**
- Vertex shader: Instanced rectangles (2px thick)
- Fragment shader: Solid yellow color
- Blend mode: Opaque

---

## Scrolling and Navigation

### Scroll State

```rust
// In Renderer
scroll_offset: f32  // Fractional lines scrolled into history
```

### Scroll Flow

```
Mouse Wheel or Trackpad
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ WindowEvent::MouseWheel { delta }                      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ handle_mouse_wheel()                                    │
│ • Convert delta to lines                                │
│   - LineDelta: y * 3.0                                  │
│   - PixelDelta: y / 20.0                                │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ renderer.scroll(delta)                                  │
│ • scroll_offset += delta                                │
│ • Clamp to 0.0..history_size                           │
│ • Fractional for smooth scrolling                       │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Update Window Title                                     │
│ • Calculate percentage: (offset * 100) / history_size   │
│ • Set title: "Saternal [↑ 42%]"                        │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Render Frame                                            │
│ • Apply scroll offset when reading grid lines           │
│ • Line(row - scroll_offset) accesses scrollback         │
│ • Cursor hidden when scrolled (scroll_offset > 0)       │
│ • UI overlays move with content (scroll-aware)          │
└─────────────────────────────────────────────────────────┘
```

### Jump to Bottom

```
Shift+G or Shift+End
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ is_jump_to_bottom() returns true                        │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ renderer.reset_scroll()                                 │
│ • scroll_offset = 0.0                                   │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Update Window Title                                     │
│ • Remove scroll indicator                               │
│ • Set title: "Saternal"                                 │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Scroll-Aware Rendering

```rust
// When rendering each pane:

// 1. Get scroll offset (only for focused pane)
let pane_scroll_offset = if viewport.focused {
    scroll_offset.min(history_size as f32).round() as usize
} else {
    0 // Non-focused panes show live view
};

// 2. Access grid lines with negative indices
for row_idx in 0..rows {
    let line = Line(row_idx as i32 - pane_scroll_offset as i32);
    let cell = &term.grid()[line][column];
    // Negative line values access scrollback buffer
}

// 3. UI overlays account for scroll
let cursor_line_in_viewport = cursor.line.0 + pane_scroll_offset;
// UI box positioned relative to scrolled cursor position
```

---

## Clipboard Operations

### Copy Flow

```
Cmd+C (with selection)
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ handle_copy()                                           │
│ saternal/src/app/clipboard.rs                          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Get selection range from SelectionManager              │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Extract text from terminal grid                         │
│ • Iterate from start to end point                       │
│ • Read characters from each cell                        │
│ • Handle multi-line selections with newlines            │
│ • Skip wide char spacers                                │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Copy to system clipboard                                │
│ • macOS: NSPasteboard via clipboard-rs                  │
│ • Cross-platform clipboard API                          │
└─────────────────────────────────────────────────────────┘
```

### Paste Flow

```
Cmd+V
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ handle_paste()                                          │
│ saternal/src/app/clipboard.rs                          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Read from system clipboard                              │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Sanitize text                                           │
│ • Replace \r\n with \n                                  │
│ • Handle multi-line pastes                              │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Enable bracketed paste mode                             │
│ • Send: ESC[200~                                        │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Write text to PTY                                       │
│ • tab.write_input(text.as_bytes())                      │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Disable bracketed paste mode                            │
│ • Send: ESC[201~                                        │
└─────────────────────────────────────────────────────────┘
```

**Bracketed Paste Mode:**
- Tells shell that pasted text is not typed interactively
- Prevents auto-execution of newlines in multi-line pastes
- Shell can handle paste content specially (e.g., no auto-indent)

---

## Window Management

### Window Resize Flow

```
User resizes window
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ WindowEvent::Resized { size }                          │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ handle_resize()                                         │
│ saternal/src/app/window.rs                             │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ renderer.resize(width, height)                          │
│ • Update surface config                                 │
│ • Resize texture manager                                │
│ • Update glyph renderer screen size                     │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ Calculate new terminal size                             │
│ cols = (width - padding) / cell_width                   │
│ rows = (height - padding) / cell_height                 │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ tab.resize(cols, rows)                                  │
│ • pane_tree.resize() (recursive)                        │
│ • terminal.resize() for each pane                       │
│ • Send SIGWINCH to shell PTY                            │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Scale Factor Changed (DPI Change)

```
User moves window to different monitor
    │
    ▼
┌─────────────────────────────────────────────────────────┐
│ WindowEvent::ScaleFactorChanged { scale_factor }       │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ handle_scale_factor_changed()                           │
│ saternal/src/app/window.rs                             │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────────────┐
│ renderer.handle_scale_factor_changed(scale)             │
│ • font_manager.update_scale_factor()                    │
│ • Recalculate effective font size                       │
│ • Update glyph renderer cell dimensions                 │
│ • Update text rasterizer dimensions                     │
└────────────────┬────────────────────────────────────────┘
                 │
                 ▼
           window.request_redraw()
```

### Window Title Updates

**Idle State:**
```
"Saternal"
```

**Scrolled State:**
```
"Saternal [↑ 42%] - Press Shift+G to jump to bottom"
```

**Calculation:**
```rust
if scroll_offset > 0 && history_size > 0 {
    let percentage = (scroll_offset * 100) / history_size.max(1);
    window.set_title(&format!(
        "Saternal [↑ {}%] - Press Shift+G to jump to bottom", 
        percentage
    ));
} else {
    window.set_title("Saternal");
}
```

---

## Performance Characteristics

### Rendering Performance

**Parallel Pane Rendering:**
- CPU-bound: Uses Rayon for parallel rasterization
- Each pane renders independently
- Scales with CPU core count
- Typical: 2-4ms per frame with 2 panes on 8-core CPU

**GPU Rendering:**
- GPU-bound: Texture upload + shader passes
- Instanced rendering for selections and borders
- Typical: 1-2ms per frame at 1080p

**Total Frame Time:**
- Target: 16.67ms (60 FPS)
- Typical: 3-6ms (CPU + GPU)
- Headroom: 10ms+ for async work

### Input Latency

**Keyboard to PTY:**
- Direct path: < 1ms
- No buffering or processing delays

**PTY to Screen:**
- Polling: Every AboutToWait event (~1-2ms)
- Processing: Minimal (alacritty parser)
- Rendering: Next frame (0-16ms depending on timing)
- Total: 2-20ms typical

**Mouse Selection:**
- Immediate feedback on drag
- GPU selection updates: < 1ms
- No perceptible latency

### Natural Language Performance

**Detection:**
- Heuristic check: < 100ns
- Regex-free, zero allocations

**LLM API Call:**
- Network latency: 800ms - 2s (depends on network and API)
- Non-blocking: UI remains responsive
- Cached results: < 1ms (LRU cache hit)

**UI Overlay:**
- Rendering: < 1ms (small box, few cells)
- Alpha blending: GPU-accelerated

---

## Error Handling Patterns

### Try-Lock Pattern

```rust
// Never block the event loop
if let Some(mut lock) = resource.try_lock() {
    // Do work
} else {
    // Skip this frame or log warning
    log::warn!("Could not acquire lock, skipping");
}
```

**Why:**
- Prevents deadlocks in event loop
- Allows graceful degradation
- Main thread never blocks on locks

### Channel Communication

```rust
// Async task sends result
nl_tx.send(message).await?;

// Main thread receives non-blocking
match nl_rx.try_recv() {
    Ok(msg) => handle_message(msg),
    Err(TryRecvError::Empty) => { /* No messages */ }
    Err(TryRecvError::Disconnected) => { /* Channel closed */ }
}
```

**Why:**
- Async/sync boundary
- Non-blocking message passing
- Event loop never waits

### PTY Error Recovery

```rust
match pane.terminal.process_output() {
    Ok(bytes) => { /* Success */ }
    Err(e) => {
        log::debug!("PTY output error: {}", e);
        // Continue processing other panes
    }
}
```

**Why:**
- One pane failure doesn't affect others
- PTY closure handled gracefully
- User can close pane manually

---

## State Diagram Summary

```
┌───────────────┐
│ Application   │
│ Start         │
└───────┬───────┘
        │
        ▼
┌───────────────┐
│ Initialize    │
│ • Window      │
│ • Renderer    │
│ • TabManager  │
│ • PTY         │
└───────┬───────┘
        │
        ▼
┌───────────────────────────────────────────┐
│         Main Event Loop                   │
│                                           │
│  ┌─────────────────────────────────────┐ │
│  │  Wait for Event                     │ │
│  └────────┬────────────────────────────┘ │
│           │                               │
│           ▼                               │
│  ┌─────────────────────────────────────┐ │
│  │  Keyboard/Mouse/Window Event?       │ │
│  └────┬──────────────┬──────────────┬──┘ │
│       │              │              │    │
│    INPUT          WINDOW         ABOUT   │
│       │              │           TO WAIT  │
│       ▼              ▼              │    │
│  ┌─────────┐   ┌─────────┐        │    │
│  │ Process │   │ Process │        │    │
│  │ Input   │   │ Resize  │        │    │
│  └────┬────┘   └────┬────┘        │    │
│       │             │             │    │
│       └─────┬───────┴─────────────┘    │
│             │                           │
│             ▼                           │
│  ┌─────────────────────────────────────┐ │
│  │  Update State                       │ │
│  │  • TabManager                       │ │
│  │  • Renderer                         │ │
│  │  • PTY I/O                          │ │
│  └────────┬────────────────────────────┘ │
│           │                               │
│           ▼                               │
│  ┌─────────────────────────────────────┐ │
│  │  Request Redraw?                    │ │
│  └────┬──────────────┬─────────────────┘ │
│       │              │                    │
│     YES│              │NO                 │
│       │              │                    │
│       ▼              └──► (Continue)      │
│  ┌─────────────────────────────────────┐ │
│  │  Render Frame                       │ │
│  │  • Calculate viewports              │ │
│  │  • Render panes (parallel)          │ │
│  │  • Combine buffers                  │ │
│  │  • Upload to GPU                    │ │
│  │  • Execute render pass              │ │
│  │  • Present                          │ │
│  └─────────────────────────────────────┘ │
│           │                               │
│           └──────► (Loop)                 │
└───────────────────────────────────────────┘
        │
        ▼
┌───────────────┐
│ Exit Event    │
│ Close Window  │
└───────┬───────┘
        │
        ▼
┌───────────────┐
│ Cleanup       │
│ • Drop PTYs   │
│ • Drop GPU    │
│ • Exit        │
└───────────────┘
```

---

## Conclusion

This document provides a comprehensive overview of all UI flows in Saternal. Key architectural principles include:

1. **Non-blocking event loop** - Uses try_lock() and try_recv() to prevent deadlocks
2. **Async/sync separation** - Tokio runtime for I/O, synchronous event loop for UI
3. **Parallel rendering** - CPU parallelism for multi-pane rendering with Rayon
4. **GPU acceleration** - WGPU/Metal for efficient text rendering and compositing
5. **State isolation** - Thread-safe shared state with Arc<Mutex<T>>
6. **Graceful degradation** - Errors handled locally without crashing entire app

For specific feature details:
- Natural language commands: See `NATURAL_LANGUAGE_COMMANDS.md`
- Terminal emulation: See `TERMINAL_INPUT_REFERENCE.md`
- Architecture: See `Architecture.md`
