# PTY Terminal Styling: Essential Requirements

## First Principles

When a program writes to a PTY, it outputs:
1. **Text characters** - what to display
2. **ANSI escape codes** - how to style that text

The terminal emulator must:
1. **Parse** the escape codes from the byte stream
2. **Store** style information per character
3. **Render** characters with their associated styles

That's it. Everything else is optimization or advanced features.

---

## The Essential Escape Sequence

**Format:** `\x1b[<number>m`

- `\x1b` = ESC character (byte 27)
- `[` = starts control sequence
- `<number>` = what style to apply (can be multiple: `1;31m`)
- `m` = end of style command

**Example:**
```
\x1b[31m     → Make text red
Hello        → "Hello" appears in red
\x1b[0m      → Reset to normal
World        → "World" appears normal
```

---

## Minimum Required Styles

### Text Attributes
| Code | Effect | How to Reset |
|------|--------|--------------|
| `0` | Reset all styles | N/A |
| `1` | Bold | `22` or `0` |
| `4` | Underline | `24` or `0` |
| `7` | Reverse (swap fg/bg colors) | `27` or `0` |

### Colors
**Foreground (text color):**
```
30=black  31=red    32=green  33=yellow
34=blue   35=magenta 36=cyan  37=white
39=default
```

**Background:**
```
40=black  41=red    42=green  43=yellow
44=blue   45=magenta 46=cyan  47=white
49=default
```

**Bright variants:** Add 60 (e.g., `91`=bright red, `100`=bright red background)

**Example combinations:**
```
\x1b[1m        → bold
\x1b[31m       → red text
\x1b[1;31m     → bold red text
\x1b[31;44m    → red text on blue background
```

---

## How to Parse

### State Machine (Simplified)

```
State: NORMAL
  - If byte is \x1b → go to ESCAPE state
  - Otherwise → display character with current style

State: ESCAPE
  - If byte is [ → go to CSI state
  - Otherwise → back to NORMAL

State: CSI (Control Sequence)
  - If byte is digit (0-9) or ; → accumulate parameters
  - If byte is m → execute style change, go to NORMAL
  - Otherwise → ignore, go to NORMAL
```

### Parsing Example

Input: `\x1b[1;31mHello\x1b[0m`

```
1. Read \x1b → enter ESCAPE
2. Read [    → enter CSI
3. Read 1    → param = [1]
4. Read ;    → separator
5. Read 3    → param = [1, 3...]
6. Read 1    → param = [1, 31]
7. Read m    → execute: set bold + red, back to NORMAL
8. Read H    → display 'H' with bold+red
9. Read e    → display 'e' with bold+red
... (continue with bold+red)
10. Read \x1b → enter ESCAPE
11. Read [    → enter CSI
12. Read 0    → param = [0]
13. Read m    → execute: reset all styles
```

---

## How to Store Styles

Each cell in your terminal buffer needs to store both the character and its style:

```rust
struct Cell {
    character: char,
    bold: bool,
    underline: bool,
    reverse: bool,
    fg_color: u8,  // 0-15 (or 0xFF for default)
    bg_color: u8,  // 0-15 (or 0xFF for default)
}
```

When you parse `\x1b[1;31m`, update your "current style":
```rust
current_style.bold = true;
current_style.fg_color = 1; // red
```

When you display a character, copy `current_style` into that cell.

---

## How to Render

When drawing a cell to the screen:

```rust
fn render_cell(cell: &Cell) {
    // 1. Select font
    let font = if cell.bold {
        bold_font
    } else {
        regular_font
    };

    // 2. Get colors
    let fg = color_palette[cell.fg_color];
    let bg = color_palette[cell.bg_color];

    // 3. Apply reverse if needed
    let (text_color, bg_color) = if cell.reverse {
        (bg, fg)  // swap
    } else {
        (fg, bg)
    };

    // 4. Draw
    draw_rect(bg_color);  // background
    draw_char(cell.character, font, text_color);

    if cell.underline {
        draw_line_under_char();
    }
}
```

---

## Critical Rules

### 1. Styles Persist
```
\x1b[31m      ← Turn red on
Hello         ← Red
World         ← Still red!
\x1b[0m       ← Reset
!             ← Normal
```

### 2. Styles Stack
```
\x1b[1m       ← Bold on
\x1b[31m      ← Add red (still bold)
Hello         ← Bold + red
\x1b[0m       ← Reset both
```

