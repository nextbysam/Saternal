# Terminal Scroll Gesture Implementation Reference

## Overview
This document outlines how scroll gestures work in modern terminal emulators, providing implementation guidance for saternal's scroll functionality.

## Core Concepts

### Scrollback Buffer
- Terminal emulators maintain a **scrollback buffer** to store lines that have scrolled off the visible screen
- As new output appears at the bottom, old lines are pushed off the top into the scrollback
- Users can scroll through this buffer to review past output
- Default scroll behavior: **5 rows per wheel notch** (configurable in most terminals)

### Primary vs Alternate Screen Buffer
Terminal emulators manage two separate screen buffers:

1. **Primary Screen Buffer**
   - Normal terminal operation mode
   - Has scrollback history
   - Mouse wheel scrolls through the scrollback buffer

2. **Alternate Screen Buffer**
   - Used by full-screen applications (vim, less, tmux, etc.)
   - Exact window dimensions (width × height)
   - **No scrollback** - content that scrolls off is lost
   - Mouse wheel behavior depends on application:
     - If app enables mouse reporting: events sent to application
     - If app doesn't enable mouse reporting: terminal converts scroll to Arrow Up/Down key presses

## Mouse/Trackpad Event Handling

### macOS Implementation (NSEvent)

#### Basic Event Handler Pattern
```objective-c
- (void)scrollWheel:(NSEvent *)theEvent {
    [super scrollWheel:theEvent];

    // Get scroll deltas
    CGFloat deltaY = theEvent.deltaY;
    CGFloat deltaX = theEvent.deltaX;

    // Handle vertical scrolling through scrollback buffer
    if (deltaY != 0) {
        scrollScrollbackBuffer(deltaY);
    }
}
```

#### Key NSEvent Properties
- `deltaY` - Vertical scroll amount (positive = scroll down, negative = scroll up)
- `deltaX` - Horizontal scroll amount (for horizontal scrolling if supported)
- `phase` - Tracks gesture phases (began, changed, ended, none)
- `momentumPhase` - Tracks momentum/inertial scrolling phases

#### Distinguishing Input Devices
```objective-c
// Trackpad gestures have phase information
if (theEvent.phase != NSEventPhaseNone) {
    // Modern trackpad with gesture support
    // Phases: began, changed, ended
}

// Legacy mouse wheel or Mighty Mouse
if (theEvent.phase == NSEventPhaseNone &&
    theEvent.momentumPhase == NSEventPhaseNone) {
    // Traditional mouse wheel (discrete scroll events)
}
```

### X11/Linux Implementation
- Mouse wheel events typically use **Button4** (scroll up) and **Button5** (scroll down)
- Events can be:
  - Consumed by terminal emulator for scrollback navigation
  - Passed through to applications that have enabled mouse reporting

## Implementation Strategy

### 1. Detect Scroll Events
- Capture NSScrollWheel events on macOS
- Distinguish between trackpad gestures and mouse wheel scrolls
- Track phase information for smooth scrolling feedback

### 2. Determine Screen Mode
```rust
enum ScreenMode {
    Primary,    // Normal mode with scrollback
    Alternate,  // Full-screen app mode
}

fn handle_scroll(&mut self, delta_y: f64) {
    match self.screen_mode {
        ScreenMode::Primary => {
            // Scroll through scrollback buffer
            self.scroll_viewport(delta_y);
        },
        ScreenMode::Alternate => {
            if self.mouse_reporting_enabled {
                // Send mouse event to application
                self.send_mouse_event(delta_y);
            } else {
                // Convert to arrow key presses
                self.send_arrow_keys(delta_y);
            }
        },
    }
}
```

### 3. Scrollback Buffer Navigation
```rust
struct ScrollbackBuffer {
    lines: VecDeque<Line>,
    viewport_offset: usize,  // Current scroll position
    max_scrollback: usize,   // e.g., 10000 lines
}

impl ScrollbackBuffer {
    fn scroll(&mut self, delta: f64) {
        // Convert delta to line count
        let lines = (delta / SCROLL_SENSITIVITY).round() as isize;

        // Update viewport offset
        self.viewport_offset = self.viewport_offset
            .saturating_add_signed(-lines)
            .min(self.lines.len().saturating_sub(self.visible_rows));
    }
}
```

