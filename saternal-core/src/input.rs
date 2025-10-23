/// Terminal input handling - converts keyboard events to terminal byte sequences
/// 
/// Implements VT100/xterm-compatible key sequences for terminal emulation.
/// Reference: TERMINAL_INPUT_REFERENCE.md

use winit::keyboard::{KeyCode, Key, ModifiersState};

/// Modifier key states for generating modified escape sequences
#[derive(Debug, Clone, Copy)]
pub struct InputModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl InputModifiers {
    pub fn from_winit(mods: ModifiersState) -> Self {
        Self {
            shift: mods.shift_key(),
            ctrl: mods.control_key(),
            alt: mods.alt_key(),
            meta: mods.super_key(),
        }
    }

    /// Get the modifier code for xterm modified keys
    /// Returns the number used in sequences like ESC[1;5A for Ctrl+Up
    fn xterm_modifier_code(&self) -> u8 {
        let mut code = 1;
        if self.shift { code += 1; }
        if self.alt { code += 2; }
        if self.ctrl { code += 4; }
        code
    }

    /// Check if any modifiers are active (excluding shift for letters)
    pub fn has_modifiers(&self) -> bool {
        self.ctrl || self.alt || self.meta
    }
}

/// Convert a keyboard input to terminal bytes
/// Returns Some(bytes) if the key produces terminal input, None otherwise
pub fn key_to_bytes(
    key: &Key,
    physical_key: KeyCode,
    mods: InputModifiers,
) -> Option<Vec<u8>> {
    // Handle Ctrl+[key] control characters (C0 codes)
    if mods.ctrl && !mods.alt && !mods.meta {
        if let Some(byte) = ctrl_key_to_byte(physical_key) {
            return Some(vec![byte]);
        }
    }

    // Handle special keys (arrows, function keys, navigation, etc.)
    if let Some(sequence) = special_key_to_sequence(physical_key, mods) {
        return Some(sequence);
    }

    // Handle Alt+key combinations (send ESC prefix)
    if mods.alt && !mods.ctrl && !mods.meta {
        if let Key::Character(ref s) = key {
            let mut bytes = vec![0x1B]; // ESC
            bytes.extend_from_slice(s.as_bytes());
            return Some(bytes);
        }
    }

    // Regular text input (handled by winit's event.text)
    None
}

/// Map Ctrl+key to control character bytes (C0 codes)
fn ctrl_key_to_byte(key: KeyCode) -> Option<u8> {
    match key {
        // Essential control characters
        KeyCode::KeyC => Some(0x03),  // ETX - SIGINT (interrupt)
        KeyCode::KeyD => Some(0x04),  // EOT - EOF
        KeyCode::KeyZ => Some(0x1A),  // SUB - SIGTSTP (suspend)
        KeyCode::Backslash => Some(0x1C), // FS - SIGQUIT
        
        // Readline/shell editing
        KeyCode::KeyA => Some(0x01),  // SOH - beginning of line
        KeyCode::KeyB => Some(0x02),  // STX - backward char
        KeyCode::KeyE => Some(0x05),  // ENQ - end of line
        KeyCode::KeyF => Some(0x06),  // ACK - forward char
        KeyCode::KeyK => Some(0x0B),  // VT - kill to end of line
        KeyCode::KeyL => Some(0x0C),  // FF - clear screen
        KeyCode::KeyN => Some(0x0E),  // SO - next history
        KeyCode::KeyP => Some(0x10),  // DLE - previous history
        KeyCode::KeyR => Some(0x12),  // DC2 - reverse search
        KeyCode::KeyT => Some(0x14),  // DC4 - transpose chars
        KeyCode::KeyU => Some(0x15),  // NAK - kill line backward
        KeyCode::KeyW => Some(0x17),  // ETB - kill word backward
        KeyCode::KeyY => Some(0x19),  // EM - yank (paste)
        
        // Flow control
        KeyCode::KeyQ => Some(0x11),  // DC1 - XON (resume)
        KeyCode::KeyS => Some(0x13),  // DC3 - XOFF (pause)
        
        // Other control characters
        KeyCode::KeyG => Some(0x07),  // BEL - bell
        KeyCode::KeyH => Some(0x08),  // BS - backspace
        KeyCode::KeyI => Some(0x09),  // HT - tab
        KeyCode::KeyJ => Some(0x0A),  // LF - line feed
        KeyCode::KeyM => Some(0x0D),  // CR - carriage return
        KeyCode::KeyV => Some(0x16),  // SYN - literal next
        KeyCode::KeyX => Some(0x18),  // CAN - cancel
        
        // Bracket keys for special control chars
        KeyCode::BracketLeft => Some(0x1B),   // ESC
        KeyCode::BracketRight => Some(0x1D),  // GS
        
        // Misc
        KeyCode::Minus => Some(0x1F),  // US - undo (Ctrl+_ is Ctrl+Shift+-)
        KeyCode::Digit6 if false => Some(0x1E), // RS - Ctrl+^ (but ^ is Shift+6)
        
        _ => None,
    }
}

