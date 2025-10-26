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
            .with_transparent(true)
            .with_visible(false)
            .build(&event_loop)?;

        let window = Arc::new(window);

        let dropdown = DropdownWindow::new();
        let (window_width, window_height, window_scale_factor) = unsafe {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_view = appkit_handle.ns_view.as_ptr() as id;
                    let ns_window: id = msg_send![ns_view, window];
                    dropdown.configure_window(ns_window, ns_view, config.window.height_percentage)?
                } else {
                    return Err(anyhow::anyhow!("Failed to get AppKit window handle"));
                }
            } else {
                return Err(anyhow::anyhow!("Failed to get window handle"));
            }
        };
        let dropdown = Arc::new(Mutex::new(dropdown));

        let mut renderer = Renderer::new(
            window.clone(),
            &config.appearance.font_family,
            config.appearance.font_size,
            config.appearance.cursor,
            config.appearance.palette,
            config.appearance.wallpaper_path.as_deref(),
            config.appearance.wallpaper_opacity,
            config.appearance.opacity,
        )
        .await?;
        
        // Apply DPI scale from the window's screen (or override if configured)
        let effective_scale = config.appearance.dpi_scale_override.unwrap_or(window_scale_factor);
        if effective_scale != window.scale_factor() {
            info!("Applying scale factor: {:.2}x (window reported: {:.2}x)", 
                  effective_scale, window.scale_factor());
            renderer.handle_scale_factor_changed(effective_scale)?;
        }
        
        // Calculate terminal dimensions from the actual window dimensions
        let effective_size = renderer.font_manager().effective_font_size();
        let line_metrics = renderer.font_manager().font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = renderer.font_manager().font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let (initial_cols, initial_rows) = Self::calculate_terminal_size(
            window_width,
            window_height,
            cell_width,
            cell_height
        );
        info!("Calculated initial terminal size: {}x{} for window {}x{} (scale: {:.2}x)",
              initial_cols, initial_rows, window_width, window_height, effective_scale);
        
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
        let renderer_clone = renderer.clone();
        let tab_manager_clone = tab_manager.clone();
        let hotkey_manager = HotkeyManager::new(move || {
            info!("Hotkey triggered!");
            let mut dropdown = dropdown_clone.lock();
            unsafe {
                if let Ok(handle) = window_clone.window_handle() {
                    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                        let ns_view = appkit_handle.ns_view.as_ptr() as id;
                        let ns_window: id = msg_send![ns_view, window];
                        
                        match dropdown.toggle(ns_window) {
                            Ok(maybe_dimensions) => {
                                // ALWAYS check actual window size when hotkey is pressed
                                // The window size might have changed without toggle() detecting it
                                let size = window_clone.inner_size();
                                info!("Hotkey pressed - checking window size: {}x{}", size.width, size.height);

                                if let Some(mut renderer_lock) = renderer_clone.try_lock() {
                                    // CRITICAL: Update renderer dimensions first (like handle_resize)
                                    // This ensures padding calculations use current window size
                                    renderer_lock.resize(size.width, size.height);

                                    let fm = renderer_lock.font_manager();
                                    let effective_size = fm.effective_font_size();
                                    let line_metrics = fm.font().horizontal_line_metrics(effective_size).unwrap();
                                    let cell_width = fm.font().metrics('M', effective_size).advance_width;
                                    let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

                                    let (cols, rows) = App::calculate_terminal_size(size.width, size.height, cell_width, cell_height);
                                    info!("Resizing terminal to {}x{} for window {}x{}", cols, rows, size.width, size.height);
                                    drop(renderer_lock);

                                    if let Some(mut tab_mgr) = tab_manager_clone.try_lock() {
                                        if let Some(active_tab) = tab_mgr.active_tab_mut() {
                                            if let Err(e) = active_tab.resize(cols, rows) {
                                                log::error!("Failed to resize terminal: {}", e);
                                            }
                                        }
                                    }
                                }

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
