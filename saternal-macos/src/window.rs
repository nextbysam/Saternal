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
    /// ns_view is the winit NSView where wgpu will create the CAMetalLayer
    pub unsafe fn configure_window(&self, ns_window: id, ns_view: id, height_percentage: f64) -> Result<()> {
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

        // CRITICAL: Don't make window transparent - this prevents Metal from rendering
        // The window needs to be opaque for the Metal layer to be visible
        // We'll handle transparency through the rendering itself
        let () = msg_send![ns_window, setOpaque:YES];

        // Set a black background so we can see if Metal is rendering
        let black_color: id = msg_send![class!(NSColor), blackColor];
        let () = msg_send![ns_window, setBackgroundColor:black_color];

        // CRITICAL: Make the WINIT VIEW layer-backed BEFORE wgpu creates the surface
        // wgpu will add the CAMetalLayer to THIS view, not the window's contentView!
        let () = msg_send![ns_view, setWantsLayer:YES];
        info!("Set winit NSView to layer-backed mode");

        info!("Configured dropdown window: {}x{} at ({}, {})",
              window_width, window_height, window_x, window_y);

        Ok(())
    }

    /// Enable vibrancy after wgpu surface is created
    /// Call this AFTER the renderer is initialized
    pub unsafe fn enable_vibrancy_layer(&self, ns_window: id, ns_view: id) -> Result<()> {
        // First, let's inspect and configure the Metal layer on the winit view
        self.configure_metal_layer(ns_view)?;
        // Don't add vibrancy yet - let's just get Metal rendering working first
        // self.enable_vibrancy(ns_window)
        Ok(())
    }

    /// Configure the CAMetalLayer to be visible and opaque
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

            // Make sure layer is opaque
            let () = msg_send![layer, setOpaque:YES];

            // Ensure it's not hidden
            let () = msg_send![layer, setHidden:NO];

            info!("Layer configured: opaque=YES, hidden=NO");
        } else {
            info!("WARNING: No layer found on winit NSView! wgpu may not have created it yet.");
        }

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

        // Set state to active
        let () = msg_send![visual_effect_view, setState:1i64]; // NSVisualEffectStateActive = 1

        // Set autoresizing mask
        let () = msg_send![visual_effect_view, setAutoresizingMask:0x12]; // Width + Height sizable

        // CRITICAL: Insert at index 0 (bottom of the view hierarchy) so wgpu's Metal layer renders on top
        // Using addSubview:positioned:relativeTo: with NSWindowBelow (-1)
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
