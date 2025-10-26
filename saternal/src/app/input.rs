use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Column;
use log::info;
use parking_lot::Mutex;
use saternal_core::{
    Config, InputModifiers, Renderer, SearchState, SelectionManager, SplitDirection,
    is_jump_to_bottom, key_to_bytes,
};
use saternal_macos::DropdownWindow;
use std::sync::Arc;
use winit::{
    event::{ElementState, KeyEvent, Modifiers},
    keyboard::{Key, KeyCode, PhysicalKey},
};

/// Handle keyboard input events
pub(super) fn handle_keyboard_input(
    event: &KeyEvent,
    state: ElementState,
    modifiers_state: &Modifiers,
    renderer: &Arc<Mutex<Renderer>>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    selection_manager: &mut SelectionManager,
    search_state: &mut SearchState,
    config: &mut Config,
    font_size: &mut f32,
    window: &winit::window::Window,
    dropdown: &Arc<Mutex<DropdownWindow>>,
) -> bool {
    if state != ElementState::Pressed {
        return false;
    }

    let cmd = modifiers_state.state().super_key();
    let shift = modifiers_state.state().shift_key();
    let ctrl = modifiers_state.state().control_key();

    // Handle Escape key
    if matches!(event.logical_key, Key::Named(winit::keyboard::NamedKey::Escape)) {
        return handle_escape(search_state, selection_manager, renderer, tab_manager);
    }

    // Pane navigation removed from Ctrl+Tab (conflicts with system shortcuts)
    // Now handled by Cmd+Shift+[ and Cmd+Shift+] below

    // Handle Cmd shortcuts
    if cmd {
        return handle_cmd_shortcuts(
            event,
            shift,
            tab_manager,
            selection_manager,
            search_state,
            config,
            font_size,
            renderer,
            window,
        );
    }

    // Handle Ctrl shortcuts (split pane operations)
    if ctrl {
        if let PhysicalKey::Code(keycode) = event.physical_key {
            if handle_ctrl_shortcuts(keycode, tab_manager, config, window) {
                return true;
            }
        }
    }

    // Handle terminal input
    handle_terminal_input(event, modifiers_state, tab_manager, renderer, window, dropdown)
}

fn handle_escape(
    search_state: &mut SearchState,
    selection_manager: &mut SelectionManager,
    renderer: &Arc<Mutex<Renderer>>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) -> bool {
    if search_state.is_active() {
        search_state.deactivate();
        info!("Search deactivated");
        return true;
    }
    if selection_manager.range().is_some() {
        selection_manager.clear();
        let (grid_cols, grid_lines) = super::mouse::get_grid_dimensions(tab_manager);
        renderer.lock().update_selection(None, grid_cols, grid_lines);
        info!("Selection cleared");
        return true;
    }
    false
}

fn handle_pane_navigation(
    previous: bool,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    window: &winit::window::Window,
) -> bool {
    if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
        if previous {
            active_tab.pane_tree.focus_prev();
            info!("Focus moved to previous pane (Cmd+Shift+[)");
        } else {
            active_tab.pane_tree.focus_next();
            info!("Focus moved to next pane (Cmd+Shift+])");
        }
        window.request_redraw();
    }
    true
}

fn handle_cmd_shortcuts(
    event: &KeyEvent,
    shift: bool,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    selection_manager: &mut SelectionManager,
    search_state: &mut SearchState,
    config: &mut Config,
    font_size: &mut f32,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
) -> bool {
    if let PhysicalKey::Code(keycode) = event.physical_key {
        match keycode {
            KeyCode::KeyC => {
                super::clipboard::handle_copy(tab_manager, selection_manager);
                return true;
            }
            KeyCode::KeyV => {
                super::clipboard::handle_paste(tab_manager, renderer, window);
                return true;
            }
            KeyCode::KeyF => {
                info!("Search activated (Cmd+F)");
                search_state.activate();
                return true;
            }
            KeyCode::KeyG => {
                return handle_search_navigation(shift, search_state, tab_manager);
            }
            KeyCode::BracketLeft => {
                // Cmd+Shift+[ - Navigate to previous pane
                if shift {
                    return handle_pane_navigation(true, tab_manager, window);
                }
            }
            KeyCode::BracketRight => {
                // Cmd+Shift+] - Navigate to next pane
                if shift {
                    return handle_pane_navigation(false, tab_manager, window);
                }
            }
            KeyCode::KeyD => {
                info!("Splitting pane vertically (Cmd+D) - side by side");
                if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                    if let Err(e) = active_tab.split(
                        SplitDirection::Vertical,
                        Some(config.terminal.shell.clone())
                    ) {
                        log::error!("Failed to split pane: {}", e);
                    }
                }
                window.request_redraw();
                return true;
            }
            _ => {}
        }
    }

    // Font size adjustment
    handle_font_size_shortcuts(event, config, font_size, renderer)
}

