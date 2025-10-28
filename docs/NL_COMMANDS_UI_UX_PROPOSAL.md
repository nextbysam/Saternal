# Natural Language Commands UI/UX Proposal

**Date:** October 28, 2025
**Status:** Draft Proposal
**Related:** `saternal-core/src/llm_client.rs`, `NATURAL_LANGUAGE_COMMANDS.md`

## Executive Summary

This document proposes a comprehensive UI/UX design for Saternal's natural language to shell command feature. Based on research of leading implementations (Warp, ai-shell, PromptShell, Spren, ShellGPT, uwu), this proposal outlines specific interaction patterns, visual design, and safety mechanisms to create an intuitive, secure, and delightful command generation experience.

---

## Table of Contents

1. [Research Findings](#research-findings)
2. [UI/UX Principles](#uiux-principles)
3. [Interaction Patterns](#interaction-patterns)
4. [Visual Design Specifications](#visual-design-specifications)
5. [User Flows](#user-flows)
6. [Safety & Error Handling](#safety--error-handling)
7. [Implementation Recommendations](#implementation-recommendations)

---

## Research Findings

### Key Players Analyzed

| Tool | Language | Approach | Key UX Differentiator |
|------|----------|----------|----------------------|
| **Warp** | Rust | Inline suggestions | Context-aware prompt suggestions, keyboard shortcuts |
| **ai-shell** | TypeScript | CLI with explanation | Three-option flow (Execute/Revise/Cancel) |
| **PromptShell** | Python | REPL mode | Prefix-based commands (!?, --) |
| **Spren** | Rust | Preview-first | Platform-specific command adaptation |
| **ShellGPT** | Python | Multi-mode | REPL + shell integration + chat sessions |
| **uwu** | Shell | Direct inject | Minimal friction, edits in-place |

### Common UX Patterns

1. **Preview Before Execute** - Universal pattern across all tools
2. **Confirmation Gates** - Explicit user approval required
3. **Explanation Layer** - Context about what command does
4. **Safety Indicators** - Visual warnings for dangerous operations
5. **Keyboard-First** - Efficient shortcuts for power users
6. **Multi-Mode Support** - Different interaction styles (inline, REPL, chat)

### Differentiated Features

- **Warp**: Inline banners, context-aware suggestions, suggested code diffs
- **ai-shell**: Numbered explanations, revision flow
- **ShellGPT**: Function call visibility, markdown rendering
- **uwu**: Shell history integration, clipboard support

---

## UI/UX Principles

### 1. Safety First
Natural language commands can be powerful and potentially dangerous. Every design decision should prioritize user safety through:
- Clear preview of what will execute
- Explicit confirmation before execution
- Visual warnings for destructive operations
- Undo/rollback where possible

### 2. Transparency
Users should always understand:
- What command was generated
- Why that command was chosen
- What the command will do
- What risks exist

### 3. Progressive Disclosure
- Simple by default, powerful when needed
- Don't overwhelm beginners with options
- Provide escape hatches for advanced users
- Layer information intelligently

### 4. Speed & Efficiency
- Keyboard shortcuts for common actions
- Minimal friction between thought and action
- Smart defaults reduce decision fatigue
- Cache and learn from user patterns

### 5. Contextual Intelligence
- Understand current directory, shell, OS
- Learn from command history
- Adapt to user expertise level
- Provide relevant suggestions

---

## Interaction Patterns

### Pattern 1: Inline Suggestion Mode (Primary)

**Trigger:** User types natural language after special prefix

**Input:**
```
> ? list all files larger than 100MB
```

**Output Display:**
```
╭─ AI Generated Command ────────────────────────────────────╮
│ find . -type f -size +100M -exec ls -lh {} \; | awk      │
│ '{print $5, $9}' | sort -rh                              │
├─ Explanation ─────────────────────────────────────────────┤
│ 1. Searches current directory for files >100MB           │
│ 2. Displays file sizes in human-readable format          │
│ 3. Sorts results by size (largest first)                 │
├─ Actions ─────────────────────────────────────────────────┤
│ [↵ Enter] Execute  [⌘E] Edit  [⌘R] Revise  [Esc] Cancel  │
╰───────────────────────────────────────────────────────────╯
```

**Visual Specifications:**
- **Box Style:** Rounded corners, subtle shadow
- **Colors:**
  - Border: `#4A9EFF` (blue for neutral commands)
  - Background: Semi-transparent overlay over terminal
  - Text: Terminal foreground color
  - Accent: `#50FA7B` for safe, `#FFB86C` for caution, `#FF5555` for dangerous
- **Typography:**
  - Command: Monospace, slightly larger than terminal text
  - Explanation: Terminal default font, dimmed 80% opacity
  - Actions: Bold for keys, regular for labels
- **Animation:** Fade in over 200ms, scale from 0.95 to 1.0

### Pattern 2: Split View Mode (Alternative)

**Use Case:** When user needs more context or multiple suggestions

**Layout:**
```
┌─ Terminal ─────────────────────┐ ┌─ AI Assistant ──────────┐
│ > ? find all TODO comments    │ │ Command Options:        │
│                                │ │                          │
│                                │ │ 1. grep -r "TODO" .      │
│                                │ │    Simple recursive grep│
│                                │ │                          │
│                                │ │ 2. rg "TODO" -n -C 2     │
│                                │ │    Ripgrep with context │
│                                │ │                          │
│                                │ │ 3. git grep "TODO"       │
│                                │ │    Git-aware search     │
│                                │ │                          │
│                                │ │ [Tab] Cycle  [↵] Select │
└────────────────────────────────┘ └─────────────────────────┘
```

**Visual Specifications:**
- **Split Ratio:** 60/40 (terminal/assistant) - configurable
- **Divider:** Thin vertical line with drag handle
- **Highlight:** Selected option has subtle background highlight
- **Collapse:** Can minimize assistant pane with keyboard shortcut

### Pattern 3: REPL/Chat Mode

**Trigger:** Special command or keyboard shortcut to enter conversational mode

**Interface:**
```
╔═══════════════════════════════════════════════════════════╗
║ AI Command Assistant - Chat Mode                  [✕ Exit]║
╠═══════════════════════════════════════════════════════════╣
║ You: I need to backup all my project files               ║
║                                                            ║
║ AI: I can help with that. A few questions:                ║
║     • What directory contains your project files?         ║
║     • Where should the backup be stored?                  ║
║     • Do you want compression?                            ║
║                                                            ║
║ You: ~/projects to ~/backups with compression            ║
║                                                            ║
║ AI: Here's the command:                                   ║
║ ┌────────────────────────────────────────────────────────┐║
║ │ tar -czf ~/backups/projects-$(date +%Y%m%d).tar.gz \  │║
║ │ ~/projects                                             │║
║ └────────────────────────────────────────────────────────┘║
║     [Execute] [Revise] [Cancel]                           ║
╠═══════════════════════════════════════════════════════════╣
║ > _                                                        ║
╚═══════════════════════════════════════════════════════════╝
```

**Visual Specifications:**
- **Conversation Style:** Chat bubble aesthetic with alternating alignment
- **User messages:** Right-aligned, subtle background
- **AI messages:** Left-aligned, contrasting background
- **Commands:** Distinct box within AI messages
- **Timestamps:** Optional, shown on hover
- **Scroll behavior:** Auto-scroll to latest, preserve scroll on user interaction

---

## Visual Design Specifications

### Color System

#### Command Safety Levels

```rust
pub enum CommandSafety {
    Safe,        // Green accent: #50FA7B
    Neutral,     // Blue accent: #4A9EFF
    Caution,     // Orange accent: #FFB86C
    Dangerous,   // Red accent: #FF5555
}
```

**Examples:**
- **Safe:** `ls`, `pwd`, `echo`, `cat` (read-only operations)
- **Neutral:** `mkdir`, `touch`, `cp` (standard operations)
- **Caution:** `mv`, `chmod`, `chown` (modify file system)
- **Dangerous:** `rm -rf`, `sudo`, `dd`, `mkfs` (destructive operations)

#### Visual Indicators by Safety Level

| Level | Border Color | Icon | Confirm Required |
|-------|-------------|------|------------------|
| Safe | `#50FA7B` | ✓ | No |
| Neutral | `#4A9EFF` | ○ | Yes (single) |
| Caution | `#FFB86C` | ⚠ | Yes (double) |
| Dangerous | `#FF5555` | ⚠️ | Yes (typed confirmation) |

### Typography Hierarchy

```css
/* Command Text */
font-family: 'JetBrains Mono', 'Fira Code', monospace;
font-size: 14px;
line-height: 1.5;
font-weight: 500;

/* Explanation Text */
font-family: system-ui, -apple-system, sans-serif;
font-size: 13px;
line-height: 1.6;
font-weight: 400;
opacity: 0.8;

/* Action Labels */
font-family: system-ui, -apple-system, sans-serif;
font-size: 12px;
font-weight: 600;
letter-spacing: 0.02em;

/* Keyboard Shortcuts */
font-family: 'SF Pro', system-ui, sans-serif;
font-size: 11px;
font-weight: 700;
```

### Spacing & Layout

```
Padding:
- Container: 16px
- Between sections: 12px
- Between items: 8px
- Inline elements: 4px

Border Radius:
- Main container: 8px
- Inner elements: 4px
- Buttons: 6px

Shadows:
- Popup overlay: 0 4px 20px rgba(0,0,0,0.3)
- Hover elements: 0 2px 8px rgba(0,0,0,0.2)
```

### Animation Timing

```css
/* Fade In */
transition: opacity 200ms ease-in-out,
            transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);

/* Fade Out */
transition: opacity 150ms ease-in,
            transform 150ms ease-in;

/* Button Hover */
transition: background-color 100ms ease-out,
            transform 50ms ease-out;

/* Loading Indicator */
animation: spin 1s linear infinite;
```

### Icon System

**Status Icons:**
- ✓ Safe command
- ○ Neutral operation
- ⚠ Caution required
- ⚠️ Dangerous operation
- ⏳ Generating command
- ✗ Error occurred
- 💡 Suggestion available
- 📝 Edit mode
- 🔄 Revising command

**Action Icons:**
- ↵ Enter/Execute
- ⌘E Edit command
- ⌘R Revise with AI
- Esc Cancel
- Tab Cycle options
- ⌘? Help

---

## User Flows

### Flow 1: Simple Command Generation

```
User Input → AI Processing → Display Command → User Decision → Execute/Cancel

┌─────────────┐
│ User types  │
│ NL command  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Show        │
│ loading     │──→ Display "Generating..." indicator
│ indicator   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Query LLM   │──→ Background: Call llm_client.rs
│ via API     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Parse &     │
│ validate    │──→ Check for dangerous patterns
│ response    │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Display     │
│ command +   │──→ Show box with command, explanation, actions
│ explanation │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Wait for    │
│ user input  │
└──────┬──────┘
       │
       ├─→ [Enter] ──→ Execute command
       ├─→ [⌘E] ────→ Edit in terminal
       ├─→ [⌘R] ────→ Revise prompt
       └─→ [Esc] ───→ Cancel
```

**Timing Targets:**
- Loading indicator appears: <100ms
- LLM response received: <3s (avg), <10s (max)
- Display command: <50ms after response
- Total user-perceivable latency: <3.5s

### Flow 2: Command Revision

```
Initial Command → User Requests Revision → Show Revision Input → Generate New Command

┌─────────────┐
│ Command     │
│ displayed   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ User presses│
│ ⌘R (Revise) │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Show inline │
│ revision    │──→ "How should I modify this?"
│ input       │    [text input field]
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ User types  │
│ revision    │──→ "make it recursive"
│ request     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Send to LLM │
│ with context│──→ Include original command + revision request
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Display new │
│ command     │──→ Highlight what changed
│ with diff   │
└─────────────┘
```

**Visual Diff Display:**
```
╭─ Revised Command ─────────────────────────────────────────╮
│ find . -type f -name "*.log"                              │
│ [removed] └─ removed: current directory only              │
│                                                            │
│ find . -type f -name "*.log" -o -type l -name "*.log"    │
│ [added]   └─ added: recursive search including links      │
├───────────────────────────────────────────────────────────┤
│ Changes: Made search recursive with symbolic link support │
╰───────────────────────────────────────────────────────────╯
```

### Flow 3: Dangerous Command Warning

```
Dangerous Command Detected → Show Warning → Require Explicit Confirmation

┌─────────────┐
│ LLM returns │
│ command     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Safety      │
│ analysis    │──→ Detect: rm -rf, sudo, dd, etc.
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Display     │
│ warning UI  │
└─────────────┘

Warning UI:
╭─ ⚠️ DANGEROUS COMMAND DETECTED ───────────────────────────╮
│                                                            │
│ sudo rm -rf /var/log/*                                    │
│                                                            │
│ This command will:                                        │
│ • Run with elevated privileges (sudo)                     │
│ • Permanently delete files (rm -rf)                       │
│ • Affect system files (/var/log)                          │
│                                                            │
│ Type "I understand the risks" to proceed:                 │
│ > _                                                        │
│                                                            │
│ [Esc] Cancel                                              │
╰───────────────────────────────────────────────────────────╯
```

**Typed Confirmation Rules:**
- Must match exactly (case-sensitive)
- Cannot use keyboard shortcuts to bypass
- Optional: Add delay (e.g., 3 seconds) before allowing execution
- Log all dangerous command executions

### Flow 4: Error Handling

```
Command Execution → Error Occurs → Analyze Error → Suggest Fix

┌─────────────┐
│ Execute     │
│ command     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Command     │
│ fails       │──→ Capture stderr, exit code
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Display     │
│ error       │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Offer AI    │
│ analysis    │──→ "Analyze this error?" [Yes] [No]
└──────┬──────┘
       │
       ▼ [Yes]
┌─────────────┐
│ Send error  │
│ to LLM      │──→ Context: command + error + env
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ Display     │
│ diagnosis + │──→ Explain what went wrong + suggest fix
│ suggested   │
│ fix         │
└─────────────┘
```

**Error Display:**
```
╭─ ✗ Command Failed ────────────────────────────────────────╮
│ find /root -name "*.log"                                  │
│                                                            │
│ Error: Permission denied                                  │
│ Exit code: 1                                              │
├─ 💡 Suggestion ───────────────────────────────────────────┤
│ This command failed because you don't have permission     │
│ to access /root directory.                                │
│                                                            │
│ Try this instead:                                         │
│ sudo find /root -name "*.log"                             │
│                                                            │
│ Or search in a directory you have access to:              │
│ find ~ -name "*.log"                                      │
├───────────────────────────────────────────────────────────┤
│ [Try Suggested Fix] [Modify Original] [Cancel]            │
╰───────────────────────────────────────────────────────────╯
```

### Flow 5: Multi-Command Sequences

When LLM returns multiple commands:

```
╭─ AI Generated Commands (3 steps) ─────────────────────────╮
│                                                            │
│ Step 1/3: Create backup directory                         │
│ └─ mkdir -p ~/backups/$(date +%Y%m%d)                     │
│                                                            │
│ Step 2/3: Copy files                                      │
│ └─ cp -r ~/projects ~/backups/$(date +%Y%m%d)/           │
│                                                            │
│ Step 3/3: Verify backup                                   │
│ └─ ls -lh ~/backups/$(date +%Y%m%d)/                     │
├───────────────────────────────────────────────────────────┤
│ [Execute All] [Execute Step-by-Step] [Edit] [Cancel]     │
╰───────────────────────────────────────────────────────────╯
```

**Step-by-Step Execution:**
```
╭─ Executing Step 1/3 ──────────────────────────────────────╮
│ mkdir -p ~/backups/20251028                               │
│                                                            │
│ ✓ Success                                                 │
├───────────────────────────────────────────────────────────┤
│ [Continue to Step 2] [Stop Here] [Skip to Step 3]        │
╰───────────────────────────────────────────────────────────╯
```

---

## Safety & Error Handling

### Safety Classification System

Implement a multi-level safety classifier in Rust:

```rust
pub struct CommandSafetyAnalyzer {
    dangerous_patterns: Vec<Regex>,
    dangerous_flags: Vec<&'static str>,
    protected_paths: Vec<PathBuf>,
}

impl CommandSafetyAnalyzer {
    pub fn analyze(&self, command: &str) -> SafetyLevel {
        // Check for dangerous patterns
        if self.contains_dangerous_pattern(command) {
            return SafetyLevel::Dangerous;
        }

        // Check for dangerous flags
        if self.contains_dangerous_flags(command) {
            return SafetyLevel::Caution;
        }

        // Check if affects protected paths
        if self.affects_protected_path(command) {
            return SafetyLevel::Caution;
        }

        // Check for write operations
        if self.is_write_operation(command) {
            return SafetyLevel::Neutral;
        }

        // Default to safe for read-only operations
        SafetyLevel::Safe
    }
}
```

**Dangerous Patterns:**
- `rm -rf` or `rm -fr`
- `sudo rm`
- `dd if=` (disk operations)
- `mkfs`, `fdisk`, `parted`
- `chmod -R 777`
- `> /dev/sda` (redirect to disk)
- `:(){:|:&};:` (fork bomb)
- `curl ... | sh` or `wget ... | bash`

**Dangerous Flags:**
- `-f` (force) with destructive commands
- `-R` or `-r` (recursive) with rm, chmod
- `--no-preserve-root`

**Protected Paths:**
- `/` (root)
- `/etc`
- `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`
- `/boot`
- `/dev`
- `/proc`, `/sys`
- `~/.ssh`
- `~/.config`

### Error Message Design

**Principles:**
1. Explain what went wrong in plain language
2. Explain why it went wrong (if possible)
3. Suggest actionable fixes
4. Provide links to documentation (optional)

**Template:**
```
╭─ Error Type: [Error Name] ────────────────────────────────╮
│                                                            │
│ What happened:                                            │
│ [Plain language explanation]                              │
│                                                            │
│ Why it happened:                                          │
│ [Root cause analysis]                                     │
│                                                            │
│ How to fix:                                               │
│ [Specific actionable steps]                               │
│                                                            │
│ [Learn More] [Try Fix] [Cancel]                           │
╰───────────────────────────────────────────────────────────╯
```

**Example Error Messages:**

1. **Permission Denied:**
```
What happened: You don't have permission to access this file/directory

Why it happened: The file is owned by another user or requires elevated privileges

How to fix:
• Use 'sudo' to run with administrator privileges
• Check file permissions with 'ls -la'
• Contact your system administrator
```

2. **Command Not Found:**
```
What happened: The command 'xyz' is not installed on your system

Why it happened: This tool is not in your PATH or not installed

How to fix:
• Install it: sudo apt install xyz (Ubuntu/Debian)
• Or: brew install xyz (macOS)
• Or: Use an alternative like [abc]
```

3. **Syntax Error:**
```
What happened: The command has invalid syntax

Why it happened: Missing quote, unclosed parenthesis, or invalid flag

How to fix:
• Check for matching quotes and brackets
• Verify flag syntax with 'man [command]'
• The error occurred near: [highlight problematic section]
```

### Undo/Rollback Features

For commands that modify the file system, offer undo capabilities:

```
╭─ Command Executed ────────────────────────────────────────╮
│ mv ~/documents/report.pdf ~/archive/                      │
│                                                            │
│ ✓ Success: Moved 1 file                                   │
├───────────────────────────────────────────────────────────┤
│ [Undo This Action] [Continue]                             │
╰───────────────────────────────────────────────────────────╯
```

**Undo Implementation Strategy:**
```rust
pub struct CommandHistory {
    entries: Vec<CommandEntry>,
}

pub struct CommandEntry {
    command: String,
    timestamp: SystemTime,
    undo_action: Option<UndoAction>,
}

pub enum UndoAction {
    ReverseMove { from: PathBuf, to: PathBuf },
    RestoreFile { path: PathBuf, backup: PathBuf },
    RemoveCreated { paths: Vec<PathBuf> },
    Custom { script: String },
}
```

**Supported Undo Operations:**
- `mv` → Move back to original location
- `cp` → Remove copied files
- `mkdir` → Remove created directory (if empty)
- `touch` → Remove created file
- `rm` → Restore from backup (if backup exists)

---

## Implementation Recommendations

### Phase 1: Core UI (MVP)

**Deliverables:**
1. Inline suggestion mode with basic styling
2. Simple confirmation (Enter/Cancel)
3. Loading indicator
4. Basic error display

**Technical Stack:**
- Rendering: Use terminal UI library (e.g., `ratatui` for Rust)
- Styling: ANSI color codes + box drawing characters
- Input handling: Raw mode terminal input

**Implementation Steps:**
```rust
// 1. Terminal UI setup
pub struct NLCommandUI {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    state: UIState,
}

// 2. State management
pub enum UIState {
    Idle,
    InputCapture { buffer: String },
    Generating { query: String },
    DisplayingCommand { command: GeneratedCommand },
    Error { message: String },
}

// 3. Render pipeline
impl NLCommandUI {
    pub fn render(&mut self) -> Result<()> {
        self.terminal.draw(|f| {
            match &self.state {
                UIState::Generating { query } => {
                    self.render_loading(f, query);
                }
                UIState::DisplayingCommand { command } => {
                    self.render_command_box(f, command);
                }
                // ... other states
            }
        })?;
        Ok(())
    }
}
```

### Phase 2: Enhanced Interaction

**Deliverables:**
1. Revision flow
2. Safety classification
3. Dangerous command warnings
4. Edit mode integration
5. Multi-command support

**Key Features:**
- Command editing in terminal buffer
- Diff display for revisions
- Typed confirmation for dangerous commands
- Step-by-step execution for sequences

### Phase 3: Advanced Features

**Deliverables:**
1. REPL/Chat mode
2. Error analysis with AI
3. Command history with search
4. Undo/rollback system
5. Custom shortcuts
6. Configuration UI

**Advanced Capabilities:**
- Conversational command refinement
- Learning from user corrections
- Predictive command suggestions
- Integration with shell history

### Phase 4: Polish & Optimization

**Deliverables:**
1. Animations and transitions
2. Accessibility features
3. Customizable themes
4. Performance optimizations
5. Analytics and telemetry

**Quality Improvements:**
- Smooth animations (fade, slide)
- Screen reader support
- High contrast mode
- Reduced motion mode
- Smart caching of LLM responses

---

## UI Components Library

### Component: CommandBox

**Purpose:** Primary container for displaying generated commands

**Props:**
```rust
pub struct CommandBoxProps {
    command: String,
    explanation: Vec<String>,
    safety_level: SafetyLevel,
    actions: Vec<Action>,
    show_explanation: bool,
}
```

**Visual States:**
- Normal (default)
- Hovered (action highlighted)
- Loading (animated dots)
- Error (red border, shake animation)

### Component: ConfirmationDialog

**Purpose:** User confirmation before execution

**Props:**
```rust
pub struct ConfirmationDialogProps {
    message: String,
    actions: Vec<Action>,
    default_action: usize,
    dangerous: bool,
}
```

**Variants:**
- Simple (Yes/No)
- Typed (requires text input)
- Delayed (countdown timer)

### Component: LoadingIndicator

**Purpose:** Show AI processing status

**Variants:**
- Spinner (rotating)
- Dots (pulsing)
- Progress bar (with estimated time)

**Animation:**
```
⠋ Generating command...
⠙ Generating command...
⠹ Generating command...
⠸ Generating command...
⠼ Generating command...
```

### Component: ErrorMessage

**Purpose:** Display errors with context

**Props:**
```rust
pub struct ErrorMessageProps {
    error_type: ErrorType,
    message: String,
    suggestions: Vec<String>,
    actions: Vec<Action>,
}
```

### Component: DiffView

**Purpose:** Show changes between command versions

**Format:**
```
- original line
+ new line
  unchanged line
```

**Colors:**
- Removed: Red background
- Added: Green background
- Unchanged: Default

---

## Keyboard Shortcuts

### Global Shortcuts

| Shortcut | Action | Context |
|----------|--------|---------|
| `?` (prefix) | Trigger NL mode | Terminal input |
| `Ctrl+Space` | Open AI assistant | Anywhere |
| `Esc` | Cancel/Close | Any dialog |
| `Ctrl+C` | Abort generation | While generating |

### Command Display Shortcuts

| Shortcut | Action |
|----------|--------|
| `Enter` or `↵` | Execute command |
| `⌘E` or `Ctrl+E` | Edit command in buffer |
| `⌘R` or `Ctrl+R` | Revise with AI |
| `Tab` | Cycle through options |
| `⌘/` or `Ctrl+/` | Toggle explanation |
| `⌘C` or `Ctrl+Shift+C` | Copy command |

### REPL Mode Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+L` | Clear conversation |
| `Ctrl+P` | Previous message |
| `Ctrl+N` | Next message |
| `Ctrl+U` | Clear input line |
| `Ctrl+D` | Exit REPL mode |

---

## Accessibility Considerations

### Screen Reader Support

1. **Announce state changes**
   - "Generating command, please wait"
   - "Command generated: [command]"
   - "Warning: dangerous operation detected"

2. **Semantic structure**
   - Use proper heading hierarchy
   - Label all interactive elements
   - Provide alternative text for icons

3. **Keyboard navigation**
   - All features accessible via keyboard
   - Logical tab order
   - Visible focus indicators

### High Contrast Mode

Automatically detect system preference and adjust:
- Increase border width from 1px to 2px
- Use solid colors instead of gradients
- Higher contrast ratios (7:1 minimum)
- Thicker text weights

### Reduced Motion

When user prefers reduced motion:
- Disable fade animations
- Disable scale transforms
- Use instant state changes
- Keep loading spinners (essential feedback)

### Font Scaling

Support different terminal font sizes:
- Proportional spacing (use em/rem units)
- Responsive layout (adjust box width)
- Minimum touch target size: 44x44px equivalent

---

## Configuration & Customization

### User Preferences File

```toml
# ~/.config/saternal/nl_commands.toml

[ui]
theme = "auto"  # auto, light, dark
animation = true
show_explanations = true
confirm_before_execute = true

[shortcuts]
trigger = "?"
edit = "Ctrl+E"
revise = "Ctrl+R"
cancel = "Esc"

[safety]
warn_on_dangerous = true
require_typed_confirmation = true
backup_before_delete = true

[ai]
model = "anthropic/claude-3.5-sonnet"
temperature = 0.2
max_tokens = 512
cache_enabled = true
cache_ttl = 3600  # seconds

[display]
box_style = "rounded"  # rounded, square, minimal
border_color = "auto"  # auto, blue, green, purple
max_width = 80  # characters
```

### Theme System

Support custom themes:

```toml
# ~/.config/saternal/themes/ocean.toml
[theme.ocean]
name = "Ocean"
author = "Saternal"

[colors]
background = "#1e2030"
foreground = "#e0e0e0"
border_safe = "#56d364"
border_neutral = "#58a6ff"
border_caution = "#ffa657"
border_dangerous = "#f85149"
accent = "#79c0ff"

[styles]
border_width = 2
border_radius = 8
shadow = "0 4px 20px rgba(0,0,0,0.5)"
```

### Plugin System (Future)

Allow custom command processors:

```rust
pub trait CommandProcessor {
    fn name(&self) -> &str;
    fn process(&self, nl: &str, context: &Context) -> Result<Vec<String>>;
    fn supports(&self, nl: &str) -> bool;
}

// Example: Git-specific processor
pub struct GitCommandProcessor;

impl CommandProcessor for GitCommandProcessor {
    fn supports(&self, nl: &str) -> bool {
        nl.contains("git") || nl.contains("commit") || nl.contains("push")
    }

    fn process(&self, nl: &str, context: &Context) -> Result<Vec<String>> {
        // Custom git command generation logic
    }
}
```

---

## Performance Targets

### Latency Budgets

| Operation | Target | Maximum |
|-----------|--------|---------|
| UI render | 16ms (60fps) | 33ms (30fps) |
| Input response | 50ms | 100ms |
| LLM query | 2s | 10s |
| Command display | 50ms | 100ms |
| Error display | 50ms | 100ms |

### Memory Usage

- Base UI: <10MB
- Per command: <1MB
- Cache: <50MB (configurable)
- Total: <100MB typical

### Network

- LLM API call: <100KB request, <500KB response
- Retry strategy: 3 attempts with exponential backoff
- Timeout: 10s per request, 30s total

---

## Testing Strategy

### Unit Tests

1. Command parser
2. Safety analyzer
3. Error message generator
4. Diff generator

### Integration Tests

1. End-to-end command flow
2. Error handling paths
3. Revision workflow
4. Multi-command execution

### UI Tests

1. Render consistency
2. Keyboard navigation
3. Screen reader compatibility
4. Theme rendering

### User Testing

1. **Beginner users:** Can they understand and use the feature?
2. **Advanced users:** Is it fast enough? Too much friction?
3. **Accessibility:** Works with screen readers, high contrast, etc.?

**Test Scenarios:**
- "Find all large files in my home directory"
- "Backup my projects folder to external drive"
- "Fix permissions on a directory"
- "Create a new user account"
- "Delete old log files"

---

## Metrics & Analytics

### Usage Metrics

- Commands generated per day/week/month
- Acceptance rate (executed vs. cancelled)
- Revision rate (how often users revise)
- Error rate (failed executions)
- Safety warnings shown

### Performance Metrics

- LLM response time (p50, p95, p99)
- UI render time
- Cache hit rate
- Memory usage

### Quality Metrics

- Command accuracy (did it do what user wanted?)
- Safety incidents (dangerous commands executed)
- User satisfaction (survey/feedback)

---

## Future Enhancements

### Voice Input

Allow voice commands via speech-to-text:
```
> 🎤 [Recording] "find all my photos from last year"
```

### Command Templates

Save and reuse common command patterns:
```
> ? backup my projects  [using template: daily-backup]
```

### Multi-Agent Collaboration

Different AI models for different command types:
- Git commands → GitHub Copilot
- System admin → Specialized sysadmin model
- Data processing → Data science model

### Smart Suggestions

Learn from user behavior:
```
╭─ 💡 Suggestion ───────────────────────────────────────────╮
│ You often run 'git status' after 'git commit'            │
│ Want to do that now?                                      │
│ [Yes] [No] [Always]                                       │
╰───────────────────────────────────────────────────────────╯
```

### Cross-Platform Sync

Sync command history, preferences across devices via cloud.

### Integration with IDEs

VS Code extension, IntelliJ plugin for embedded terminal NL commands.

---

## Conclusion

This proposal outlines a comprehensive UI/UX design for Saternal's natural language command feature, drawing from best practices across the industry while maintaining a focus on safety, transparency, and user delight.

**Key Takeaways:**

1. **Safety First:** Multi-level safety classification with explicit confirmations for dangerous operations
2. **Progressive Disclosure:** Simple by default, powerful when needed
3. **Transparency:** Always show what will be executed and why
4. **Keyboard-First:** Efficient shortcuts for power users
5. **Contextual Intelligence:** Adapt to user's environment and expertise

**Next Steps:**

1. Review this proposal with the team
2. Create interactive prototypes for key flows
3. Implement Phase 1 (Core UI MVP)
4. Conduct user testing with early adopters
5. Iterate based on feedback

**References:**

- Warp Terminal: https://www.warp.dev/
- ai-shell: https://github.com/BuilderIO/ai-shell
- PromptShell: https://github.com/Kirti-Rathi/PromptShell
- Spren: https://github.com/smadgulkar/spren-ai-terminal-assistant-rust
- ShellGPT: https://github.com/TheR1D/shell_gpt
- uwu: https://github.com/context-labs/uwu

---

**Document Version:** 1.0
**Last Updated:** October 28, 2025
**Author:** Claude + Sam (Saternal Team)
