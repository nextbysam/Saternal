use log::info;
use parking_lot::Mutex;
use saternal_core::{
    Config, InputModifiers, Renderer, SearchState, SelectionManager, SplitDirection,
    is_jump_to_bottom, key_to_bytes,
};
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

    // Handle Tab navigation
    if matches!(event.logical_key, Key::Named(winit::keyboard::NamedKey::Tab)) {
        if ctrl {
            return handle_tab_navigation(shift, tab_manager, window);
        }
    }

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
    handle_terminal_input(event, modifiers_state, tab_manager, renderer, window)
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

fn handle_tab_navigation(
    shift: bool,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    window: &winit::window::Window,
) -> bool {
    if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
        if shift {
            active_tab.pane_tree.focus_prev();
            info!("Focus moved to previous pane");
        } else {
            active_tab.pane_tree.focus_next();
            info!("Focus moved to next pane");
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
) -> bool {
    if let PhysicalKey::Code(keycode) = event.physical_key {
        match keycode {
            KeyCode::KeyC => {
                super::clipboard::handle_copy(tab_manager, selection_manager);
                return true;
            }
            KeyCode::KeyV => {
                super::clipboard::handle_paste(tab_manager);
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
    config: &Config,
    window: &winit::window::Window,
) -> bool {
    match keycode {
        KeyCode::KeyD => {
            info!("Splitting pane horizontally");
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                if let Err(e) = active_tab.split(
                    SplitDirection::Horizontal,
                    Some(config.terminal.shell.clone())
                ) {
                    log::error!("Failed to split pane: {}", e);
                }
            }
            window.request_redraw();
            true
        }
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

fn handle_terminal_input(
    event: &KeyEvent,
    modifiers_state: &Modifiers,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
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
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                let _ = active_tab.write_input(&bytes);
            }
            return true;
        }
    }

    // Handle regular text input
    if !input_mods.ctrl && !input_mods.alt {
        if let Some(text) = &event.text {
            if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                let _ = active_tab.write_input(text.as_bytes());
            }
        }
    }

    false
}
