/// Clipboard integration for copy/paste support
mod platform;

pub use platform::Clipboard;

/// Check if text should use bracketed paste mode
pub fn should_bracket_paste(text: &str) -> bool {
    // Use bracketed paste for multi-line text or text with special chars
    text.contains('\n') || text.contains('\r')
}

/// Wrap text in bracketed paste sequences
pub fn bracket_paste(text: &str) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b"\x1b[200~");  // Start paste
    result.extend_from_slice(text.as_bytes());
    result.extend_from_slice(b"\x1b[201~");  // End paste
    result
}
