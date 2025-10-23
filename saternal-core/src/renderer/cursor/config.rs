use serde::{Deserialize, Serialize};

/// Cursor style types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    Block,
    Beam,
    Underline,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// Cursor configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorConfig {
    /// Cursor style (block, beam, underline)
    pub style: CursorStyle,
    /// Enable blinking
    pub blink: bool,
    /// Blink interval in milliseconds
    pub blink_interval_ms: u64,
    /// Cursor color (RGBA, values 0.0-1.0)
    pub color: [f32; 4],
    /// Force show cursor even when applications request to hide it
    /// Useful for TUI apps that don't properly manage cursor visibility
    #[serde(default)]
    pub force_show: bool,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            style: CursorStyle::Block,
            blink: true,
            blink_interval_ms: 530, // Standard terminal blink rate
            color: [1.0, 1.0, 1.0, 0.8], // White with 80% opacity
            force_show: false, // Respect application hide commands by default
        }
    }
}
