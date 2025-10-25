use super::App;
use anyhow::Result;
use cocoa::base::id;
use log::info;
use objc::{msg_send, sel, sel_impl};
use parking_lot::Mutex;
use saternal_core::{Clipboard, Renderer, SearchState, SelectionManager, MouseState};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::{
    event_loop::EventLoop,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::WindowBuilder,
};

impl App {
    /// Create a new application
    pub async fn new(config: saternal_core::Config) -> Result<Self> {
        info!("Initializing application");

        let event_loop = EventLoop::new()?;

        #[cfg(target_os = "macos")]
        unsafe {
            saternal_macos::set_app_icon();
        }

        let window = WindowBuilder::new()
            .with_title("Saternal")
            .with_decorations(false)
            .with_transparent(false)
            .with_visible(false)
            .build(&event_loop)?;

        let window = Arc::new(window);

        let dropdown = DropdownWindow::new();
        unsafe {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_view = appkit_handle.ns_view.as_ptr() as id;
                    let ns_window: id = msg_send![ns_view, window];
                    dropdown.configure_window(ns_window, ns_view, config.window.height_percentage)?;
                }
            }
        }
        let dropdown = Arc::new(Mutex::new(dropdown));

        let mut renderer = Renderer::new(
            window.clone(),
            &config.appearance.font_family,
            config.appearance.font_size,
            config.appearance.cursor,
            config.appearance.palette,
        )
        .await?;
        
        if let Some(scale_override) = config.appearance.dpi_scale_override {
            info!("Applying DPI scale override: {:.2}x", scale_override);
            renderer.handle_scale_factor_changed(scale_override)?;
        }
        
        let window_size = window.inner_size();
        let effective_size = renderer.font_manager().effective_font_size();
        let line_metrics = renderer.font_manager().font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = renderer.font_manager().font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let (initial_cols, initial_rows) = Self::calculate_terminal_size(
            window_size.width,
            window_size.height,
            cell_width,
            cell_height
        );
        info!("Calculated initial terminal size: {}x{} for window {}x{}",
              initial_cols, initial_rows, window_size.width, window_size.height);
        
        let renderer = Arc::new(Mutex::new(renderer));

        info!("Configuring Metal layer for rendering");
        unsafe {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_view = appkit_handle.ns_view.as_ptr() as id;
                    let ns_window: id = msg_send![ns_view, window];
                    dropdown.lock().enable_vibrancy_layer(ns_window, ns_view)?;
                }
            }
        }

        let tab_manager = crate::tab::TabManager::new_with_size(
            config.terminal.shell.clone(),
            initial_cols,
            initial_rows
        )?;
        let tab_manager = Arc::new(Mutex::new(tab_manager));

        let window_clone = window.clone();
        let dropdown_clone = dropdown.clone();
        let hotkey_manager = HotkeyManager::new(move || {
            info!("Hotkey triggered!");
            let mut dropdown = dropdown_clone.lock();
            unsafe {
                if let Ok(handle) = window_clone.window_handle() {
                    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                        let ns_view = appkit_handle.ns_view.as_ptr() as id;
                        let ns_window: id = msg_send![ns_view, window];
                        
                        match dropdown.toggle(ns_window) {
                            Ok(Some((width, height, scale_factor))) => {
                                info!("Window repositioned: {}x{} at scale {:.2}x - waiting for OS resize events",
                                      width, height, scale_factor);
                                window_clone.request_redraw();
                            }
                            Ok(None) => {
                                window_clone.request_redraw();
                            }
                            Err(e) => {
                                log::error!("Failed to toggle window: {}", e);
                            }
                        }
                    }
                }
            }
        })?;
        let hotkey_manager = Arc::new(hotkey_manager);

        let font_size = config.appearance.font_size;
        let selection_manager = SelectionManager::new();
        let clipboard = Clipboard::new()?;
        let search_state = SearchState::new();
        let mouse_state = MouseState::new();

        Ok(Self {
            config,
            event_loop,
            window,
            renderer,
            tab_manager,
            dropdown,
            hotkey_manager,
            font_size,
            selection_manager,
            clipboard,
            search_state,
            mouse_state,
        })
    }
}
