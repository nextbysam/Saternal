use log::info;
use parking_lot::Mutex;
use saternal_core::{Clipboard, SelectionManager};
use std::sync::Arc;

/// Handle copy operation (Cmd+C)
pub(super) fn handle_copy(
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    selection_manager: &mut SelectionManager,
) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            log::error!("Failed to create clipboard: {}", e);
            return;
        }
    };

    if let Some(tab_mgr) = tab_manager.try_lock() {
        if let Some(pane) = tab_mgr.active_tab().and_then(|tab| tab.pane_tree.focused_pane()) {
            if let Some(term_lock) = pane.terminal.term().try_lock() {
                if let Some(text) = selection_manager.get_text(&term_lock.grid()) {
                    if let Err(e) = clipboard.set_text(&text) {
                        log::error!("Failed to copy to clipboard: {}", e);
                    } else {
                        info!("Copied {} chars to clipboard", text.len());
                    }
                }
            }
        }
    }
}

/// Handle paste operation (Cmd+V)
pub(super) fn handle_paste(
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    renderer: &Arc<Mutex<saternal_core::Renderer>>,
    window: &winit::window::Window,
) {
    let mut clipboard = match Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            log::error!("Failed to create clipboard: {}", e);
            return;
        }
    };

    if let Ok(text) = clipboard.get_text() {
        info!("Pasting {} chars from clipboard", text.len());
        let bytes = if saternal_core::clipboard::should_bracket_paste(&text) {
            saternal_core::clipboard::bracket_paste(&text)
        } else {
            text.into_bytes()
        };
        
        if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
            let _ = active_tab.write_input(&bytes);
        }
        // Auto-scroll to bottom when user pastes text
        renderer.lock().reset_scroll();
        window.request_redraw();
    }
}
