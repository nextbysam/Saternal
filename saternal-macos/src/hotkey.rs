use anyhow::Result;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use log::info;
use parking_lot::Mutex;
use std::sync::Arc;

/// Manages global hotkey registration and events
pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    hotkey: HotKey,
    callback: Arc<Mutex<Box<dyn FnMut() + Send + 'static>>>,
}

impl HotkeyManager {
    /// Create a new hotkey manager with Cmd+` (backtick)
    pub fn new<F>(callback: F) -> Result<Self>
    where
        F: FnMut() + Send + 'static,
    {
        info!("Initializing global hotkey manager");

        let manager = GlobalHotKeyManager::new()
            .map_err(|e| anyhow::anyhow!("Failed to create hotkey manager: {}", e))?;

        // Cmd+` on macOS (META modifier is Cmd key)
        let hotkey = HotKey::new(Some(Modifiers::META), Code::Backquote);

        manager
            .register(hotkey)
            .map_err(|e| anyhow::anyhow!("Failed to register hotkey: {}", e))?;

        info!("Registered global hotkey: Cmd+`");

        Ok(Self {
            manager,
            hotkey,
            callback: Arc::new(Mutex::new(Box::new(callback))),
        })
    }

    /// Process hotkey events (call this in your event loop)
    pub fn process_events(&self) {
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey.id() {
                let mut callback = self.callback.lock();
                callback();
            }
        }
    }

    /// Unregister the hotkey
    pub fn unregister(&self) -> Result<()> {
        self.manager
            .unregister(self.hotkey)
            .map_err(|e| anyhow::anyhow!("Failed to unregister hotkey: {}", e))
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_creation() {
        let _manager = HotkeyManager::new(|| {
            println!("Hotkey pressed!");
        });
        // Just ensure it doesn't panic
    }
}
