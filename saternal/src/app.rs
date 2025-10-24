use crate::tab::TabManager;
use alacritty_terminal::grid::Dimensions;
use anyhow::Result;
use cocoa::base::id;
use log::{debug, info};
use objc::{msg_send, sel, sel_impl};
use parking_lot::Mutex;
use saternal_core::{
    Clipboard, Config, MouseButton, MouseState, Renderer, SearchState, SelectionManager, 
    SelectionMode, key_to_bytes, InputModifiers, is_jump_to_bottom, pixel_to_grid,
};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent, ElementState, Modifiers, MouseScrollDelta, MouseButton as WinitMouseButton},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{PhysicalKey, KeyCode, Key},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::WindowBuilder,
};

/// Main application state
pub struct App<'a> {
    config: Config,
    event_loop: EventLoop<()>,
    window: Arc<winit::window::Window>,  // Keep for event loop
    renderer: Arc<Mutex<Renderer<'a>>>,
    tab_manager: Arc<Mutex<TabManager>>,
    dropdown: Arc<Mutex<DropdownWindow>>,
    hotkey_manager: Arc<HotkeyManager>,
    font_size: f32,  // Current font size (dynamically adjustable)
    // Phase 2: User interaction features
    selection_manager: SelectionManager,
    clipboard: Clipboard,
    search_state: SearchState,
    mouse_state: MouseState,
}

// SAFETY: The App struct is self-referential - renderer borrows from window.
// This is safe because:
// 1. Both are behind Arc, preventing moves
// 2. The renderer's lifetime is tied to 'a which spans the entire App lifetime
// 3. We never move window while renderer exists
// 4. The window Arc is never dropped before renderer

impl<'a> App<'a> {
    /// Create a new application
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing application");

        // Create event loop
        let event_loop = EventLoop::new()?;

        // Set application icon (macOS) - must be after EventLoop creation
        #[cfg(target_os = "macos")]
        unsafe {
            saternal_macos::set_app_icon();
        }

        // Create window
        let window = WindowBuilder::new()
            .with_title("Saternal")
            .with_decorations(false)
            .with_transparent(false) // CRITICAL: Must be opaque for Metal to render
            .with_visible(false) // Start hidden
            .build(&event_loop)?;

        let window = Arc::new(window);

        // Configure dropdown window behavior
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

