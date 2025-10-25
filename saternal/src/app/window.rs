use log::{debug, info};
use parking_lot::Mutex;
use saternal_core::Renderer;
use std::sync::Arc;
use winit::dpi::PhysicalSize;

/// Handle window resize events
pub(super) fn handle_resize(
    size: PhysicalSize<u32>,
    renderer: &Arc<Mutex<Renderer>>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    window: &winit::window::Window,
) {
    debug!("Window resized: {:?}", size);
    let mut renderer = renderer.lock();
    renderer.resize(size.width, size.height);
    
    let font_mgr = renderer.font_manager();
    let effective_size = font_mgr.effective_font_size();
    let line_metrics = font_mgr.font().horizontal_line_metrics(effective_size).unwrap();
    let cell_width = font_mgr.font().metrics('M', effective_size).advance_width;
    let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
    
    let (cols, rows) = super::App::calculate_terminal_size(
        size.width,
        size.height,
        cell_width,
        cell_height
    );
    debug!("Resizing terminal to {}x{} ({}x{} window, {}x{} cells)",
           cols, rows, size.width, size.height, cell_width, cell_height);
    drop(renderer);
    
    if let Some(mut tab_mgr) = tab_manager.try_lock() {
        if let Some(active_tab) = tab_mgr.active_tab_mut() {
            if let Err(e) = active_tab.resize(cols, rows) {
                log::error!("Failed to resize terminal: {}", e);
            }
        }
    }
    
    window.request_redraw();
}

/// Handle scale factor changed events
pub(super) fn handle_scale_factor_changed(
    scale_factor: f64,
    renderer: &Arc<Mutex<Renderer>>,
    window: &winit::window::Window,
) {
    info!("Scale factor changed: {:.2}x", scale_factor);
    let mut renderer = renderer.lock();
    if let Err(e) = renderer.handle_scale_factor_changed(scale_factor) {
        log::error!("Failed to handle scale factor change: {}", e);
    }
    window.request_redraw();
}

/// Handle redraw requests
pub(super) fn handle_redraw(
    renderer: &Arc<Mutex<Renderer>>,
    tab_manager: &Arc<Mutex<crate::tab::TabManager>>,
    window: &winit::window::Window,
) {
    if let (Some(mut renderer), Some(tab_mgr)) = (renderer.try_lock(), tab_manager.try_lock()) {
        if let Some(tab) = tab_mgr.active_tab() {
            let history_size = if let Some(pane) = tab.pane_tree.focused_pane() {
                if let Some(term_lock) = pane.terminal.term().try_lock() {
                    term_lock.history_size()
                } else {
                    0
                }
            } else {
                0
            };

            let scroll_offset = renderer.scroll_offset();
            if scroll_offset > 0 && history_size > 0 {
                let percentage = (scroll_offset * 100) / history_size.max(1);
                window.set_title(&format!("Saternal [↑ {}%] - Press Shift+G to jump to bottom", percentage));
            } else {
                window.set_title("Saternal");
            }
            
            if let Err(e) = renderer.render_with_panes(&tab.pane_tree) {
                log::error!("Render error: {}", e);
            }
        }
    }
}
