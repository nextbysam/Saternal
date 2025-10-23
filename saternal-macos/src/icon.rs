use cocoa::base::{id, nil};
use cocoa::foundation::NSData;
use objc::{class, msg_send, sel, sel_impl};
use std::path::PathBuf;
use log::{info, warn};

/// Set the application dock icon
pub unsafe fn set_app_icon() {
    // Get NSApplication shared instance
    let app: id = msg_send![class!(NSApplication), sharedApplication];
    
    // Try to load icon from different locations
    let icon = try_load_icon();
    
    if icon != nil {
        let () = msg_send![app, setApplicationIconImage: icon];
        info!("Application dock icon set successfully");
    } else {
        warn!("Failed to load application icon");
    }
}

unsafe fn try_load_icon() -> id {
    // Try locations in order:
    // 1. Bundle resources (when running from .app)
    // 2. Project resources (when running with cargo run)
    // 3. Target resources (after build.rs copies them)
    
    // Try bundle resources first
    if let Some(icon) = load_from_bundle() {
        info!("Loaded icon from bundle resources");
        return icon;
    }
    
    // Try common development paths
    let dev_paths = vec![
        // When running from workspace root with cargo run
        "target/release/resources/AppIcon.icns",
        "target/debug/resources/AppIcon.icns",
        // When running from saternal directory
        "../target/release/resources/AppIcon.icns",
        "../target/debug/resources/AppIcon.icns",
        // Direct project resources
        "saternal/resources/macos/AppIcon.icns",
        "resources/macos/AppIcon.icns",
        "../saternal/resources/macos/AppIcon.icns",
        // If running from target directory
        "resources/AppIcon.icns",
        "../../saternal/resources/macos/AppIcon.icns",
    ];
    
    for path in dev_paths {
        if let Some(icon) = load_from_file(path) {
            info!("Loaded icon from path: {}", path);
            return icon;
        }
    }
    
    nil
}

unsafe fn load_from_bundle() -> Option<id> {
    let bundle: id = msg_send![class!(NSBundle), mainBundle];
    if bundle == nil {
        return None;
    }
    
    let icon_path_nsstring: id = msg_send![class!(NSString), stringWithUTF8String: "AppIcon\0".as_ptr()];
    let icns_nsstring: id = msg_send![class!(NSString), stringWithUTF8String: "icns\0".as_ptr()];
    
    let path: id = msg_send![bundle, pathForResource:icon_path_nsstring ofType:icns_nsstring];
    if path == nil {
        return None;
    }
    
    let image: id = msg_send![class!(NSImage), alloc];
    let image: id = msg_send![image, initWithContentsOfFile: path];
    
    if image != nil {
        Some(image)
    } else {
        None
    }
}

unsafe fn load_from_file(path: &str) -> Option<id> {
    let full_path = PathBuf::from(path);
    if !full_path.exists() {
        return None;
    }
    
    let path_str = full_path.to_str()?;
    let path_cstr = format!("{}\0", path_str);
    let path_nsstring: id = msg_send![class!(NSString), stringWithUTF8String: path_cstr.as_ptr()];
    
    let image: id = msg_send![class!(NSImage), alloc];
    let image: id = msg_send![image, initWithContentsOfFile: path_nsstring];
    
    if image != nil {
        Some(image)
    } else {
        None
    }
}


