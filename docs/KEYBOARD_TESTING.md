# Keyboard Input Testing Guide for Saternal

This document provides a comprehensive testing guide for all the keyboard input sequences implemented in Saternal.

## How to Test

1. Build and run Saternal:
   ```bash
   cd /Users/sam/saternal
   cargo run --release
   ```

2. Press `Cmd+` ` (Command + Backtick) to toggle the dropdown terminal

3. Test each category below

---

## ✅ Control Characters (C0 Codes)

### Essential Terminal Controls
| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **Interrupt Process** | `Ctrl+C` | Sends SIGINT - should interrupt running commands (try `sleep 10` then Ctrl+C) |
| **EOF/Exit Shell** | `Ctrl+D` | Sends EOF - exits shell if line is empty |
| **Suspend Process** | `Ctrl+Z` | Sends SIGTSTP - suspends process (can resume with `fg`) |
| **Quit Process** | `Ctrl+\` | Sends SIGQUIT - quits with core dump |

### Readline/Shell Editing (Bash/Zsh)
| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **Beginning of Line** | `Ctrl+A` | Move cursor to start of line |
| **End of Line** | `Ctrl+E` | Move cursor to end of line |
| **Backward Char** | `Ctrl+B` | Move cursor back one character |
| **Forward Char** | `Ctrl+F` | Move cursor forward one character |
| **Kill to End** | `Ctrl+K` | Delete from cursor to end of line |
| **Kill Line Backward** | `Ctrl+U` | Delete from cursor to start of line |
| **Kill Word Backward** | `Ctrl+W` | Delete word before cursor |
| **Yank (Paste)** | `Ctrl+Y` | Paste previously killed text |
| **Clear Screen** | `Ctrl+L` | Clear terminal screen |
| **Reverse Search** | `Ctrl+R` | Enter reverse history search mode |
| **Next History** | `Ctrl+N` | Next command in history |
| **Previous History** | `Ctrl+P` | Previous command in history |
| **Transpose Chars** | `Ctrl+T` | Swap character under cursor with previous |

### Flow Control
| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **XON (Resume)** | `Ctrl+Q` | Resume output (after Ctrl+S) |
| **XOFF (Pause)** | `Ctrl+S` | Pause output (use Ctrl+Q to resume) |

---

## ✅ Navigation Keys

### Arrow Keys
| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **Up Arrow** | `↑` | Move cursor up / previous command in history |
| **Down Arrow** | `↓` | Move cursor down / next command in history |
| **Left Arrow** | `←` | Move cursor left |
| **Right Arrow** | `→` | Move cursor right |

### Modified Arrow Keys
| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **Ctrl+Up** | `Ctrl+↑` | Application-specific (vim: page up) |
| **Ctrl+Down** | `Ctrl+↓` | Application-specific (vim: page down) |
| **Ctrl+Left** | `Ctrl+←` | Jump word backward (bash/zsh) |
| **Ctrl+Right** | `Ctrl+→` | Jump word forward (bash/zsh) |
| **Shift+Left** | `Shift+←` | Select text leftward (if supported) |
| **Shift+Right** | `Shift+→` | Select text rightward (if supported) |

### Home/End Keys
| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **Home** | `Home` | Jump to beginning of line |
| **End** | `End` | Jump to end of line |

### Page Navigation
| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **Page Up** | `PgUp` | Scroll up (less/more/vim) |
| **Page Down** | `PgDn` | Scroll down (less/more/vim) |

### Delete Keys
| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **Backspace** | `Backspace` | Delete character before cursor |
| **Delete** | `Delete` | Delete character under cursor |
| **Insert** | `Insert` | Toggle insert/overwrite mode (vim) |

---

## ✅ Function Keys

| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **F1** | `F1` | Help (many programs) |
| **F2** | `F2` | Rename (mc, ranger) |
| **F3** | `F3` | View (mc) |
| **F4** | `F4` | Edit (mc) |
| **F5** | `F5` | Refresh/Copy (mc) |
| **F6** | `F6` | Move (mc) |
| **F7** | `F7` | Create directory (mc) |
| **F8** | `F8` | Delete (mc) |
| **F9** | `F9` | Menu (mc) |
| **F10** | `F10` | Exit (mc) |
| **F11** | `F11` | Fullscreen (many programs) |
| **F12** | `F12` | Save (many editors) |

---

## ✅ Special Keys

| Test | Key | Expected Behavior |
|------|-----|-------------------|
| **Enter** | `Enter` | Execute command / newline |
| **Tab** | `Tab` | Autocomplete / indent |
| **Shift+Tab** | `Shift+Tab` | Reverse tab (backtab) |
| **Escape** | `Esc` | Cancel / exit mode (vim: exit insert mode) |

---

## ✅ Alt/Meta Key Combinations

These send ESC prefix followed by the character:

| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **Alt+A** | `Alt+A` | Sends `ESC a` - varies by application |
| **Alt+B** | `Alt+B` | Backward word (bash/zsh) |
| **Alt+F** | `Alt+F` | Forward word (bash/zsh) |
| **Alt+D** | `Alt+D` | Delete word forward (bash/zsh) |
| **Alt+.** | `Alt+.` | Insert last argument of previous command |

---

## ✅ Saternal-Specific Shortcuts

These are macOS-specific and don't go to the terminal:

| Test | Key Combination | Expected Behavior |
|------|----------------|-------------------|
| **Toggle Dropdown** | `Cmd+`` | Show/hide terminal dropdown |
| **Increase Font** | `Cmd++` or `Cmd+=` | Increase font size |
| **Decrease Font** | `Cmd+-` | Decrease font size |
| **Reset Font** | `Cmd+0` | Reset font size to default (14pt) |

