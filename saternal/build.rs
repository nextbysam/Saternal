use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    if target_os == "macos" {
        println!("cargo:rerun-if-changed=resources/macos/Info.plist");
        println!("cargo:rerun-if-changed=resources/macos/AppIcon.icns");
        println!("cargo:rerun-if-changed=resources/macos/entitlements.plist");
        
        // Copy resources to target directory for development
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let profile = env::var("PROFILE").unwrap();
        let target_dir = out_dir
            .ancestors()
            .nth(3)
            .unwrap()
            .to_path_buf();
        
        let resources_src = PathBuf::from("resources/macos");
        let resources_dst = target_dir.join("resources");
        
        if let Err(e) = fs::create_dir_all(&resources_dst) {
            println!("cargo:warning=Failed to create resources directory: {}", e);
        }
        
        for entry in ["Info.plist", "AppIcon.icns", "entitlements.plist"].iter() {
            let src = resources_src.join(entry);
            let dst = resources_dst.join(entry);
            if src.exists() {
                if let Err(e) = fs::copy(&src, &dst) {
                    println!("cargo:warning=Failed to copy {}: {}", entry, e);
                }
            }
        }
    }
}
