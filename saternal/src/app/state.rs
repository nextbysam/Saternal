use parking_lot::Mutex;
use saternal_core::{
    Clipboard, Config, Renderer, SearchState, SelectionManager, MouseState,
};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::event_loop::EventLoop;

/// Main application state
pub struct App {
    pub(super) config: Config,
    pub(super) event_loop: EventLoop<()>,
    pub(super) window: Arc<winit::window::Window>,
    pub(super) renderer: Arc<Mutex<Renderer>>,
    pub(super) tab_manager: Arc<Mutex<crate::tab::TabManager>>,
    pub(super) dropdown: Arc<Mutex<DropdownWindow>>,
    pub(super) hotkey_manager: Arc<HotkeyManager>,
    pub(super) font_size: f32,
    pub(super) selection_manager: SelectionManager,
    pub(super) clipboard: Clipboard,
    pub(super) search_state: SearchState,
    pub(super) mouse_state: MouseState,
}

impl App {
    /// Calculate terminal dimensions from window size
    /// Returns (cols, rows) with padding at bottom to prevent text cutoff
    pub(super) fn calculate_terminal_size(
        window_width: u32,
        window_height: u32,
        cell_width: f32,
        cell_height: f32,
    ) -> (usize, usize) {
        let cols = ((window_width as f32) / cell_width).floor() as usize;
        let rows = (((window_height as f32) / cell_height).floor() - 1.0).max(24.0) as usize;
        (cols.max(80), rows)
    }
}