---

## 🧪 Testing Scenarios

### Scenario 1: Basic Shell Navigation
```bash
# Type a long command
echo "This is a very long command line for testing navigation"

# Test navigation:
# - Ctrl+A (should jump to start)
# - Ctrl+E (should jump to end)
# - Ctrl+B / Ctrl+F (should move char by char)
# - Ctrl+W (should delete "navigation")
# - Ctrl+U (should clear entire line)
```

### Scenario 2: History Navigation
```bash
# Type some commands
ls -la
pwd
echo "test"

# Test history:
# - Up arrow (should show "echo test")
# - Up arrow again (should show "pwd")
# - Down arrow (should go forward)
# - Ctrl+R then type "ls" (should search for ls command)
```

### Scenario 3: Process Control
```bash
# Start a long-running process
sleep 30

# Test interruption:
# - Ctrl+C (should cancel immediately)

# Start another process
sleep 30

# Test suspension:
# - Ctrl+Z (should suspend - shows [1]+ Stopped)
# - Type 'fg' to resume
```

### Scenario 4: Text Editing
```bash
# Start typing a command
echo "testing keyboard input"

# Test editing:
# - Ctrl+A (go to start)
# - Ctrl+K (delete to end - saves to kill ring)
# - Type something else
# - Ctrl+Y (should paste back the killed text)
```

### Scenario 5: Vim Testing
```bash
# Open a file in vim
vim test.txt

# Test vim keys:
# - i (enter insert mode)
# - Esc (exit insert mode)
# - Arrow keys (navigate)
# - Ctrl+F (page down)
# - Ctrl+B (page up)
# - :wq (save and quit)
```

### Scenario 6: Nano Editor
```bash
# Open nano
nano test.txt

# Test nano keys:
# - Type some text
# - Ctrl+K (cut line)
# - Ctrl+U (uncut/paste line)
# - Ctrl+X (exit - should prompt to save)
# - Y (yes to save)
# - Enter (confirm filename)
```

### Scenario 7: Less Pager
```bash
# View a large file
less /var/log/system.log

# Test pager keys:
# - Space (next page)
# - b (previous page)
# - Arrow keys (scroll line by line)
# - / (search)
# - q (quit)
```

---

## 📊 Implementation Status

### ✅ Fully Implemented
- [x] Control characters (Ctrl+A through Ctrl+Z)
- [x] Arrow keys (Up, Down, Left, Right)
- [x] Modified arrow keys (Ctrl+Arrow, Shift+Arrow, Alt+Arrow)
- [x] Navigation keys (Home, End, PageUp, PageDown, Insert, Delete)
- [x] Function keys (F1-F12)
- [x] Special keys (Enter, Tab, Backspace, Escape)
- [x] Alt/Meta key combinations (ESC prefix)
- [x] Shift+Tab (backtab)

### 🔧 Advanced Features (Implemented but needs PTY support)
- [x] Bracketed paste mode functions (implemented, needs terminal initialization)

---

## 🐛 Known Issues / Notes

1. **Bracketed Paste**: The infrastructure is implemented but requires terminal initialization to enable `?2004h` mode
2. **Application Cursor Mode**: Currently using normal mode (ESC[A) not application mode (ESC OA)
3. **Some Ctrl combinations** might conflict with macOS system shortcuts

---

## 📝 Testing Checklist

Use this checklist when testing the terminal:

### Basic Input
- [ ] Regular typing works (a-z, A-Z, 0-9, symbols)
- [ ] Enter executes commands
- [ ] Backspace deletes characters
- [ ] Tab autocompletes

### Navigation
- [ ] Arrow keys navigate cursor
- [ ] Home/End jump to line boundaries
- [ ] Ctrl+A / Ctrl+E jump to line boundaries

### Editing
- [ ] Ctrl+K kills to end of line
- [ ] Ctrl+U kills to start of line
- [ ] Ctrl+W kills word backward
- [ ] Ctrl+Y yanks killed text

### Process Control
- [ ] Ctrl+C interrupts running process
- [ ] Ctrl+Z suspends process
- [ ] Ctrl+D exits shell on empty line

### History
- [ ] Up/Down arrows navigate history
- [ ] Ctrl+R reverse searches history

### Editors
- [ ] Vim works (insert mode, navigation, commands)
- [ ] Nano works (all Ctrl commands)
- [ ] Less/More work (navigation, search, quit)

---

**Testing Date**: 2025-10-23
**Implementation Status**: Complete
**Next Steps**: Test with real applications and refine based on user feedback
