use crate::tab::TabManager;
use anyhow::Result;
use log::{debug, info};
use parking_lot::Mutex;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use saternal_core::{Config, Renderer};
use saternal_macos::{DropdownWindow, HotkeyManager};
use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

/// Main application state
pub struct App<'a> {
    config: Config,
    event_loop: EventLoop<()>,
    window: Arc<winit::window::Window>,
    renderer: Arc<Mutex<Renderer<'a>>>,
    tab_manager: Arc<Mutex<TabManager>>,
    dropdown: Arc<Mutex<DropdownWindow>>,
    hotkey_manager: Arc<HotkeyManager>,
}

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
            .with_transparent(true)
            .with_visible(false) // Start hidden
            .build(&event_loop)?;

        let window = Arc::new(window);

        // Configure dropdown window behavior
        let dropdown = DropdownWindow::new();
        unsafe {
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                    let ns_window = appkit_handle.ns_window.as_ptr() as cocoa::base::id;
                    dropdown.configure_window(ns_window, config.window.height_percentage)?;
                }
            }
        }
        let dropdown = Arc::new(Mutex::new(dropdown));

        // Create renderer
        let renderer = Renderer::new(
            &window,
            &config.appearance.font_family,
            config.appearance.font_size,
        )
        .await?;
        let renderer = Arc::new(Mutex::new(renderer));

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
                        let ns_window = appkit_handle.ns_window.as_ptr() as cocoa::base::id;
                        let _ = dropdown.toggle(ns_window);
                    }
                }
            }
        })?;
        let hotkey_manager = Arc::new(hotkey_manager);

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
    pub async fn run(self) -> Result<()> {
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
                            let _ = active_tab.process_output();
                        }
                    }

                    // Request redraw
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Render the frame
                    if let Some(mut renderer) = renderer.try_lock() {
                        if let Err(e) = renderer.render() {
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