        // Setup global hotkey (before renderer)
        let window_clone = window.clone();
        let dropdown_clone = dropdown.clone();
        let hotkey_manager = HotkeyManager::new(move || {
            info!("Hotkey triggered!");
            let dropdown = dropdown_clone.lock();
            unsafe {
                if let Ok(handle) = window_clone.window_handle() {
                    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                        let ns_view = appkit_handle.ns_view.as_ptr() as id;
                        let ns_window: id = msg_send![ns_view, window];
                        let _ = dropdown.toggle(ns_window);
                    }
                }
            }
        })?;
        let hotkey_manager = Arc::new(hotkey_manager);

        // SAFETY: We're creating a self-referential structure here.
        // The renderer holds a reference to window with lifetime 'a.
        // This is safe because:
        // 1. Both window and renderer are stored in the same struct
        // 2. The lifetime 'a is the lifetime of the App itself
        // 3. We use std::mem::transmute to extend the window's lifetime to 'a
        //    since it will live as long as the App struct
        let window_static: &'a winit::window::Window = unsafe {
            std::mem::transmute(&*window as &winit::window::Window)
        };
        
        // Now create the renderer with the extended lifetime
        let mut renderer = Renderer::new(
            window_static,
            &config.appearance.font_family,
            config.appearance.font_size,
            config.appearance.cursor,
            config.appearance.palette,
        )
        .await?;
        
        // Apply DPI scale override if configured
        if let Some(scale_override) = config.appearance.dpi_scale_override {
            info!("Applying DPI scale override: {:.2}x", scale_override);
            renderer.handle_scale_factor_changed(scale_override)?;
        }
        
        // Calculate proper terminal size from window dimensions BEFORE creating terminals
        let window_size = window.inner_size();
        let effective_size = renderer.font_manager().effective_font_size();
        let line_metrics = renderer.font_manager().font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = renderer.font_manager().font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let (initial_cols, initial_rows) = Self::calculate_terminal_size(window_size.width, window_size.height, cell_width, cell_height);
        info!("Calculated initial terminal size: {}x{} for window {}x{}", initial_cols, initial_rows, window_size.width, window_size.height);
        
        let renderer = Arc::new(Mutex::new(renderer));

        // IMPORTANT: Configure Metal layer AFTER wgpu creates it
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

        // Create tab manager with properly sized terminal from the start
        let tab_manager = TabManager::new_with_size(config.terminal.shell.clone(), initial_cols, initial_rows)?;
        let tab_manager = Arc::new(Mutex::new(tab_manager));

        let font_size = config.appearance.font_size;

        // Initialize Phase 2 features
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
    
    /// Calculate terminal dimensions from window size
    /// Returns (cols, rows) with padding at bottom to prevent text cutoff
    fn calculate_terminal_size(window_width: u32, window_height: u32, cell_width: f32, cell_height: f32) -> (usize, usize) {
        let cols = ((window_width as f32) / cell_width).floor() as usize;
        // Reserve ~1 row of padding at bottom to prevent descenders from being cut off
        let rows = (((window_height as f32) / cell_height).floor() - 1.0).max(24.0) as usize;
        (cols.max(80), rows)
    }

    /// Run the application event loop
    pub fn run(self) -> Result<()> {
        let event_loop = self.event_loop;
        let window = self.window.clone();
        let renderer = self.renderer.clone();
        let tab_manager = self.tab_manager.clone();
        let hotkey_manager = self.hotkey_manager.clone();
        let mut font_size = self.font_size;
        let mut config = self.config.clone();
        let mut modifiers_state = Modifiers::default();  // Track modifier keys state
        
        // Phase 2 features
        let mut selection_manager = self.selection_manager;
        let mut clipboard = self.clipboard;
        let mut search_state = self.search_state;
        let mut mouse_state = self.mouse_state;

        info!("Starting event loop");

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            // Process hotkey events
            hotkey_manager.process_events();

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    info!("Close requested");
                    elwt.exit();
                }

                Event::WindowEvent {
                    event: WindowEvent::ModifiersChanged(new_modifiers),
                    ..
                } => {
                    modifiers_state = new_modifiers;
                }

                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    debug!("Window resized: {:?}", size);
                    let mut renderer = renderer.lock();
                    renderer.resize(size.width, size.height);
                    
                    // Calculate new terminal dimensions based on font metrics
                    let font_mgr = renderer.font_manager();
                    let effective_size = font_mgr.effective_font_size();
                    let line_metrics = font_mgr.font().horizontal_line_metrics(effective_size).unwrap();
                    let cell_width = font_mgr.font().metrics('M', effective_size).advance_width;
                    let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
                    
                    let (cols, rows) = Self::calculate_terminal_size(size.width, size.height, cell_width, cell_height);
                    debug!("Resizing terminal to {}x{} ({}x{} window, {}x{} cells)", 
                           cols, rows, size.width, size.height, cell_width, cell_height);
                    drop(renderer);
                    
                    // Resize all terminals in all tabs
                    if let Some(mut tab_mgr) = tab_manager.try_lock() {
                        if let Some(active_tab) = tab_mgr.active_tab_mut() {
                            if let Err(e) = active_tab.resize(cols, rows) {
                                log::error!("Failed to resize terminal: {}", e);
                            }
                        }
                    }
                    
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::ScaleFactorChanged { scale_factor, .. },
                    ..
                } => {
                    info!("Scale factor changed: {:.2}x", scale_factor);
                    let mut renderer = renderer.lock();
                    if let Err(e) = renderer.handle_scale_factor_changed(scale_factor) {
                        log::error!("Failed to handle scale factor change: {}", e);
                    }
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } => {
                    // Only handle key presses, not releases
                    if event.state == ElementState::Pressed {
                        let cmd = modifiers_state.state().super_key();  // Cmd on macOS
                        let shift = modifiers_state.state().shift_key();

                        // Phase 2: Handle Cmd+[key] shortcuts for user interaction
                        if cmd {
                            if let PhysicalKey::Code(keycode) = event.physical_key {
                                match keycode {
                                    KeyCode::KeyC => {
                                        // Cmd+C: Copy selection to clipboard
                                        if let Some(tab_mgr) = tab_manager.try_lock() {
                                            if let Some(pane) = tab_mgr.active_tab()
                                                .and_then(|tab| tab.pane_tree.focused_pane()) 
                                            {
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
                                        return;
                                    }
                                    KeyCode::KeyV => {
                                        // Cmd+V: Paste from clipboard
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
                                        }
                                        return;
                                    }
                                    KeyCode::KeyF => {
                                        // Cmd+F: Activate search
                                        info!("Search activated (Cmd+F)");
                                        search_state.activate();
                                        // TODO: Show search UI overlay
                                        return;
                                    }
                                    KeyCode::KeyG => {
                                        // Cmd+G / Cmd+Shift+G: Find next/prev
                                        if search_state.is_active() {
                                            if let Some(tab_mgr) = tab_manager.try_lock() {
                                                if let Some(pane) = tab_mgr.active_tab()
                                                    .and_then(|tab| tab.pane_tree.focused_pane()) 
                                                {
                                                    if let Some(term_lock) = pane.terminal.term().try_lock() {
                                                        let result = if shift {
                                                            search_state.prev_match(&term_lock.grid())
                                                        } else {
                                                            search_state.next_match(&term_lock.grid())
                                                        };
                                                        
                                                        if let Some(match_point) = result {
                                                            info!("Found match at {:?}", match_point);
                                                            // TODO: Scroll to match
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        return;
                                    }
                                    _ => {}
                                }
                            }
                        }

                        // Handle Escape key
                        if matches!(event.logical_key, Key::Named(winit::keyboard::NamedKey::Escape)) {
                            if search_state.is_active() {
                                search_state.deactivate();
                                info!("Search deactivated");
                                return;
                            }
                            if selection_manager.range().is_some() {
                                selection_manager.clear();
                                renderer.lock().update_selection(None, 80, 24);
                                info!("Selection cleared");
                                return;
                            }
                        }

                        // Handle Cmd+[key] hotkeys for font size adjustment (macOS-specific UI)
                        if cmd {
                            // Check both text and physical key to handle both Cmd+= and Cmd++
                            let key_text = match &event.logical_key {
                                Key::Character(s) => Some(s.as_str()),
                                _ => None,
                            };
                            
                            let should_increase_font = match key_text {
                                Some("=" | "+") => true,
                                _ => {
                                    // Also check physical key for the equal/plus key
                                    if let PhysicalKey::Code(KeyCode::Equal) = event.physical_key {
                                        true
                                    } else {
                                        false
                                    }
                                }
                            };
                            
                            if should_increase_font {
                                // Increase font size
                                font_size = (font_size + 2.0).min(48.0);
                                info!("Increased font size to {}", font_size);
                                // Update config and save
                                config.appearance.font_size = font_size;
                                let _ = config.save(None);
                                // Update renderer font size in real-time
                                if let Some(mut renderer) = renderer.try_lock() {
                                    if let Err(e) = renderer.set_font_size(font_size) {
                                        log::error!("Failed to update font size: {}", e);
                                    }
                                }
                                return;  // Don't process as text input
                            } else if let Some(key_text) = key_text {
                                match key_text {
                                    "-" => {
                                        // Decrease font size
                                        font_size = (font_size - 2.0).max(8.0);
                                        info!("Decreased font size to {}", font_size);
                                        // Update config and save
                                        config.appearance.font_size = font_size;
                                        let _ = config.save(None);
                                        // Update renderer font size in real-time
                                        if let Some(mut renderer) = renderer.try_lock() {
                                            if let Err(e) = renderer.set_font_size(font_size) {
                                                log::error!("Failed to update font size: {}", e);
                                            }
                                        }
                                        return;  // Don't process as text input
                                    }
                                    "0" => {
                                        // Reset to default (14.0)
                                        font_size = 14.0;
                                        info!("Reset font size to default (14.0)");
                                        config.appearance.font_size = font_size;
                                        let _ = config.save(None);
                                        // Update renderer font size in real-time
                                        if let Some(mut renderer) = renderer.try_lock() {
                                            if let Err(e) = renderer.set_font_size(font_size) {
                                                log::error!("Failed to update font size: {}", e);
                                            }
                                        }
                                        return;  // Don't process as text input
                                    }
                                    _ => {}
                                }
                            }
                            // If we got here with Cmd pressed, don't send to terminal
                            return;
                        }

                        // Convert modifiers to our InputModifiers type
                        let input_mods = InputModifiers::from_winit(modifiers_state.state());

                        // Check for jump to bottom shortcut (Shift+G or Shift+End)
                        if let PhysicalKey::Code(keycode) = event.physical_key {
                            if is_jump_to_bottom(&event.logical_key, keycode, input_mods) {
                                info!("Jump to bottom triggered");
                                renderer.lock().reset_scroll();
                                window.request_redraw();
                                return; // Don't send to terminal
                            }
                        }

                        // Try to convert key to terminal bytes using the input module
                        if let PhysicalKey::Code(keycode) = event.physical_key {
                            if let Some(bytes) = key_to_bytes(&event.logical_key, keycode, input_mods) {
                                debug!("Sending key sequence: {:?}", bytes);
                                if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                                    let _ = active_tab.write_input(&bytes);
                                }
                                return;  // Processed by input module
                            }
                        }

                        // Handle regular text input (printable characters)
                        // Only send if no special modifiers (Ctrl/Alt) were active
                        if !input_mods.ctrl && !input_mods.alt {
                            if let Some(text) = &event.text {
                                debug!("Received text: {}", text);
                                // Send input to active terminal
                                if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                                    let _ = active_tab.write_input(text.as_bytes());
                                }
                            }
                        }
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => {
                    // Phase 2: Mouse selection
                    let mouse_button = match button {
                        WinitMouseButton::Left => MouseButton::Left,
                        WinitMouseButton::Right => MouseButton::Right,
                        WinitMouseButton::Middle => MouseButton::Middle,
                        _ => return,
                    };

                    match state {
                        ElementState::Pressed => {
                            mouse_state.press_button(mouse_button);
                            
                            // Start selection on left click
                            if mouse_button == MouseButton::Left {
                                let mode = match mouse_state.click_count {
                                    1 => SelectionMode::Normal,
                                    2 => SelectionMode::Word,
                                    3 => SelectionMode::Line,
                                    _ => SelectionMode::Normal,
                                };
                                
                                if mouse_state.click_count == 1 {
                                    selection_manager.start(mouse_state.position, mode);
                                } else if mouse_state.click_count == 2 {
                                    // Double-click: word selection
                                    if let Some(tab_mgr) = tab_manager.try_lock() {
                                        if let Some(pane) = tab_mgr.active_tab()
                                            .and_then(|tab| tab.pane_tree.focused_pane()) 
                                        {
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
                                } else if mouse_state.click_count == 3 {
                                    // Triple-click: line selection
                                    if let Some(tab_mgr) = tab_manager.try_lock() {
                                        if let Some(pane) = tab_mgr.active_tab()
                                            .and_then(|tab| tab.pane_tree.focused_pane()) 
                                        {
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
                            }
                        }
                        ElementState::Released => {
                            // Finalize selection on release
                            if mouse_button == MouseButton::Left && selection_manager.is_active() {
                                if let Some(tab_mgr) = tab_manager.try_lock() {
                                    if let Some(pane) = tab_mgr.active_tab()
                                        .and_then(|tab| tab.pane_tree.focused_pane()) 
                                    {
                                        if let Some(term_lock) = pane.terminal.term().try_lock() {
                                            let _ = selection_manager.finalize(&term_lock.grid());
                                        }
                                    }
                                }
                            }
                            mouse_state.release_button();
                        }
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    // Phase 2: Update mouse position and selection
                    // Get cell dimensions from renderer
                    if let Some(mut renderer_lock) = renderer.try_lock() {
                        let fm = renderer_lock.font_manager();
                        let line_metrics = fm.font().horizontal_line_metrics(fm.font_size()).unwrap();
                        let cell_width = fm.font().metrics('M', fm.font_size()).advance_width;
                        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
                        
                        mouse_state.update_position(position.x as f32, position.y as f32, cell_width, cell_height);
                        
                        // Update selection if dragging
                        if mouse_state.is_dragging() && selection_manager.is_active() {
                            selection_manager.update(mouse_state.position);
                            drop(renderer_lock);  // Drop before calling update_selection
                            
                            // Get grid dimensions from terminal
                            let (grid_cols, grid_lines) = if let Some(tab_mgr) = tab_manager.try_lock() {
                                if let Some(pane) = tab_mgr.active_tab()
                                    .and_then(|tab| tab.pane_tree.focused_pane()) 
                                {
                                    if let Some(term_lock) = pane.terminal.term().try_lock() {
                                        let grid = term_lock.grid();
                                        (grid.columns(), grid.screen_lines())
                                    } else {
                                        (80, 24)
                                    }
                                } else {
                                    (80, 24)
                                }
                            } else {
                                (80, 24)
                            };
                            
                            renderer.lock().update_selection(selection_manager.range(), grid_cols, grid_lines);
                        }
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    // Convert scroll delta to fractional lines for smooth scrolling
                    let scroll_delta = match delta {
                        MouseScrollDelta::LineDelta(_x, y) => {
                            // Mouse wheel: y > 0 = scroll up, y < 0 = scroll down
                            // Multiply by 3 for comfortable scrolling speed (3 lines per notch)
                            y * 3.0
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            // Trackpad: convert pixels to lines using typical cell height
                            // Average terminal cell height is ~18-20px
                            (pos.y / 18.0) as f32
                        }
                    };

                    if scroll_delta.abs() > 0.001 {
                        renderer.lock().scroll(scroll_delta);
                        window.request_redraw();
                    }
                }

                Event::AboutToWait => {
                    // Process terminal output
                    if let Some(mut tab_mgr) = tab_manager.try_lock() {
                        if let Some(active_tab) = tab_mgr.active_tab_mut() {
                            if let Err(e) = active_tab.process_output() {
                                log::error!("Error processing output: {}", e);
                            }
                        } else {
                            log::warn!("No active tab found");
                        }
                    }

                    // Request redraw
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Render the frame with terminal content
                    if let Some(mut renderer) = renderer.try_lock() {
                        // Get the active terminal for rendering and history size
                        let (term, history_size) = if let Some(tab_mgr) = tab_manager.try_lock() {
                            if let Some(pane) = tab_mgr.active_tab()
                                .and_then(|tab| tab.pane_tree.focused_pane()) 
                            {
                                let term_arc = pane.terminal.term();
                                // Get history size from the terminal (it's available on Term, not Grid)
                                let history_size = if let Some(term_lock) = term_arc.try_lock() {
                                    term_lock.history_size()
                                } else {
                                    0
                                };
                                (Some(term_arc), history_size)
                            } else {
                                (None, 0)
                            }
                        } else {
                            (None, 0)
                        };

                        // Update window title based on scroll position
                        let scroll_offset = renderer.scroll_offset();
                        if scroll_offset > 0 && history_size > 0 {
                            let percentage = (scroll_offset * 100) / history_size.max(1);
                            window.set_title(&format!("Saternal [↑ {}%] - Press Shift+G to jump to bottom", percentage));
                        } else {
                            window.set_title("Saternal");
                        }
                        
                        if let Err(e) = renderer.render(term) {
                            log::error!("Render error: {}", e);
                        }
                    }
                }

                _ => {}
            }
        })?;

        Ok(())
    }
}
