use anyhow::Result;
use cocoa::appkit::{NSScreen, NSWindow, NSWindowStyleMask};
use cocoa::base::{id, nil, YES, NO};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use core_graphics::display::CGDisplay;
use log::info;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use parking_lot::Mutex;
use std::sync::Arc;

/// Manages the dropdown window behavior on macOS
pub struct DropdownWindow {
    visible: Arc<Mutex<bool>>,
    animation_duration: f64,
}

impl DropdownWindow {
    pub fn new() -> Self {
        Self {
            visible: Arc::new(Mutex::new(false)),
            animation_duration: 0.18, // 180ms
        }
    }

    /// Configure a winit window to behave as a dropdown terminal
    pub unsafe fn configure_window(&self, ns_window: id, height_percentage: f64) -> Result<()> {
        // Get main screen dimensions
        let screen: id = msg_send![class!(NSScreen), mainScreen];
        let screen_frame: NSRect = msg_send![screen, frame];

        // Calculate window dimensions
        let window_width = screen_frame.size.width;
        let window_height = screen_frame.size.height * height_percentage;

        // Position at top of screen
        let window_x = screen_frame.origin.x;
        let window_y = screen_frame.origin.y + screen_frame.size.height - window_height;

        let window_frame = NSRect::new(
            NSPoint::new(window_x, window_y),
            NSSize::new(window_width, window_height),
        );

        // Configure window style
        let () = msg_send![ns_window, setFrame:window_frame display:YES];

        // Set window level to float above other windows
        // NSStatusWindowLevel = 25
        let window_level: i64 = 25;
        let () = msg_send![ns_window, setLevel:window_level];

        // Set borderless style
        let style_mask = NSWindowStyleMask::NSBorderlessWindowMask
            | NSWindowStyleMask::NSResizableWindowMask;
        let () = msg_send![ns_window, setStyleMask:style_mask];

        // Make window not activate (doesn't steal focus from other apps)
        // but can receive key events when visible
        let () = msg_send![ns_window, setHidesOnDeactivate:NO];

        // Enable vibrancy/blur effect
        self.enable_vibrancy(ns_window)?;

        info!("Configured dropdown window: {}x{} at ({}, {})",
              window_width, window_height, window_x, window_y);

        Ok(())
    }

    /// Enable vibrancy (background blur) effect
    unsafe fn enable_vibrancy(&self, ns_window: id) -> Result<()> {
        // Create NSVisualEffectView for background blur
        let content_view: id = msg_send![ns_window, contentView];

        // NSVisualEffectView with dark appearance
        let visual_effect_class = class!(NSVisualEffectView);
        let visual_effect_view: id = msg_send![visual_effect_class, alloc];
        let frame: NSRect = msg_send![content_view, bounds];
        let visual_effect_view: id = msg_send![visual_effect_view, initWithFrame:frame];

        // NSVisualEffectBlendingModeBehindWindow = 0
        let blending_mode: i64 = 0;
        let () = msg_send![visual_effect_view, setBlendingMode:blending_mode];

        // NSVisualEffectMaterialDark = 2
        let material: i64 = 2;
        let () = msg_send![visual_effect_view, setMaterial:material];

        // Set autoresizing mask
        let () = msg_send![visual_effect_view, setAutoresizingMask:0x12]; // Width + Height sizable

        // Add as subview (NSWindowBelow = -1 to place it behind other views)
        let () = msg_send![content_view, addSubview:visual_effect_view positioned:(-1i64) relativeTo:nil];

        Ok(())
    }

    /// Toggle window visibility with animation
    pub unsafe fn toggle(&self, ns_window: id) -> Result<()> {
        let mut visible = self.visible.lock();
        *visible = !*visible;

        if *visible {
            self.show_animated(ns_window)?;
        } else {
            self.hide_animated(ns_window)?;
        }

        Ok(())
    }

    /// Show window with slide-down animation
    unsafe fn show_animated(&self, ns_window: id) -> Result<()> {
        info!("Showing dropdown window");

        // Make window visible
        let () = msg_send![ns_window, makeKeyAndOrderFront:nil];
        let () = msg_send![ns_window, orderFrontRegardless];

        // Animate opacity
        let () = msg_send![ns_window, setAlphaValue:0.0f64];

        // Use NSAnimationContext for smooth animation
        let animation_context: id = msg_send![class!(NSAnimationContext), currentContext];
        let () = msg_send![animation_context, setDuration:self.animation_duration];

        let () = msg_send![ns_window, animator];
        let () = msg_send![ns_window, setAlphaValue:1.0f64];

        Ok(())
    }

    /// Hide window with slide-up animation
    unsafe fn hide_animated(&self, ns_window: id) -> Result<()> {
        info!("Hiding dropdown window");

        // Animate opacity
        let animation_context: id = msg_send![class!(NSAnimationContext), currentContext];
        let () = msg_send![animation_context, setDuration:self.animation_duration];

        let () = msg_send![ns_window, animator];
        let () = msg_send![ns_window, setAlphaValue:0.0f64];

        // Hide after animation completes
        let () = msg_send![ns_window, performSelector:sel!(orderOut:)
                          withObject:nil
                          afterDelay:self.animation_duration];

        Ok(())
    }

    pub fn is_visible(&self) -> bool {
        *self.visible.lock()
    }

    pub fn set_animation_duration(&mut self, duration: f64) {
        self.animation_duration = duration;
    }
}

impl Default for DropdownWindow {
    fn default() -> Self {
        Self::new()
    }
}
