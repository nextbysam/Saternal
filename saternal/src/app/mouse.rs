use alacritty_terminal::grid::Dimensions;
use log::info;
use parking_lot::Mutex;
use saternal_core::{MouseButton, MouseState, Renderer, SelectionManager, SelectionMode};
use std::sync::Arc;
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta};

/// Handle mouse button events
pub(super) fn handle_mouse_input(
    state: ElementState,
    button: WinitMouseButton,
    mouse_state: &mut MouseState,
    selection_manager: &mut SelectionManager,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
) {
    let mouse_button = match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        _ => return,
    };

    match state {
        ElementState::Pressed => {
            handle_mouse_press(mouse_button, mouse_state, selection_manager, tab_manager, renderer);
        }
        ElementState::Released => {
            handle_mouse_release(mouse_button, mouse_state, selection_manager, tab_manager);
        }
    }
}

fn handle_mouse_press(
    mouse_button: MouseButton,
    mouse_state: &mut MouseState,
    selection_manager: &mut SelectionManager,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
) {
    mouse_state.press_button(mouse_button);
    
    if mouse_button != MouseButton::Left {
        return;
    }

    let mode = match mouse_state.click_count {
        1 => SelectionMode::Normal,
        2 => SelectionMode::Word,
        3 => SelectionMode::Line,
        _ => SelectionMode::Normal,
    };
    
    if mouse_state.click_count == 1 {
        selection_manager.start(mouse_state.position, mode);
    } else if mouse_state.click_count == 2 {
        handle_double_click(selection_manager, mouse_state, tab_manager, renderer);
    } else if mouse_state.click_count == 3 {
        handle_triple_click(selection_manager, mouse_state, tab_manager, renderer);
    }
}

fn handle_double_click(
    selection_manager: &mut SelectionManager,
    mouse_state: &MouseState,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
) {
    if let Some(tab_mgr) = tab_manager.try_lock() {
        if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
            if let Some(term_lock) = pane.terminal.term().try_lock() {
                let grid = term_lock.grid();
                let grid_cols = grid.columns();
                let grid_lines = grid.screen_lines();
                selection_manager.expand_word(grid, mouse_state.position);
                drop(term_lock);
                renderer.lock().update_selection(selection_manager.range(), grid_cols, grid_lines);
            }
        }
    }
}

fn handle_triple_click(
    selection_manager: &mut SelectionManager,
    mouse_state: &MouseState,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<Renderer>>,
) {
    if let Some(tab_mgr) = tab_manager.try_lock() {
        if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
            if let Some(term_lock) = pane.terminal.term().try_lock() {
                let grid = term_lock.grid();
                let grid_cols = grid.columns();
                let grid_lines = grid.screen_lines();
                selection_manager.expand_line(grid, mouse_state.position);
                drop(term_lock);
                renderer.lock().update_selection(selection_manager.range(), grid_cols, grid_lines);
            }
        }
    }
}

fn handle_mouse_release(
    mouse_button: MouseButton,
    mouse_state: &mut MouseState,
    selection_manager: &mut SelectionManager,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) {
    if mouse_button == MouseButton::Left && selection_manager.is_active() {
        if let Some(tab_mgr) = tab_manager.try_lock() {
            if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
                if let Some(term_lock) = pane.terminal.term().try_lock() {
                    let _ = selection_manager.finalize(&term_lock.grid());
                }
            }
        }
    }
    mouse_state.release_button();
}

/// Handle cursor movement
pub(super) fn handle_cursor_moved(
    x: f32,
    y: f32,
    mouse_state: &mut MouseState,
    selection_manager: &mut SelectionManager,
    renderer: &Arc<Mutex<Renderer>>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
) {
    if let Some(mut renderer_lock) = renderer.try_lock() {
        let fm = renderer_lock.font_manager();
        let effective_size = fm.effective_font_size();
        let line_metrics = fm.font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = fm.font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        
        mouse_state.update_position(x, y, cell_width, cell_height);
        
        if mouse_state.is_dragging() && selection_manager.is_active() {
            selection_manager.update(mouse_state.position);
            drop(renderer_lock);
            
            let (grid_cols, grid_lines) = get_grid_dimensions(tab_manager);
            renderer.lock().update_selection(selection_manager.range(), grid_cols, grid_lines);
        }
    }
}

pub(super) fn get_grid_dimensions(tab_manager: &Arc<Mutex<crate::tab::TabManager>>) -> (usize, usize) {
    if let Some(tab_mgr) = tab_manager.try_lock() {
        if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
            if let Some(term_lock) = pane.terminal.term().try_lock() {
                let grid = term_lock.grid();
                return (grid.columns(), grid.screen_lines());
            }
        }
    }
    (80, 24)
}

/// Handle mouse wheel scrolling
pub(super) fn handle_mouse_wheel(
    delta: MouseScrollDelta,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
) {
    let scroll_delta = match delta {
        MouseScrollDelta::LineDelta(_x, y) => y * 3.0,
        MouseScrollDelta::PixelDelta(pos) => (pos.y / 18.0) as f32,
    };

    if scroll_delta.abs() > 0.001 {
        renderer.lock().scroll(scroll_delta);
        window.request_redraw();
    }
}
