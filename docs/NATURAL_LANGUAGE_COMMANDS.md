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
🤖 Generating command with Claude...
💡 Suggested command:
  find . -name "*.rs" -type f

Execute? [y/n]: y
./src/main.rs
./saternal-core/src/lib.rs
...
```

### Git Workflows
```bash
$ commit all changes with message "Add feature"
🤖 Generating command with Claude...
💡 Suggested commands:
  1. git add .
  2. git commit -m "Add feature"

Execute? [y/n]: y
[main abc1234] Add feature
 5 files changed, 120 insertions(+)
```

### System Monitoring
```bash
$ find processes using more than 1GB of memory
🤖 Generating command with Claude...
💡 Suggested command:
  ps aux | awk '$6 > 1000000 {print $0}'

Execute? [y/n]: y
USER   PID  %CPU %MEM  VSZ     RSS   ...
```

### Text Processing
```bash
$ count lines of code in all source files
🤖 Generating command with Claude...
💡 Suggested command:
  find . -name "*.rs" -type f -exec wc -l {} + | awk '{s+=$1} END {print s}'

Execute? [y/n]: y
12847
```

## Safety Features

### Dangerous Command Detection

The system automatically detects potentially dangerous commands and requires elevated confirmation:

```bash
$ delete everything in this directory
🤖 Generating command with Claude...

⚠️  WARNING: This command may permanently delete or modify system files!

💡 Suggested command:
  rm -rf *

⚠️  Type 'yes' to execute (or 'n' to cancel): _
```

### Sudo Commands

Commands requiring root privileges show a special warning:

```bash
$ install nginx
🤖 Generating command with Claude...

🔐 This command requires root/administrator privileges.

💡 Suggested command:
  sudo apt install nginx

Execute? Type 'yes' to confirm: _
```

## How It Works

### 1. Natural Language Detection

When you press Enter, the terminal uses a fast heuristic detector (<100ns) to check if your input looks like natural language:

- **Detected as NL**: "show me all files", "find rust code", "list processes"
- **Not detected**: `ls -la`, `git status`, `cd /tmp`

Detection criteria:
- Contains question words (how, what, show, list, find, etc.)
- Contains articles (the, a, an, my, all)
- More than 5 words
- Doesn't start with known shell commands

### 2. Async API Call

If natural language is detected:
1. Display "🤖 Generating command with Claude..."
2. Spawn non-blocking tokio task
3. Call Anannas AI API with Claude model
4. Cache result for future use
5. Return to event loop (UI never blocks)

### 3. Command Presentation & Confirmation Mode

Once the API responds:
1. Parse commands from LLM response
2. Check for dangerous patterns
3. Display suggestions with appropriate warnings
4. **Store commands in memory buffer** (`pending_nl_commands`)
5. **Enter confirmation mode** (`nl_confirmation_mode = true`)
6. Wait for user confirmation

**In confirmation mode:**
- All text input is intercepted and stored in `confirmation_input` buffer
- Input is NOT passed to the shell or read from terminal grid
- This prevents the prompt text from interfering with yes/no detection

### 4. Execution

User types `y`, `yes`, `n`, or `no` and presses Enter:
- **Yes**: Commands pulled from memory buffer and executed via PTY
- **No**: Commands and buffer are cleared, memory freed
- **Invalid input**: Buffer cleared, user prompted again

After execution or cancellation:
- Confirmation mode disabled
- Memory buffers cleared
- Terminal returns to normal input mode

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
- `tab.rs`: Pending commands tracking

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
