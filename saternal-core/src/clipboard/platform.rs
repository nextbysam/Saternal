/// Platform-specific clipboard implementation using arboard
use anyhow::Result;

/// Cross-platform clipboard
pub struct Clipboard {
    ctx: arboard::Clipboard,
}

impl Clipboard {
    /// Create a new clipboard instance
    pub fn new() -> Result<Self> {
        let ctx = arboard::Clipboard::new()?;
        Ok(Self { ctx })
    }

    /// Set clipboard text content
    pub fn set_text(&mut self, text: &str) -> Result<()> {
        self.ctx.set_text(text)?;
        Ok(())
    }

    /// Get clipboard text content
    pub fn get_text(&mut self) -> Result<String> {
        let text = self.ctx.get_text()?;
        Ok(text)
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new().expect("Failed to initialize clipboard")
    }
}
