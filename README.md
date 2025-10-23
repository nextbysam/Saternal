# Saternal

A blazing-fast dropdown terminal emulator for macOS, built with Rust.

## Features

- **Global Hotkey**: Toggle the terminal from anywhere with `Cmd+\``
- **Dropdown Animation**: Smooth slide-down/slide-up animations
- **GPU Accelerated**: Metal-powered rendering for maximum performance
- **Tabs**: Multiple terminal sessions in tabs
- **Split Panes**: Horizontal and vertical pane splitting
- **Based on Alacritty**: Proven terminal emulation core

## Installation

### Prerequisites

- macOS 11.0 or later
- Rust toolchain (1.70+)

### Build from Source

```bash
git clone https://github.com/yourusername/saternal.git
cd saternal
cargo build --release
```

The binary will be at `target/release/saternal`.

### Run

```bash
cargo run --release
```

Or run the binary directly:

```bash
./target/release/saternal
```

## Configuration

Saternal looks for configuration at `~/.config/saternal/config.toml`. On first run, a default configuration will be created.

Example configuration:

```toml
[window]
width_percentage = 1.0      # Full screen width
height_percentage = 0.5     # Half screen height
animation_duration_ms = 180 # Animation speed

[hotkey]
toggle = "cmd+`"            # Global hotkey

[appearance]
theme = "tokyo-night"
font_family = "JetBrains Mono"
font_size = 14.0
opacity = 0.95
blur = true

[terminal]
shell = "/bin/zsh"
scrollback_lines = 10000
ligatures = true
```

## Usage

### Global Hotkey

Press `Cmd+\`` from anywhere to toggle the terminal.

### Keyboard Shortcuts

#### Tabs
- `Cmd+T` - New tab
- `Cmd+W` - Close tab
- `Cmd+1-9` - Switch to tab 1-9
- `Cmd+Shift+[` - Previous tab
- `Cmd+Shift+]` - Next tab

#### Panes
- `Cmd+D` - Split pane vertically
- `Cmd+Shift+D` - Split pane horizontally
- `Cmd+H/J/K/L` - Navigate between panes (vim-style)
- `Cmd+Ctrl+H/J/K/L` - Resize panes

## Architecture

Saternal is organized as a Rust workspace with three crates:

- **saternal**: Main application binary
- **saternal-core**: Terminal emulation, rendering, and configuration
- **saternal-macos**: macOS-specific window management and hotkeys

### Tech Stack

- **Terminal Emulation**: `alacritty_terminal` - Battle-tested VTE parser and PTY handling
- **GPU Rendering**: `wgpu` with Metal backend for blazing-fast text rendering
- **Window Management**: `winit` + `cocoa` for native macOS window behavior
- **Global Hotkeys**: `global-hotkey` for system-wide keyboard shortcuts
- **Font Rendering**: `fontdue` for high-quality glyph rasterization

## Development

### Project Structure

```
saternal/
├── saternal/              # Main application
│   ├── src/
│   │   ├── main.rs       # Entry point
│   │   ├── app.rs        # Application state and event loop
│   │   └── tab.rs        # Tab management
│   └── resources/
│       └── macos/        # macOS bundle resources
├── saternal-core/         # Core terminal functionality
│   └── src/
│       ├── config.rs     # Configuration management
│       ├── font.rs       # Font loading and glyph cache
│       ├── pane.rs       # Pane splitting and layout
│       ├── renderer.rs   # GPU-accelerated rendering
│       └── terminal.rs   # Terminal emulation wrapper
└── saternal-macos/        # macOS platform code
    └── src/
        ├── hotkey.rs     # Global hotkey registration
        └── window.rs     # Dropdown window behavior
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code without building
cargo check
```

### TODO

- [ ] Complete terminal rendering implementation
- [ ] Add proper tab UI rendering
- [ ] Implement pane focus visual indicators
- [ ] Add theme support
- [ ] Implement configuration hot-reload
- [ ] Add search functionality
- [ ] Performance optimizations
- [ ] macOS .app bundle packaging
- [ ] CI/CD pipeline

## Performance Goals

- **Startup time**: <100ms (cold start)
- **Toggle latency**: <200ms (hotkey press → visible)
- **Frame rate**: 60fps sustained during scrolling
- **Memory**: <100MB with 5 tabs, 10 panes total

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

- [Alacritty](https://github.com/alacritty/alacritty) - For the excellent terminal emulation core
- [Ghostty](https://ghostty.org/) - Inspiration for the dropdown terminal concept
- [WezTerm](https://wezfurlong.org/wezterm/) - Inspiration for GPU-accelerated rendering

---

Made with ❤️ and Rust