### 3. Reset Doesn't Remember
```
\x1b[31m      ← Red
\x1b[0m       ← Reset
\x1b[1m       ← Bold only (not bold+red)
```

---

## Reference Implementation (Minimal)

```rust
enum ParserState {
    Normal,
    Escape,
    Csi,
}

struct Parser {
    state: ParserState,
    params: Vec<u16>,
    current_param: u16,
}

impl Parser {
    fn process_byte(&mut self, byte: u8, current_style: &mut CellStyle) -> Option<char> {
        match self.state {
            ParserState::Normal => {
                if byte == 0x1b {  // ESC
                    self.state = ParserState::Escape;
                    None
                } else {
                    Some(byte as char)  // Display this char with current_style
                }
            }

            ParserState::Escape => {
                if byte == b'[' {
                    self.state = ParserState::Csi;
                    self.params.clear();
                    self.current_param = 0;
                } else {
                    self.state = ParserState::Normal;
                }
                None
            }

            ParserState::Csi => {
                match byte {
                    b'0'..=b'9' => {
                        self.current_param = self.current_param * 10 + (byte - b'0') as u16;
                        None
                    }
                    b';' => {
                        self.params.push(self.current_param);
                        self.current_param = 0;
                        None
                    }
                    b'm' => {
                        self.params.push(self.current_param);
                        self.apply_sgr(&self.params, current_style);
                        self.state = ParserState::Normal;
                        None
                    }
                    _ => {
                        self.state = ParserState::Normal;
                        None
                    }
                }
            }
        }
    }

    fn apply_sgr(&self, params: &[u16], style: &mut CellStyle) {
        for &param in params {
            match param {
                0 => style.reset(),
                1 => style.bold = true,
                4 => style.underline = true,
                7 => style.reverse = true,
                22 => style.bold = false,
                24 => style.underline = false,
                27 => style.reverse = false,
                30..=37 => style.fg_color = (param - 30) as u8,
                39 => style.fg_color = 0xFF,  // default
                40..=47 => style.bg_color = (param - 40) as u8,
                49 => style.bg_color = 0xFF,  // default
                90..=97 => style.fg_color = (param - 90 + 8) as u8,  // bright
                100..=107 => style.bg_color = (param - 100 + 8) as u8,
                _ => {} // ignore unknown
            }
        }
    }
}
```

---

## Testing Your Implementation

```bash
# Test 1: Basic color
echo -e "\x1b[31mRed\x1b[0m Normal"

# Test 2: Bold
echo -e "\x1b[1mBold\x1b[22m Normal"

# Test 3: Combination
echo -e "\x1b[1;31mBold Red\x1b[0m Normal"

# Test 4: Persistence
echo -e "\x1b[31mRed\x1b[1m Still Red + Bold\x1b[0m Normal"

# Test 5: Background
echo -e "\x1b[37;44mWhite on Blue\x1b[0m"

# Test 6: Underline + Reverse
echo -e "\x1b[4;7mUnderlined Reversed\x1b[0m"
```

If these work correctly, your implementation is sufficient for 99% of terminal applications.

---

## What This Enables

With just these basics, you can correctly display:
- `vim` with syntax highlighting
- `htop` with colors
- `ls --color` output
- `git diff` with red/green
- Most TUI applications

---

## What's NOT Covered (Intentionally)

This document deliberately excludes:
- 256-color palette (38;5;n)
- True color RGB (38;2;r;g;b)
- Extended underline styles
- Dim, italic, strikethrough attributes
- Blink
- Hidden text
- Performance optimization
- Font fallbacks
- Color schemes

These can be added later, but are not essential for a functional terminal emulator.

---

## References

**Standards:**
- ECMA-48 Section 8.3.117 (SGR definition)
- VT100/VT220 manuals

**Working implementations:**
- [xterm ctlseqs](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) - Section on SGR
- [vte parser](https://crates.io/crates/vte) - Used by Alacritty
- [DEC ANSI parser](https://vt100.net/emu/dec_ansi_parser) - State machine spec

**Test your implementation:**
- Compare output to `xterm` or `alacritty`
- Run `ls --color`, `vim`, and `htop`

---

**Document Version:** 1.0 (Minimal)
**Philosophy:** Start simple, add complexity only when needed
