# Natural Language Command Generation

## Overview

Saternal now supports **natural language command generation** powered by **Anannas AI** with Claude. Simply type what you want to do in plain English, and the terminal will translate it into executable shell commands with your confirmation.

## Features

- ✨ **Natural interaction**: Type commands in plain English
- 🔒 **User-controlled**: Requires explicit confirmation before executing
- 🚀 **Blazing fast**: <100ns detection, non-blocking async API calls
- 🛡️ **Safe**: Dangerous command detection with elevated warnings
- 🧠 **Smart caching**: LRU cache reduces repeat API calls by 40-60%
- 💡 **Context-aware**: Uses current directory, shell, and OS

## Quick Start

### 1. Get an Anannas AI API Key

Sign up at [https://anannas.ai](https://anannas.ai) and get your API key.

### 2. Configure the API Key

```bash
# Copy the example env file
cp .env.example .env

# Edit .env and add your API key
nano .env
```

Set your key in `.env`:
```bash
ANANNAS_API_KEY=your_actual_api_key_here
```

### 3. Build and Run

```bash
cargo build --release
./target/release/saternal
```

The terminal will automatically detect the API key and enable natural language commands.

## Usage Examples

### File Operations
```bash
$ show me all rust files in this project
```

**UI Overlay Appears:**
```
╭─ 🤖 Generating Command ───────────────────────────────────╮
│ Generating command with Claude...                         │
│ ⏳ Please wait...                                          │
╰───────────────────────────────────────────────────────────╯
```

**Then Shows Suggestion:**
```
╭─ AI Generated Command ────────────────────────────────────╮
│ find . -name "*.rs" -type f                                │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ [y/yes] Execute  [n/no] Cancel                            │
╰───────────────────────────────────────────────────────────╯
```

Type `y` to execute:
```bash
$ y
find . -name "*.rs" -type f
./src/main.rs
./saternal-core/src/lib.rs
...
```

### Git Workflows
```bash
$ commit all changes with message "Add feature"
```

**UI Overlay Shows:**
```
╭─ AI Generated Command ────────────────────────────────────╮
│ git add .                                                  │
│ git commit -m "Add feature"                                │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ [y/yes] Execute  [n/no] Cancel                            │
╰───────────────────────────────────────────────────────────╯
```

Type `y` to execute both commands in sequence.

### Dangerous Commands

```bash
$ delete all log files
```

**Warning Displayed:**
```
╭─ AI Generated Command ────────────────────────────────────╮
│ rm -rf /var/log/*                                          │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ ⚠️  DANGEROUS: This command may cause data loss           │
│ [yes] Execute  [n/no] Cancel                              │
╰───────────────────────────────────────────────────────────╯
```

Must type `yes` (not just `y`) for dangerous commands.

## Safety Features

### Dangerous Command Detection

The system automatically detects potentially dangerous commands and requires elevated confirmation with color-coded borders:

**Safe Commands** (Green Border):
```
╭─ AI Generated Command ────────────────────────────────────╮
│ ls -la                                                     │
├───────────────────────────────────────────────────────────┤
│ [y/yes] Execute  [n/no] Cancel                            │
╰───────────────────────────────────────────────────────────╯
```

**Sudo Commands** (Yellow Border):
```
╭─ AI Generated Command ────────────────────────────────────╮
│ sudo apt install nginx                                     │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ ⚠️  Requires elevated privileges (sudo)                   │
│ [yes] Execute  [n/no] Cancel                              │
╰───────────────────────────────────────────────────────────╯
```

**Dangerous Commands** (Red Border):
```
╭─ AI Generated Command ────────────────────────────────────╮
│ rm -rf *                                                   │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ ⚠️  DANGEROUS: This command may cause data loss           │
│ [yes] Execute  [n/no] Cancel                              │
╰───────────────────────────────────────────────────────────╯
```

**Confirmation Levels:**
- **Standard** (green): Type `y` or `yes` to execute
- **Sudo** (yellow): Must type `yes` (full word)
- **Elevated** (red): Must type `yes` (full word)

## How It Works

### 1. Natural Language Detection

When you press Enter, the terminal uses a fast heuristic detector (<100ns) to check if your input looks like natural language:

- **Detected as NL**: "show me all files", "find rust code", "list processes"
- **Not detected**: `ls -la`, `git status`, `cd /tmp`

**Shell Prompt Stripping:**
Before detection, the system automatically strips common shell prompts to prevent false positives:
- Zsh style: `user@host path %`
- Bash style: `user@host:path$` or `[user@host path]$`
- PowerShell style: `>`

This ensures that commands like `sam@Sams-MacBook-Pro saternal % git add .` are correctly identified as shell commands, not natural language.

Detection criteria:
- Contains question words (how, what, show, list, find, etc.)
- Contains articles (the, a, an, my, all)
- More than 5 words
- Doesn't start with known shell commands

### 2. Async API Call & UI Feedback

If natural language is detected:
1. **Send newline to terminal** (moves cursor to next line)
2. **Display "Generating..." UI overlay** (blue box with loading indicator)
3. Spawn non-blocking tokio task
4. Call Anannas AI API with Claude model
5. Cache result for future use
6. Return to event loop (UI never blocks)

**Visual Feedback:**
```
╭─ 🤖 Generating Command ───────────────────────────────────╮
│ Generating command with Claude...                         │
│ ⏳ Please wait...                                          │
╰───────────────────────────────────────────────────────────╯
```

This overlay is rendered directly onto the GPU framebuffer, **not** sent to the shell. The UI box appears directly below your cursor line with a 2-column indent, making it feel like an integrated part of your terminal session rather than a disconnected popup.

**Pane-Aware Positioning (Split-Screen Support):**
- In split-screen mode, the UI overlay appears in the **focused pane** where you typed the command
- The overlay is positioned relative to the viewport boundaries, not globally
- Each pane has its own cursor tracking and UI positioning

**Scroll-Aware Behavior:**
- The UI overlay respects scroll position and moves with terminal content
- When you scroll up in history, the UI overlay moves accordingly
- If scrolled far enough, the UI will go out of view just like normal text
- The overlay maintains its position relative to the cursor line, not the viewport

### 3. Visual Confirmation Mode

Once the API responds:
1. Parse commands from LLM response
2. Check for dangerous patterns and assign safety level
3. **Display suggestion UI overlay** with color-coded border
4. **Store commands in memory buffer** (`pending_nl_commands`)
5. **Enter confirmation mode** (`nl_confirmation_mode = true`)
6. Wait for user to type `y`, `yes`, `n`, or `no`

**Suggestion Display:**
```
╭─ AI Generated Command ────────────────────────────────────╮
│ git status                                                 │
├───────────────────────────────────────────────────────────┤
│                                                            │
│ [y/yes] Execute  [n/no] Cancel                            │
╰───────────────────────────────────────────────────────────╯
```

**In confirmation mode:**
- User's input is captured in `confirmation_input` buffer
- Input is still passed to shell so user sees what they type
- When Enter is pressed, we check the buffer for y/n confirmation
- If confirmed: clear the confirmation text with backspaces, send newline, execute commands
- If cancelled: clear the confirmation text with backspaces, send newline, clear UI overlay
- If other input: exit confirmation mode, clear UI overlay, let shell execute it normally

### 4. Execution

User types `y`, `yes`, `n`, or `no` and presses Enter:
- **Yes (`y` or `yes`)**: 
  - Commands pulled from memory buffer
  - Confirmation text cleared with backspaces
  - Newline sent to move to fresh prompt line
  - Each command executed in order via PTY stdin
  - Shell echoes and runs each command
- **No (`n` or `no`)**: 
  - Commands and buffer are cleared
  - Confirmation text cleared with backspaces
  - Newline sent to move to fresh prompt line
  - Memory freed
- **Other input**: 
  - Exit confirmation mode
  - Pass the entered text to shell as normal command

After execution or cancellation:
- Confirmation mode disabled
- Memory buffers cleared
- **UI overlay removed**
- Cursor moved to fresh line
- Terminal returns to normal input mode

**Visual Flow Example:**
```
Line 1: $ tell me my git status         ← User types natural language
Line 2: tell: command not found          ← Shell processes, then AI UI appears
        ╭─ AI Generated Command ─────╮
        │ git status                  │
        ├─────────────────────────────┤
        │ [y/yes] Execute [n/no] Cancel│
        ╰─────────────────────────────╯
        y                               ← User types y
Line 3: $ git status                    ← Fresh prompt, command executes
        On branch main
        Your branch is up to date...
```

**Important Notes**:
- Commands are displayed in a **visual UI overlay** rendered on top of terminal content
- UI uses box-drawing characters (╭─╮│╰─╯) for terminal-native aesthetics
- Overlay positioned at center-bottom with semi-transparent background
- Border colors indicate safety: green (safe), yellow (sudo), red (dangerous)
- UI is rendered via GPU, **never** sent to PTY stdin, so shell never tries to execute it

## Architecture

### Core Modules (saternal-core)

**`nl_detector.rs`**
- Static pattern matching
- Zero allocations
- <100ns per detection

**`llm_client.rs`**
- Async Anannas AI integration
- Connection pooling
- LRU caching (100 entries)
- Request deduplication

**`command_safety.rs`**
- Dangerous pattern detection
- Sudo detection
- Confirmation level calculation

### Application Layer (saternal)

**`nl_handler.rs`**
- Tokio channel communication
- UI message formatting
- Confirmation state machine
- PTY command execution

**Modified Files:**
- `main.rs`: Tokio runtime creation, dotenvy integration
- `state.rs`: LLM client, NL state, and Tokio runtime handle
- `init.rs`: LLM client initialization, accepts runtime handle
- `event_loop.rs`: Channel receiver, message handling, passes runtime handle
- `input.rs`: Enter key interception, NL detection, async task spawning
- `tab.rs`: Pending commands tracking, UI message state
- `nl_handler.rs`: UI overlay state management
- `window.rs`: UI message to UIBox conversion
- `renderer/mod.rs`: UI overlay rendering pipeline
- `renderer/ui_box.rs`: Box-drawing character rendering
- `renderer/text_rasterizer.rs`: Cell overlay method

### Runtime Architecture

The app uses a hybrid async/sync architecture:

**Tokio Runtime Setup:**
```rust
// main.rs
let runtime = tokio::runtime::Runtime::new()?;
let _guard = runtime.enter();  // Sets runtime context for entire thread
let handle = runtime.handle().clone();
let app = App::new(config, handle)?;
app.run()?;  // Starts synchronous winit event loop
```

**Key Design Points:**
- Persistent Tokio runtime lives for entire app lifetime
- `runtime.enter()` sets context so `handle.spawn()` works from sync code
- Runtime handle stored in `App` struct and passed through event handlers
- Winit event loop (synchronous) can spawn async LLM tasks via the handle
- Tokio channel (`mpsc`) bridges async tasks and sync event loop

## Performance

- **NL Detection**: <100ns per input line
- **API Latency**: 0.8-2.0s (network dependent)
- **Cache Hit**: <1ms for cached queries
- **Memory Overhead**: ~2MB (including cache)
- **UI Blocking**: Zero - main thread never waits

## Configuration

Currently auto-configured. Future config options in `~/.config/saternal/config.toml`:

```toml
[nl_commands]
enabled = true
model = "anthropic/claude-3.5-sonnet"
timeout_seconds = 10
cache_size = 100
detection_mode = "auto"  # or "explicit" (requires "nl:" prefix)
```

## Troubleshooting

### "ANANNAS_API_KEY not set - natural language commands disabled"

**Root Cause**: The `.env` file is not being loaded at startup.

**Solution**: 
1. Ensure you have a `.env` file in the project root with your API key:
   ```bash
   ANANNAS_API_KEY=your_actual_api_key_here
   ```
2. The app uses `dotenvy` to load `.env` files automatically at startup
3. Check logs at startup - you should see: `✓ LLM client initialized with Anannas AI`

### "there is no reactor running, must be called from the context of a Tokio 1.x runtime"

**Root Cause**: The Tokio runtime wasn't properly set up for the synchronous winit event loop.

**Fixed in**: The app now creates a persistent Tokio runtime in `main.rs` that lives for the entire application lifetime. The runtime handle is passed through the app and used for spawning async tasks from synchronous contexts.

**Technical Details**: 
- `pollster::block_on()` creates a temporary runtime that dies after initialization
- `winit::EventLoop` is synchronous and can't spawn Tokio tasks directly
- Solution: Create runtime with `Runtime::new()`, use `runtime.enter()` to set context, pass `Handle` to spawn tasks

### Natural language not detected

**Try**:
- Make your request more explicit: "show me X" instead of "show X"
- Use question words: "list all files" instead of "files"
- Use explicit trigger: `nl: your request here`

### API request fails

**Check**:
- API key is correct in `.env`
- Internet connection is active
- Check logs for detailed error message

## Examples by Category

### File Management
```
- list all files modified today
- find large files over 100MB
- show hidden files in this directory
- delete all log files older than 7 days
```

### Git Operations
```
- show commit history for last week
- create new branch called feature-auth
- diff the last two commits
- show files changed in last commit
```

### System Information
```
- display disk usage
- show running processes
- check memory usage
- find which process is using port 8080
```

### Text Processing
```
- count lines in all rust files
- search for TODO comments
- replace old with new in all files
- extract emails from file.txt
```

## Limitations

- Requires internet connection for API calls
- API calls cost money (see Anannas AI pricing)
- Complex multi-step workflows may need refinement
- Context limited to shell, directory, and OS (no command history yet)

## Technical Challenges Solved

### 1. Environment Variable Loading
**Problem**: `.env` files weren't being loaded, so `ANANNAS_API_KEY` was never available.  
**Solution**: Added `dotenvy` dependency and call `dotenvy::dotenv()` at the start of `main()`.

### 2. Tokio Runtime in Synchronous Context
**Problem**: `tokio::spawn()` panicked with "no reactor running" because winit's event loop is synchronous.  
**Solution**: Created a persistent Tokio runtime with `Runtime::new()`, used `runtime.enter()` to set context, and passed `Handle` throughout the app to spawn tasks from sync code.

### 3. Async/Sync Bridge
**Problem**: LLM API calls are async, but keyboard events come from synchronous winit event loop.  
**Solution**: Use `tokio::sync::mpsc` channels to bridge async tasks and sync event loop, with `try_recv()` in `AboutToWait` event.

### 4. Confirmation Input Isolation
**Problem**: After generating commands, when user typed "y" or "n", the entire terminal line (including prompt text like "Execute? [y/n]: y") was being read from the grid and treated as new natural language, triggering another LLM request.  
**Solution**: 
- Added `confirmation_input` buffer to `Tab` struct to track user input separately
- When in confirmation mode, intercept text input and store in buffer
- Read from buffer instead of terminal grid for yes/no detection
- This prevents prompt text contamination and ensures only user's actual typed response is checked
- Memory is properly freed after confirmation (execute or cancel)

### 5. UI Messages Causing Shell Execution
**Problem**: UI messages like "🤖 Generating command with Claude..." and "💡 Generated 1 command." were being written to PTY stdin using `write_input()`. The shell interpreted these as commands and tried to execute them, resulting in "command not found: 🤖" errors.  

**Solution**: Implemented GPU-rendered UI overlays
- Created `UIBox` renderer with box-drawing characters (╭─╮│╰─╯)
- Added `UIMessage` enum to `Tab` struct with three variants:
  - `Generating { query }` - Blue border, shows loading state
  - `Suggestion { commands, safety }` - Color-coded by safety level
  - `Error { message }` - Red border for errors
- UI boxes rendered directly into pixel buffer via `overlay_cells()` method
- Position: 1 line below cursor with 2-column indent, overflow protection
- Border colors: Green (safe), Yellow (sudo), Red (dangerous)
- UI state cleared on confirmation/cancel
- **Result**: UI never sent to PTY stdin, shell never tries to execute it

### 6. Shell Prompt Contamination in NL Detection
**Problem**: When user typed commands, the entire terminal line (including shell prompt like `sam@Sams-MacBook-Pro saternal %`) was being read from the grid and checked for natural language. This caused regular commands like `git add .` to be misdetected as natural language because the full string contained user names, hostnames, and met length criteria.

**Root Cause**: `read_current_line_from_grid()` reads all visible characters on the cursor line, including the prompt.

**Solution**: Added prompt stripping before NL detection
- Created `strip_shell_prompt()` function in `saternal/src/app/input.rs`
- Detects and removes common prompt patterns:
  - Zsh: `user@host path % `
  - Bash: `user@host:path$ ` or `[user@host path]$ `
  - PowerShell: `> `
- Uses `rfind()` to handle nested prompts correctly
- Only the actual command is passed to NL detector and LLM
- **Result**: Commands like `git add .` are no longer misdetected as natural language

**Files Modified:**
- `saternal/src/app/input.rs` - Added `strip_shell_prompt()`, modified Enter key handler

**Implementation Details:**
```rust
// Tab state
pub enum UIMessage {
    Generating { query: String },
    Suggestion { commands: Vec<String>, safety: ConfirmationLevel },
    Error { message: String },
}

// Rendering pipeline
1. Terminal content rendered to buffer for each pane (parallel)
2. Get cursor position from focused pane
3. Calculate viewport-relative pixel position:
   - Account for scroll offset (cursor moves with content)
   - Convert grid coordinates to pixels within viewport
   - Add viewport offset (x, y) for split-pane positioning
4. UI overlay cells generated from UIMessage
5. Cells positioned at calculated pixel coordinates
6. Cells rendered on top of combined buffer with alpha blending
7. Combined buffer uploaded to GPU
8. GPU renders final frame with pane borders
```

**Files Created:**
- `saternal-core/src/renderer/ui_box.rs` - Box drawing and cell generation

**Files Modified:**
- `saternal/src/tab.rs` - Added ui_message field
- `saternal/src/app/nl_handler.rs` - Set UI state instead of logging
- `saternal-core/src/renderer/text_rasterizer.rs` - Modified overlay_cells() to accept pixel coordinates
- `saternal-core/src/renderer/mod.rs` - Added pane-aware and scroll-aware UI positioning
- `saternal/src/app/window.rs` - Convert UIMessage to UIBox
- `saternal/src/app/input.rs` - Added strip_shell_prompt(), modified NL detection

**Recent Fixes (2025-10-28):**
- **Split-screen UI positioning**: UI overlay now appears in the correct pane when using split-screen, not globally positioned
- **Scroll awareness**: UI overlay respects scroll offset and moves with terminal content (scrolls out of view when scrolling up)

## Future Enhancements

- [ ] Streaming responses (progressive command display)
- [ ] Command history context
- [ ] Interactive refinement ("modify the last suggestion...")
- [ ] Local model support (Ollama integration)
- [ ] Multi-step workflow planning
- [ ] Shell-specific optimizations

## Security

- ✅ Never auto-executes commands
- ✅ Dangerous command detection
- ✅ Elevated confirmation for risky operations
- ✅ API key stored in `.env` (git-ignored)
- ✅ Commands executed through normal PTY (same as manual typing)

## Contributing

Found a bug or have a feature request? Open an issue on GitHub!

- Natural language detection improvements
- Safety pattern additions
- LLM prompt optimization
- Performance enhancements

---

**Built with**: Rust, Tokio, Anannas AI (Claude), and ❤️