fn handle_search_navigation(
    shift: bool,
    search_state: &mut SearchState,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) -> bool {
    if !search_state.is_active() {
        return false;
    }

    if let Some(tab_mgr) = tab_manager.try_lock() {
        if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
            if let Some(term_lock) = pane.terminal.term().try_lock() {
                let result = if shift {
                    search_state.prev_match(&term_lock.grid())
                } else {
                    search_state.next_match(&term_lock.grid())
                };
                
                if let Some(match_point) = result {
                    info!("Found match at {:?}", match_point);
                }
            }
        }
    }
    true
}

fn handle_font_size_shortcuts(
    event: &KeyEvent,
    config: &mut Config,
    font_size: &mut f32,
    renderer: &Arc<Mutex<Renderer>>,
) -> bool {
    let key_text = match &event.logical_key {
        Key::Character(s) => Some(s.as_str()),
        _ => None,
    };
    
    let should_increase_font = match key_text {
        Some("=" | "+") => true,
        _ => {
            if let PhysicalKey::Code(KeyCode::Equal) = event.physical_key {
                true
            } else {
                false
            }
        }
    };
    
    if should_increase_font {
        *font_size = (*font_size + 2.0).min(48.0);
        info!("Increased font size to {}", font_size);
        update_font_size(config, *font_size, renderer);
        return true;
    } else if let Some(key_text) = key_text {
        match key_text {
            "-" => {
                *font_size = (*font_size - 2.0).max(8.0);
                info!("Decreased font size to {}", font_size);
                update_font_size(config, *font_size, renderer);
                return true;
            }
            "0" => {
                *font_size = 14.0;
                info!("Reset font size to default (14.0)");
                update_font_size(config, *font_size, renderer);
                return true;
            }
            _ => {}
        }
    }
    
    true
}

fn update_font_size(config: &mut Config, font_size: f32, renderer: &Arc<Mutex<Renderer>>) {
    config.appearance.font_size = font_size;
    let _ = config.save(None);
    if let Some(mut renderer) = renderer.try_lock() {
        if let Err(e) = renderer.set_font_size(font_size) {
            log::error!("Failed to update font size: {}", e);
        }
    }
}

fn handle_ctrl_shortcuts(
    keycode: KeyCode,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    _config: &Config,
    window: &winit::window::Window,
) -> bool {
    match keycode {
        KeyCode::KeyW => {
            info!("Closing focused pane");
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                if let Err(e) = active_tab.close_focused_pane() {
                    log::error!("Failed to close pane: {}", e);
                }
            }
            window.request_redraw();
            true
        }
        _ => false,
    }
}

/// Fast inline function to read the current line from terminal grid
#[inline]
fn read_current_line_from_grid(tab_manager: &Arc<Mutex<crate::tab::TabManager>>) -> Option<String> {
    let tab_mgr = tab_manager.lock();
    let active_tab = tab_mgr.active_tab()?;
    let pane = active_tab.pane_tree.focused_pane()?;

    // Extend lifetime by binding the Arc first
    let term_arc = pane.terminal.term();
    let term_lock = term_arc.try_lock()?;

    let grid = term_lock.grid();
    let cursor_line = grid.cursor.point.line;

    // Pre-allocate with reasonable capacity (most commands < 256 chars)
    let mut line = String::with_capacity(256);

    // Fast iteration over grid cells - zero-copy char extraction
    let num_cols = grid.columns();
    for col_idx in 0..num_cols {
        let column = Column(col_idx);
        let cell = &grid[cursor_line][column];
        let ch = cell.c;

        // Early termination on null/empty
        if ch == '\0' || ch == ' ' && line.is_empty() {
            continue;
        }
        line.push(ch);
    }

    Some(line.trim_end().to_string())
}

