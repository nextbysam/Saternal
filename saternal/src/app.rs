use crate::tab::TabManager;
use anyhow::Result;
use cocoa::base::id;
use log::{debug, info};
use objc::{msg_send, sel, sel_impl};
use parking_lot::Mutex;
use saternal_core::{Config, Renderer, key_to_bytes, InputModifiers};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent, ElementState, Modifiers, MouseScrollDelta},
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



        // Create tab manager
        let tab_manager = TabManager::new(config.terminal.shell.clone())?;
        let tab_manager = Arc::new(Mutex::new(tab_manager));

        // Setup global hotkey
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
        let renderer = Renderer::new(
            window_static,
            &config.appearance.font_family,
            config.appearance.font_size,
        )
        .await?;
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

        let font_size = config.appearance.font_size;

        Ok(Self {
            config,
            event_loop,
            window,
            renderer,
            tab_manager,
            dropdown,
            hotkey_manager,
            font_size,
        })
    }

    /// Run the application event loop
    pub fn run(mut self) -> Result<()> {
        let event_loop = self.event_loop;
        let window = self.window.clone();
        let renderer = self.renderer.clone();
        let tab_manager = self.tab_manager.clone();
        let hotkey_manager = self.hotkey_manager.clone();
        let mut font_size = self.font_size;
        let mut config = self.config.clone();
        let mut modifiers_state = Modifiers::default();  // Track modifier keys state

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
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } => {
                    // Only handle key presses, not releases
                    if event.state == ElementState::Pressed {
                        let cmd = modifiers_state.state().super_key();  // Cmd on macOS

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
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    // Convert scroll delta to lines
                    let scroll_lines = match delta {
                        MouseScrollDelta::LineDelta(_x, y) => {
                            // Mouse wheel: y > 0 = scroll up, y < 0 = scroll down
                            // Multiply by 3 for comfortable scrolling speed (3 lines per notch)
                            (y * 3.0) as i32
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            // Trackpad: convert pixels to lines
                            // Divide by approximate cell height (20px)
                            (pos.y / 20.0) as i32
                        }
                    };

                    if scroll_lines != 0 {
                        renderer.lock().scroll(scroll_lines);
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
                        // Get the active terminal for rendering
                        let term = if let Some(tab_mgr) = tab_manager.try_lock() {
                            tab_mgr.active_tab()
                                .and_then(|tab| tab.pane_tree.focused_pane())
                                .map(|pane| pane.terminal.term())
                        } else {
                            None
                        };
                        
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