/// Map special keys to ANSI escape sequences
fn special_key_to_sequence(key: KeyCode, mods: InputModifiers) -> Option<Vec<u8>> {
    match key {
        // Backspace and Delete
        KeyCode::Backspace => Some(vec![0x7F]), // DEL character (127)
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        
        // Enter/Return
        KeyCode::Enter => Some(vec![0x0D]), // CR
        
        // Tab
        KeyCode::Tab => {
            if mods.shift {
                // Shift+Tab for reverse tab (backtab)
                Some(b"\x1b[Z".to_vec())
            } else {
                Some(vec![0x09]) // HT
            }
        }
        
        // Escape
        KeyCode::Escape => Some(vec![0x1B]),
        
        // Arrow keys
        KeyCode::ArrowUp => arrow_sequence(b'A', mods),
        KeyCode::ArrowDown => arrow_sequence(b'B', mods),
        KeyCode::ArrowRight => arrow_sequence(b'C', mods),
        KeyCode::ArrowLeft => arrow_sequence(b'D', mods),
        
        // Navigation keys
        KeyCode::Home => navigation_sequence(b'H', 1, mods),
        KeyCode::End => navigation_sequence(b'F', 4, mods),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        
        // Function keys
        KeyCode::F1 => Some(b"\x1bOP".to_vec()),
        KeyCode::F2 => Some(b"\x1bOQ".to_vec()),
        KeyCode::F3 => Some(b"\x1bOR".to_vec()),
        KeyCode::F4 => Some(b"\x1bOS".to_vec()),
        KeyCode::F5 => Some(b"\x1b[15~".to_vec()),
        KeyCode::F6 => Some(b"\x1b[17~".to_vec()),
        KeyCode::F7 => Some(b"\x1b[18~".to_vec()),
        KeyCode::F8 => Some(b"\x1b[19~".to_vec()),
        KeyCode::F9 => Some(b"\x1b[20~".to_vec()),
        KeyCode::F10 => Some(b"\x1b[21~".to_vec()),
        KeyCode::F11 => Some(b"\x1b[23~".to_vec()),
        KeyCode::F12 => Some(b"\x1b[24~".to_vec()),
        
        _ => None,
    }
}

/// Generate arrow key sequences with optional modifiers
/// Format: ESC[A or ESC[1;5A for modified (e.g., Ctrl+Up)
fn arrow_sequence(letter: u8, mods: InputModifiers) -> Option<Vec<u8>> {
    if mods.ctrl || mods.shift || mods.alt {
        // Modified arrow key: ESC[1;{mod}A
        let mod_code = mods.xterm_modifier_code();
        Some(format!("\x1b[1;{}{}", mod_code, letter as char).into_bytes())
    } else {
        // Simple arrow key: ESC[A
        Some(format!("\x1b[{}", letter as char).into_bytes())
    }
}

/// Generate navigation key sequences (Home/End)
/// Can use either ESC[H or ESC[1~ style depending on terminal mode
fn navigation_sequence(letter: u8, tilde_code: u8, mods: InputModifiers) -> Option<Vec<u8>> {
    if mods.ctrl || mods.shift || mods.alt {
        // Modified: ESC[1;{mod}H
        let mod_code = mods.xterm_modifier_code();
        Some(format!("\x1b[1;{}{}", mod_code, letter as char).into_bytes())
    } else {
        // Simple: ESC[H (using letter form, more compatible)
        Some(format!("\x1b[{}", letter as char).into_bytes())
    }
}

/// Enable bracketed paste mode
pub fn enable_bracketed_paste() -> Vec<u8> {
    b"\x1b[?2004h".to_vec()
}

/// Disable bracketed paste mode
pub fn disable_bracketed_paste() -> Vec<u8> {
    b"\x1b[?2004l".to_vec()
}

/// Wrap pasted text in bracketed paste markers
pub fn bracket_paste(text: &str) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b"\x1b[200~"); // Start paste
    result.extend_from_slice(text.as_bytes());
    result.extend_from_slice(b"\x1b[201~"); // End paste
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ctrl_characters() {
        assert_eq!(ctrl_key_to_byte(KeyCode::KeyC), Some(0x03)); // Ctrl+C
        assert_eq!(ctrl_key_to_byte(KeyCode::KeyD), Some(0x04)); // Ctrl+D
        assert_eq!(ctrl_key_to_byte(KeyCode::KeyZ), Some(0x1A)); // Ctrl+Z
    }

    #[test]
    fn test_arrow_keys() {
        let mods = InputModifiers {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };
        assert_eq!(arrow_sequence(b'A', mods), Some(b"\x1b[A".to_vec())); // Up
        
        let mods_ctrl = InputModifiers {
            shift: false,
            ctrl: true,
            alt: false,
            meta: false,
        };
        assert_eq!(arrow_sequence(b'A', mods_ctrl), Some(b"\x1b[1;5A".to_vec())); // Ctrl+Up
    }

    #[test]
    fn test_special_keys() {
        let mods = InputModifiers {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        };
        assert_eq!(special_key_to_sequence(KeyCode::Backspace, mods), Some(vec![0x7F]));
        assert_eq!(special_key_to_sequence(KeyCode::Enter, mods), Some(vec![0x0D]));
        assert_eq!(special_key_to_sequence(KeyCode::Escape, mods), Some(vec![0x1B]));
    }
}
