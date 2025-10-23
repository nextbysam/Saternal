use crate::tab::TabManager;
use anyhow::Result;
use cocoa::base::id;
use log::{debug, info};
use objc::{msg_send, sel, sel_impl};
use parking_lot::Mutex;
use saternal_core::{Config, Renderer};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
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
                    dropdown.configure_window(ns_window, config.window.height_percentage)?;
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

        // IMPORTANT: Add vibrancy layer AFTER wgpu creates its Metal layer
        // This ensures the vibrancy is behind the Metal rendering surface
        info!("Adding vibrancy layer behind Metal surface");
        unsafe {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_view = appkit_handle.ns_view.as_ptr() as id;
                    let ns_window: id = msg_send![ns_view, window];
                    dropdown.lock().enable_vibrancy_layer(ns_window)?;
                }
            }
        }

        Ok(Self {
            config,
            event_loop,
            window,
            renderer,
            tab_manager,
            dropdown,
            hotkey_manager,
        })
    }

    /// Run the application event loop
    pub fn run(self) -> Result<()> {
        let Self {
            event_loop,
            window,
            renderer,
            tab_manager,
            hotkey_manager,
            ..
        } = self;

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
                } if event.text.is_some() => {
                    if let Some(text) = &event.text {
                        debug!("Received text: {}", text);
                        // Send input to active terminal
                        if let Some(active_tab) = tab_manager.lock().active_tab_mut() {
                            let _ = active_tab.write_input(text.as_bytes());
                        }
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
