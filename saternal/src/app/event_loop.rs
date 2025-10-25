use super::App;
use anyhow::Result;
use log::info;
use winit::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
};

impl App {
    /// Run the application event loop
    pub fn run(self) -> Result<()> {
        let event_loop = self.event_loop;
        let window = self.window.clone();
        let renderer = self.renderer.clone();
        let tab_manager = self.tab_manager.clone();
        let hotkey_manager = self.hotkey_manager.clone();
        let mut font_size = self.font_size;
        let mut config = self.config.clone();
        let mut modifiers_state = winit::event::Modifiers::default();
        
        let mut selection_manager = self.selection_manager;
        let mut search_state = self.search_state;
        let mut mouse_state = self.mouse_state;

        info!("Starting event loop");

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

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
                    super::window::handle_resize(size, &renderer, &tab_manager, &window);
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::ScaleFactorChanged { scale_factor, .. },
                    ..
                } => {
                    super::window::handle_scale_factor_changed(scale_factor, &renderer, &window);
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } => {
                    super::input::handle_keyboard_input(
                        &event,
                        event.state,
                        &modifiers_state,
                        &renderer,
                        &tab_manager,
                        &mut selection_manager,
                        &mut search_state,
                        &mut config,
                        &mut font_size,
                        &window,
                    );
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::MouseInput { state, button, .. },
                    ..
                } => {
                    super::mouse::handle_mouse_input(
                        state,
                        button,
                        &mut mouse_state,
                        &mut selection_manager,
                        &tab_manager,
                        &renderer,
                        &window,
                    );
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::CursorMoved { position, .. },
                    ..
                } => {
                    super::mouse::handle_cursor_moved(
                        position.x as f32,
                        position.y as f32,
                        &mut mouse_state,
                        &mut selection_manager,
                        &renderer,
                        &tab_manager,
                    );
                    window.request_redraw();
                }

                Event::WindowEvent {
                    event: WindowEvent::MouseWheel { delta, .. },
                    ..
                } => {
                    super::mouse::handle_mouse_wheel(delta, &renderer, &window);
                    window.request_redraw();
                }

                Event::AboutToWait => {
                    if let Some(mut tab_mgr) = tab_manager.try_lock() {
                        if let Some(active_tab) = tab_mgr.active_tab_mut() {
                            match active_tab.process_output() {
                                Ok(bytes_processed) => {
                                    // Only request redraw if there was actual output
                                    if bytes_processed > 0 {
                                        window.request_redraw();
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error processing output: {}", e);
                                }
                            }
                        } else {
                            log::warn!("No active tab found");
                        }
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    super::window::handle_redraw(&renderer, &tab_manager, &window);
                }

                _ => {}
            }
        })?;

        Ok(())
    }
}
