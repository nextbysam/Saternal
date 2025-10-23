# Terminal Input Reference for Saternal

This document outlines the essential keyboard inputs, control sequences, and opcodes needed to implement a functional terminal emulator.

## Table of Contents
1. [Control Characters (C0 Codes)](#control-characters-c0-codes)
2. [Common Terminal Signals](#common-terminal-signals)
3. [Special Key Sequences](#special-key-sequences)
4. [ANSI Escape Sequences](#ansi-escape-sequences)
5. [Editor-Specific Controls](#editor-specific-controls)
6. [VT100/xterm Key Codes](#vt100xterm-key-codes)

---

## Control Characters (C0 Codes)

These are ASCII control characters (0-31) that terminals interpret as special commands:

| Key Combination | ASCII Code | Hex | Description |
|----------------|------------|-----|-------------|
| Ctrl+@ | 0 | 0x00 | NUL (Null) |
| Ctrl+A | 1 | 0x01 | SOH - Start of Heading / Move to line start |
| Ctrl+B | 2 | 0x02 | STX - Move cursor backward |
| Ctrl+C | 3 | 0x03 | ETX - SIGINT (Interrupt) |
| Ctrl+D | 4 | 0x04 | EOT - End of Transmission / EOF |
| Ctrl+E | 5 | 0x05 | ENQ - Move to line end |
| Ctrl+F | 6 | 0x06 | ACK - Move cursor forward |
| Ctrl+G | 7 | 0x07 | BEL - Bell/Beep |
| Ctrl+H | 8 | 0x08 | BS - Backspace |
| Ctrl+I | 9 | 0x09 | HT - Horizontal Tab |
| Ctrl+J | 10 | 0x0A | LF - Line Feed |
| Ctrl+K | 11 | 0x0B | VT - Kill to end of line |
| Ctrl+L | 12 | 0x0C | FF - Form Feed / Clear screen |
| Ctrl+M | 13 | 0x0D | CR - Carriage Return / Enter |
| Ctrl+N | 14 | 0x0E | SO - Next history |
| Ctrl+P | 16 | 0x10 | DLE - Previous history |
| Ctrl+Q | 17 | 0x11 | DC1 - XON (Resume output) |
| Ctrl+R | 18 | 0x12 | DC2 - Reverse search |
| Ctrl+S | 19 | 0x13 | DC3 - XOFF (Pause output) |
| Ctrl+T | 20 | 0x14 | DC4 - Transpose characters |
| Ctrl+U | 21 | 0x15 | NAK - Kill line backward |
| Ctrl+V | 22 | 0x16 | SYN - Literal next (quote) |
| Ctrl+W | 23 | 0x17 | ETB - Kill word backward |
| Ctrl+X | 24 | 0x18 | CAN - Cancel |
| Ctrl+Y | 25 | 0x19 | EM - Yank (paste) |
| Ctrl+Z | 26 | 0x1A | SUB - SIGTSTP (Suspend) |
| Ctrl+[ | 27 | 0x1B | ESC - Escape |
| Ctrl+\ | 28 | 0x1C | FS - SIGQUIT |
| Ctrl+] | 29 | 0x1D | GS - |
| Ctrl+^ | 30 | 0x1E | RS - |
| Ctrl+_ | 31 | 0x1F | US - Undo |

---

## Common Terminal Signals

These control sequences send signals to the running process:

| Key Combination | Signal | Description |
|----------------|--------|-------------|
| Ctrl+C | SIGINT | Interrupt - terminates the current process |
| Ctrl+\ | SIGQUIT | Quit - terminates with core dump |
| Ctrl+Z | SIGTSTP | Suspend - pauses the process (can resume with `fg`) |
| Ctrl+D | EOF | End of input (closes shell if on empty line) |

**Important Note**: These signals are handled by the **line discipline** (terminal driver), not directly by the application. The terminal emulator must send these bytes, and the PTY (pseudo-terminal) converts them to signals.

---

## Special Key Sequences

These keys send escape sequences rather than single characters:

### Arrow Keys (Application Mode - DECCKM)
| Key | Sequence |
|-----|----------|
| Up Arrow | `ESC [ A` or `ESC O A` |
| Down Arrow | `ESC [ B` or `ESC O B` |
| Right Arrow | `ESC [ C` or `ESC O C` |
| Left Arrow | `ESC [ D` or `ESC O D` |

### Function Keys (VT100/xterm style)
| Key | Sequence |
|-----|----------|
| F1 | `ESC O P` |
| F2 | `ESC O Q` |
| F3 | `ESC O R` |
| F4 | `ESC O S` |
| F5 | `ESC [ 1 5 ~` |
| F6 | `ESC [ 1 7 ~` |
| F7 | `ESC [ 1 8 ~` |
| F8 | `ESC [ 1 9 ~` |
| F9 | `ESC [ 2 0 ~` |
| F10 | `ESC [ 2 1 ~` |
| F11 | `ESC [ 2 3 ~` |
| F12 | `ESC [ 2 4 ~` |

### Navigation Keys
| Key | Sequence |
|-----|----------|
| Home | `ESC [ H` or `ESC [ 1 ~` |
| End | `ESC [ F` or `ESC [ 4 ~` |
| Insert | `ESC [ 2 ~` |
| Delete | `ESC [ 3 ~` |
| Page Up | `ESC [ 5 ~` |
| Page Down | `ESC [ 6 ~` |

### Modified Keys (with Shift, Ctrl, Alt)
Modified keys add a parameter before the final character:

| Modifier | Code |
|----------|------|
| Shift | 2 |
| Alt/Meta | 3 |
| Shift+Alt | 4 |
| Ctrl | 5 |
| Shift+Ctrl | 6 |
| Alt+Ctrl | 7 |
| Shift+Alt+Ctrl | 8 |

Example: `Ctrl+Up` = `ESC [ 1 ; 5 A`

---

## ANSI Escape Sequences

ANSI escape sequences control cursor movement, colors, and screen manipulation.

### Structure
- All sequences start with `ESC` (0x1B or `\033`)
- Most use CSI (Control Sequence Introducer): `ESC [`
- Format: `ESC [ <parameters> <command>`

### Cursor Movement (Output - Terminal receives from application)
| Sequence | Description |
|----------|-------------|
| `ESC [ n A` | Cursor up n lines |
| `ESC [ n B` | Cursor down n lines |
| `ESC [ n C` | Cursor forward n columns |
| `ESC [ n D` | Cursor backward n columns |
| `ESC [ H` | Cursor to home (1,1) |
| `ESC [ row ; col H` | Cursor to position |
| `ESC [ J` | Clear screen from cursor down |
| `ESC [ 2 J` | Clear entire screen |
| `ESC [ K` | Clear line from cursor right |

### Text Formatting (SGR - Select Graphic Rendition)
| Sequence | Description |
|----------|-------------|
| `ESC [ 0 m` | Reset all attributes |
| `ESC [ 1 m` | Bold |
| `ESC [ 2 m` | Dim |
| `ESC [ 3 m` | Italic |
| `ESC [ 4 m` | Underline |
| `ESC [ 5 m` | Blink |
| `ESC [ 7 m` | Reverse video |
| `ESC [ 30-37 m` | Foreground color (30=black, 31=red, 32=green, 33=yellow, 34=blue, 35=magenta, 36=cyan, 37=white) |
| `ESC [ 40-47 m` | Background color |
| `ESC [ 90-97 m` | Bright foreground color |
| `ESC [ 100-107 m` | Bright background color |

### 256 Color and RGB
| Sequence | Description |
|----------|-------------|
| `ESC [ 38 ; 5 ; n m` | Set foreground to color n (0-255) |
| `ESC [ 48 ; 5 ; n m` | Set background to color n (0-255) |
| `ESC [ 38 ; 2 ; r ; g ; b m` | Set foreground to RGB |
| `ESC [ 48 ; 2 ; r ; g ; b m` | Set background to RGB |

---

## Editor-Specific Controls

### Nano Editor
| Key | Action |
|-----|--------|
| Ctrl+X | Exit (prompts to save) |
| Ctrl+O | Save (WriteOut) |
| Ctrl+R | Read file / Insert file |
| Ctrl+W | Search (Where is) |
| Ctrl+\ | Search and replace |
| Ctrl+K | Cut line |
| Ctrl+U | Paste (Uncut) |
| Ctrl+G | Help |
| Ctrl+Y | Page up |
| Ctrl+V | Page down |
| Alt+Y | Toggle syntax highlighting |
| Alt+$ | Toggle soft wrap |
| F3 | Save without exiting |

### Vi/Vim (Modal editing requires different input handling)
- Vim uses modal editing (normal, insert, visual modes)
- Terminal must pass through ESC cleanly
- Normal mode commands are single keys (h, j, k, l for navigation)

### Less/More Pager
| Key | Action |
|-----|--------|
| Space | Next page |
| b | Previous page |
| q | Quit |
| / | Search forward |
| ? | Search backward |
| n | Next search result |
| N | Previous search result |

---

## VT100/xterm Key Codes

### Alt/Meta Key Handling
When Alt/Meta is pressed with a character, there are two common encodings:

1. **8-bit**: Set high bit (char | 0x80)
2. **ESC prefix**: Send `ESC` followed by the character

Example: `Alt+a` → `ESC a` or `\033a`

### Bracketed Paste Mode
Modern terminals support bracketed paste to distinguish typed text from pasted text:

- Enable: `ESC [ ? 2 0 0 4 h`
- Disable: `ESC [ ? 2 0 0 4 l`
- Pasted text is wrapped: `ESC [ 2 0 0 ~ <text> ESC [ 2 0 1 ~`

### Application Cursor Keys Mode
- Enable: `ESC [ ? 1 h` (sends `ESC O` prefix)
- Disable: `ESC [ ? 1 l` (sends `ESC [` prefix)

---

## Implementation Checklist for Saternal

### Essential for Basic Functionality
- [ ] Ctrl+C (SIGINT) - interrupt process
- [ ] Ctrl+D (EOF) - end input / exit shell
- [ ] Ctrl+Z (SIGTSTP) - suspend process
- [ ] Ctrl+\ (SIGQUIT) - quit with core dump
- [ ] Backspace (Ctrl+H or DEL)
- [ ] Enter (Ctrl+M or LF)
- [ ] Tab (Ctrl+I)
- [ ] ESC key

### Navigation (Required for most editors)
- [ ] Arrow keys (Up, Down, Left, Right)
- [ ] Home / End
- [ ] Page Up / Page Down
- [ ] Delete key

### Readline/Shell Editing
- [ ] Ctrl+A (start of line)
- [ ] Ctrl+E (end of line)
- [ ] Ctrl+K (kill to end of line)
- [ ] Ctrl+U (kill line backward)
- [ ] Ctrl+W (kill word backward)
- [ ] Ctrl+L (clear screen)
- [ ] Ctrl+R (reverse search)

### Enhanced Features
- [ ] Function keys (F1-F12)
- [ ] Modified keys (Ctrl+Arrow, Shift+Arrow, etc.)
- [ ] Alt/Meta key combinations
- [ ] Bracketed paste mode
- [ ] Mouse reporting (if supporting mouse)

### ANSI Output Processing
- [ ] Basic cursor movement
- [ ] Screen clearing
- [ ] Text colors (basic 16 colors minimum)
- [ ] Bold, underline, reverse video
- [ ] 256 color support (recommended)
- [ ] RGB color support (optional but nice)

---

## References

- **ECMA-48**: Standard for control functions (official spec)
- **xterm Control Sequences**: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
- **VT100 User Guide**: Classic terminal reference
- **ANSI Escape Codes**: https://en.wikipedia.org/wiki/ANSI_escape_code
- **Console Virtual Terminal Sequences** (Microsoft): https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences

---

## Notes for Implementation

1. **PTY (Pseudo-Terminal)**: Your terminal needs to use a PTY to handle the line discipline properly. The PTY converts control characters to signals.

2. **Raw vs Canonical Mode**:
   - Canonical mode: Line buffered, handles Ctrl+C, Ctrl+D locally
   - Raw mode: Applications like vim need raw mode for character-by-character input

3. **TERM Environment Variable**: Set to `xterm-256color` or similar for best compatibility.

4. **Terminfo/Termcap**: Consider using the terminfo database for application compatibility, though most modern apps assume xterm-like behavior.

5. **UTF-8**: Ensure proper UTF-8 handling - multi-byte characters must not be split.

6. **Window Size (SIGWINCH)**: Report terminal size via `TIOCGWINSZ` and send `SIGWINCH` when resized.
