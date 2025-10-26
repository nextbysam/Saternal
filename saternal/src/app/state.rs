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
    pub(super) command_buffer: Arc<Mutex<String>>,
}

impl App {
    /// Calculate terminal dimensions from window size
    /// Returns (cols, rows) accounting for padding to prevent text cutoff
    pub(super) fn calculate_terminal_size(
        window_width: u32,
        window_height: u32,
        cell_width: f32,
        cell_height: f32,
    ) -> (usize, usize) {
        // Padding constants must match text_rasterizer.rs
        const PADDING_LEFT: f32 = 10.0;
        const PADDING_TOP: f32 = 5.0;
        const PADDING_RIGHT: f32 = 10.0;
        const PADDING_BOTTOM: f32 = 10.0; // Ensure bottom line is visible
        
        // Calculate available space after padding
        let available_width = (window_width as f32 - PADDING_LEFT - PADDING_RIGHT).max(0.0);
        let available_height = (window_height as f32 - PADDING_TOP - PADDING_BOTTOM).max(0.0);
        
        // Calculate terminal dimensions from available space
        let cols = (available_width / cell_width).floor() as usize;
        let rows = (available_height / cell_height).floor() as usize;
        
        // Ensure minimum dimensions (just safety checks to prevent zero-size)
        (cols.max(1), rows.max(1))
    }
}