fn handle_terminal_input(
    event: &KeyEvent,
    modifiers_state: &Modifiers,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
    dropdown: &Arc<Mutex<DropdownWindow>>,
) -> bool {
    let input_mods = InputModifiers::from_winit(modifiers_state.state());

    // Check for jump to bottom shortcut
    if let PhysicalKey::Code(keycode) = event.physical_key {
        if is_jump_to_bottom(&event.logical_key, keycode, input_mods) {
            info!("Jump to bottom triggered");
            renderer.lock().reset_scroll();
            window.request_redraw();
            return true;
        }
    }

    // Try to convert key to terminal bytes
    if let PhysicalKey::Code(keycode) = event.physical_key {
        if let Some(bytes) = key_to_bytes(&event.logical_key, keycode, input_mods) {
            // Check for Enter key - intercept to detect commands
            if bytes == b"\r" || bytes == b"\n" {
                // Read current line from grid (captures typed + autocompleted + pasted text)
                if let Some(line) = read_current_line_from_grid(tab_manager) {
                    log::debug!("Enter pressed - checking for command (line length: {})", line.len());

                    // Check if it's a terminal command
                    if let Some(cmd) = crate::app::commands::parse_command(&line) {
                        let cmd_name = get_command_name(&cmd);
                        log::info!("✓ Command detected: {}", cmd_name);

                        // Execute command
                        let success = execute_command(cmd, renderer, window, dropdown);

                        if success {
                            log::info!("✓ Command executed successfully");
                            // Don't pass Enter to shell - command was consumed
                            return true;
                        } else {
                            log::warn!("⚠️ Command execution failed");
                            return true;
                        }
                    }
                }
                // Not a command - fall through to pass Enter to terminal
            }

            // Pass to terminal (including Enter if not a command)
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                let _ = active_tab.write_input(&bytes);
            }
            renderer.lock().reset_scroll();
            window.request_redraw();
            return true;
        }
    }

    // Handle regular text input
    if !input_mods.ctrl && !input_mods.alt {
        if let Some(text) = &event.text {
            // Pass to terminal
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                let _ = active_tab.write_input(text.as_bytes());
            }
            renderer.lock().reset_scroll();
            window.request_redraw();
        }
    }

    false
}

/// Get sanitized command name without user data
fn get_command_name(cmd: &crate::app::commands::TerminalCommand) -> &'static str {
    use crate::app::commands::TerminalCommand;
    match cmd {
        TerminalCommand::Wallpaper { .. } => "Wallpaper",
        TerminalCommand::WallpaperOpacity { .. } => "WallpaperOpacity",
        TerminalCommand::BackgroundOpacity { .. } => "BackgroundOpacity",
        TerminalCommand::BlurStrength { .. } => "BlurStrength",
    }
}

/// Execute a terminal command
fn execute_command(
    cmd: crate::app::commands::TerminalCommand,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
    dropdown: &Arc<Mutex<DropdownWindow>>,
) -> bool {
    use crate::app::commands::TerminalCommand;

    let result = match &cmd {
        TerminalCommand::Wallpaper { path } => {
            renderer.lock().set_wallpaper(path.as_deref())
        }
        TerminalCommand::WallpaperOpacity { opacity } => {
            renderer.lock().set_wallpaper_opacity(*opacity);
            Ok(())
        }
        TerminalCommand::BackgroundOpacity { opacity } => {
            renderer.lock().set_background_opacity(*opacity);
            Ok(())
        }
        TerminalCommand::BlurStrength { strength } => {
            renderer.lock().set_blur_strength(*strength);
            Ok(())
        }
    };

    let success = result.is_ok();
    let _message = match result {
        Ok(_) => crate::app::commands::format_success_message(&cmd),
        Err(e) => crate::app::commands::format_error_message(&cmd, &e.to_string()),
    };

    // Note: In a real implementation, we'd display the message in the terminal
    // For now, the log messages in the renderer are sufficient

    window.request_redraw();
    success
}