### 4. Smooth Scrolling Support
For trackpad gestures with momentum:
```rust
fn handle_scroll_with_momentum(&mut self, event: &ScrollEvent) {
    match event.phase {
        Phase::Began => {
            self.scroll_state.active = true;
        },
        Phase::Changed => {
            // Apply scroll delta
            self.scroll_viewport(event.delta_y);
        },
        Phase::Ended => {
            self.scroll_state.active = false;
        },
        Phase::None => {
            // Legacy mouse wheel - discrete jumps
            self.scroll_viewport_discrete(event.delta_y);
        },
    }

    // Handle momentum/inertial scrolling
    if event.momentum_phase != Phase::None {
        self.scroll_viewport(event.delta_y * MOMENTUM_MULTIPLIER);
    }
}
```

## How Other Terminals Handle Scrolling

### WezTerm
- Configurable `alternate_buffer_wheel_scroll_speed`
- Converts wheel events to arrow keys in alternate screen when app doesn't support mouse
- Supports scrollbar UI element (optional)

### Alacritty
- Minimal approach - keyboard-based scrolling primarily
- Mouse wheel scrolls scrollback buffer in primary screen
- Simple, performant implementation

### Kitty
- Fast scrolling implementation
- Supports both mouse wheel and trackpad gestures
- Rich configuration options for scroll behavior

### iTerm2
- Advanced setting: "Scroll wheel sends arrow keys when in alternate screen mode"
- "Allow Mouse Reporting" toggle (Command+R)
- Highly configurable scroll sensitivity

## Configuration Options to Consider

```toml
[scrolling]
# Number of lines to scroll per wheel notch
multiplier = 3

# Maximum scrollback buffer size
scrollback_lines = 10000

# Enable smooth scrolling for trackpad gestures
smooth_scroll = true

# Scroll sensitivity (pixels per line)
sensitivity = 40.0

# In alternate screen, convert scroll to arrow keys
alternate_screen_scroll = true
```

## Edge Cases & Considerations

1. **Application Mouse Reporting**
   - Some apps (vim, tmux) enable mouse mode
   - Terminal must forward mouse events to app instead of scrolling
   - Detect via ANSI escape sequences: `\x1b[?1000h` (enable), `\x1b[?1000l` (disable)

2. **Scroll Bounds**
   - Prevent scrolling beyond scrollback buffer start
   - Prevent scrolling below current output (bottom of buffer)
   - Show visual indicator when at bounds

3. **Performance**
   - Scrolling should not block rendering
   - Consider debouncing rapid scroll events
   - Efficient scrollback buffer data structure (ring buffer/VecDeque)

4. **Natural vs Reverse Scrolling**
   - Respect system scroll direction preference
   - macOS: Check `NSEvent.isDirectionInvertedFromDevice`

## ANSI/VT Escape Sequences

### Mouse Reporting Modes
```
\x1b[?1000h  - Enable mouse button press/release reporting (X10)
\x1b[?1002h  - Enable button and drag reporting
\x1b[?1003h  - Enable any motion reporting
\x1b[?1006h  - Enable SGR extended mouse mode
```

### Mouse Wheel Events (SGR Mode)
```
\x1b[<64;x;yM  - Scroll up
\x1b[<65;x;yM  - Scroll down
```

## References
- [Julia Evans - Two ways the mouse wheel works in the terminal](https://jvns.ca/til/two-ways-the-mouse-wheel-works-in-the-terminal/)
- [WezTerm Scrollback Documentation](https://wezterm.org/scrollback.html)
- [Apple NSEvent scrollWheel Documentation](https://developer.apple.com/documentation/appkit/nsresponder/1534192-scrollwheel)
- tmux scrollback buffer implementation patterns
- xterm mouse reporting specifications

## Implementation Checklist for Saternal

- [ ] Implement NSEvent scrollWheel handler for macOS
- [ ] Create scrollback buffer data structure
- [ ] Add viewport offset tracking
- [ ] Detect primary vs alternate screen mode
- [ ] Implement scroll-to-arrow-key conversion for alternate screen
- [ ] Handle mouse reporting mode detection
- [ ] Add smooth scrolling support for trackpad gestures
- [ ] Respect system scroll direction preference
- [ ] Add scroll bounds checking
- [ ] Configure scrollback buffer size limit
- [ ] Add visual feedback for scroll position (scrollbar optional)
- [ ] Performance optimization for rapid scroll events
