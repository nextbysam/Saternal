use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use log::{debug, info};
use parking_lot::Mutex;
use saternal_core::{Renderer, UIBox, ConfirmationLevel};
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
                    term_lock.grid().history_size()
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
            
            // Construct UI box from tab's UI message
            let ui_box = tab.ui_message.as_ref().map(|msg| {
                convert_ui_message_to_box(msg)
            });
            
            if let Err(e) = renderer.render_with_panes_and_ui(&tab.pane_tree, ui_box.as_ref()) {
                log::error!("Render error: {}", e);
            }
        }
    }
}

/// Convert UIMessage to UIBox for rendering
fn convert_ui_message_to_box(msg: &crate::tab::UIMessage) -> UIBox {
    use crate::tab::UIMessage;
    
    match msg {
        UIMessage::Generating { query } => {
            let mut box_ui = UIBox::new("🤖 Generating Command".to_string())
                .with_border_color(AnsiColor::Named(NamedColor::Blue));
            box_ui.add_line(query.clone());
            box_ui.add_line("⏳ Please wait...".to_string());
            box_ui
        }
        UIMessage::Suggestion { commands, safety } => {
            let title = "AI Generated Command";
            let border_color = match safety {
                ConfirmationLevel::Standard => AnsiColor::Named(NamedColor::Green),
                ConfirmationLevel::Sudo => AnsiColor::Named(NamedColor::Yellow),
                ConfirmationLevel::Elevated => AnsiColor::Named(NamedColor::Red),
            };
            
            let mut box_ui = UIBox::new(title.to_string())
                .with_border_color(border_color);
            
            // Add commands
            for cmd in commands {
                box_ui.add_line(cmd.clone());
            }
            
            // Add divider before explanation
            let last_cmd_idx = commands.len().saturating_sub(1);
            box_ui = box_ui.with_divider_after(last_cmd_idx);
            
            // Add explanation based on safety level
            match safety {
                ConfirmationLevel::Standard => {
                    box_ui.add_line("".to_string());
                    box_ui.add_line("[y/yes] Execute  [n/no] Cancel".to_string());
                }
                ConfirmationLevel::Sudo => {
                    box_ui.add_line("".to_string());
                    box_ui.add_line("⚠️  Requires elevated privileges (sudo)".to_string());
                    box_ui.add_line("[yes] Execute  [n/no] Cancel".to_string());
                }
                ConfirmationLevel::Elevated => {
                    box_ui.add_line("".to_string());
                    box_ui.add_line("⚠️  DANGEROUS: This command may cause data loss".to_string());
                    box_ui.add_line("[yes] Execute  [n/no] Cancel".to_string());
                }
            }
            
            box_ui
        }
        UIMessage::Error { message } => {
            let mut box_ui = UIBox::new("❌ Error".to_string())
                .with_border_color(AnsiColor::Named(NamedColor::Red));
            box_ui.add_line(message.clone());
            box_ui
        }
    }
}
