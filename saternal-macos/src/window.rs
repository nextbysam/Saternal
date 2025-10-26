use anyhow::Result;
use cocoa::appkit::{NSEvent, NSScreen, NSWindow, NSWindowStyleMask};
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

    /// Get the screen containing the mouse cursor (active screen)
    /// Falls back to main screen if mouse position cannot be determined
    unsafe fn get_screen_with_mouse() -> id {
        // Get current mouse location in screen coordinates
        let mouse_location: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        
        // Get all available screens
        let screens: id = msg_send![class!(NSScreen), screens];
        let screen_count: usize = msg_send![screens, count];
        
        // Find which screen contains the mouse cursor
        for i in 0..screen_count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            let frame: NSRect = msg_send![screen, frame];
            
            // Check if mouse is within this screen's bounds
            if mouse_location.x >= frame.origin.x
                && mouse_location.x < frame.origin.x + frame.size.width
                && mouse_location.y >= frame.origin.y
                && mouse_location.y < frame.origin.y + frame.size.height
            {
                info!("Window will appear on screen {} (mouse at {:.0}, {:.0})", 
                      i, mouse_location.x, mouse_location.y);
                return screen;
            }
        }
        
        // Fallback to main screen if mouse not found on any screen
        info!("Mouse not found on any screen, using main screen");
        msg_send![class!(NSScreen), mainScreen]
    }

    /// Configure a winit window to behave as a dropdown terminal
    /// ns_view is the winit NSView where wgpu will create the CAMetalLayer
    /// Returns (width, height, scale_factor) for terminal sizing
    pub unsafe fn configure_window(&self, ns_window: id, ns_view: id, height_percentage: f64) -> Result<(u32, u32, f64)> {
        // Get screen containing mouse cursor (active screen)
        let screen = Self::get_screen_with_mouse();
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

        // CRITICAL: Set window to transparent for vibrancy/wallpaper to work
        let () = msg_send![ns_window, setOpaque:NO];

        // Set a clear background for transparency
        let clear_color: id = msg_send![class!(NSColor), clearColor];
        let () = msg_send![ns_window, setBackgroundColor:clear_color];

        // CRITICAL: Make the WINIT VIEW layer-backed BEFORE wgpu creates the surface
        // wgpu will add the CAMetalLayer to THIS view, not the window's contentView!
        let () = msg_send![ns_view, setWantsLayer:YES];
        info!("Set winit NSView to layer-backed mode");

        // Get the screen's scale factor for proper terminal sizing
        let backing_scale_factor: f64 = msg_send![screen, backingScaleFactor];

        // Convert from points to physical pixels
        let physical_width = (window_width as f64 * backing_scale_factor).round() as u32;
        let physical_height = (window_height as f64 * backing_scale_factor).round() as u32;

        info!("Configured dropdown window: {}x{} at ({}, {}) with scale factor {:.2}x (physical: {}x{})",
              window_width, window_height, window_x, window_y, backing_scale_factor, physical_width, physical_height);

        Ok((physical_width, physical_height, backing_scale_factor))
    }

    /// Enable transparency layer after wgpu surface is created
    /// Call this AFTER the renderer is initialized
    pub unsafe fn enable_vibrancy_layer(&self, ns_window: id, ns_view: id, window: &winit::window::Window) -> Result<()> {
        // Configure the Metal layer for transparency
        self.configure_metal_layer(ns_view)?;
        info!("✓ Transparency layer configured");
        Ok(())
    }

    /// Configure the CAMetalLayer for transparency
    /// ns_view is the winit NSView where wgpu adds the CAMetalLayer
    unsafe fn configure_metal_layer(&self, ns_view: id) -> Result<()> {
        // Get the layer from the WINIT VIEW (not the window's contentView!)
        let layer: id = msg_send![ns_view, layer];

        if layer != nil {
            info!("Found layer on winit NSView");

            // Check if it's a CAMetalLayer
            let layer_class: id = msg_send![layer, class];
            // Get the class name properly - need to convert Class to NSString first
            let class_name_nsstring: id = msg_send![layer_class, description];
            let class_name: *const i8 = msg_send![class_name_nsstring, UTF8String];
            let class_str = std::ffi::CStr::from_ptr(class_name).to_str().unwrap_or("unknown");
            info!("Layer class: {}", class_str);

            // CRITICAL: Set layer to transparent for blur/wallpaper to work
            let () = msg_send![layer, setOpaque:NO];

            // Ensure it's not hidden
            let () = msg_send![layer, setHidden:NO];

            info!("Layer configured: opaque=NO (transparent), hidden=NO");
        } else {
            info!("WARNING: No layer found on winit NSView! wgpu may not have created it yet.");
        }

        Ok(())
    }

    /// Toggle window visibility with animation
    /// Returns (width, height, scale_factor) if window was shown and repositioned
    pub unsafe fn toggle(&self, ns_window: id) -> Result<Option<(u32, u32, f64)>> {
        let mut visible = self.visible.lock();
        let was_visible = *visible;
        *visible = !*visible;

        if *visible {
            // Only reposition if window was hidden (transitioning hidden→visible)
            // Don't reposition if toggling while already visible
            let dims = self.show_animated(ns_window, !was_visible)?;
            Ok(dims)
        } else {
            self.hide_animated(ns_window)?;
            Ok(None)
        }
    }

    /// Show window with slide-down animation
    /// should_reposition: if true, move window to screen with mouse cursor
    /// Returns (width, height, scale_factor) if repositioned, None otherwise
    unsafe fn show_animated(&self, ns_window: id, should_reposition: bool) -> Result<Option<(u32, u32, f64)>> {
        info!("Showing dropdown window (reposition: {})", should_reposition);

        let mut new_dims = None;
        
        // Only reposition if window was hidden (opening on active screen)
        // Don't reposition if window is already visible (just a toggle)
        if should_reposition {
            let screen = Self::get_screen_with_mouse();
            let screen_frame: NSRect = msg_send![screen, frame];
            let current_frame: NSRect = msg_send![ns_window, frame];
            
            // Calculate new position (keep same height, but move to active screen)
            let new_x = screen_frame.origin.x;
            let new_y = screen_frame.origin.y + screen_frame.size.height - current_frame.size.height;
            let new_width = screen_frame.size.width;
            
            let new_frame = NSRect::new(
                NSPoint::new(new_x, new_y),
                NSSize::new(new_width, current_frame.size.height),
            );
            
            let () = msg_send![ns_window, setFrame:new_frame display:YES];
            
            // Get the new screen's scale factor
            let backing_scale_factor: f64 = msg_send![screen, backingScaleFactor];
            
            info!("Window repositioned to screen with scale factor: {:.2}x, dimensions: {}x{}",
                  backing_scale_factor, new_width as u32, current_frame.size.height as u32);
            
            new_dims = Some((
                new_width as u32,
                current_frame.size.height as u32,
                backing_scale_factor,
            ));
        }

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

        Ok(new_dims)
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
