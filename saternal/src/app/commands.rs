/// Terminal commands for runtime wallpaper and opacity control
///
/// Supports:
/// - `wallpaper <path>` - Set wallpaper image
/// - `wallpaper clear` - Remove wallpaper
/// - `wallpaper-opacity <value>` - Set wallpaper opacity (0.0-1.0)
/// - `background-opacity <value>` - Set background opacity (0.0-1.0)

#[derive(Debug, Clone, PartialEq)]
pub enum TerminalCommand {
    Wallpaper { path: Option<String> },
    WallpaperOpacity { opacity: f32 },
    BackgroundOpacity { opacity: f32 },
}

/// Parse a command from terminal input
pub fn parse_command(line: &str) -> Option<TerminalCommand> {
    let line = line.trim();
    log::warn!("🔍 PARSING COMMAND: '{}'", line);

    // Wallpaper command
    if line.starts_with("wallpaper ") {
        let arg = line[10..].trim();
        if arg == "clear" {
            return Some(TerminalCommand::Wallpaper { path: None });
        } else if arg.is_empty() {
            // Empty argument - not a valid command
            return None;
        } else {
            // Validate that the argument looks like a valid file path
            if arg.len() < 3 || !arg.contains('.') {
                log::warn!("⚠️ INVALID WALLPAPER ARGUMENT: '{}' - too short or no extension", arg);
                return None;
            }
            let expanded_path = expand_tilde(arg);
            return Some(TerminalCommand::Wallpaper {
                path: Some(expanded_path),
            });
        }
    }

    // Wallpaper opacity command
    if line.starts_with("wallpaper-opacity ") {
        let arg = line[18..].trim();
        if let Ok(opacity) = arg.parse::<f32>() {
            if (0.0..=1.0).contains(&opacity) {
                return Some(TerminalCommand::WallpaperOpacity { opacity });
            } else {
                log::warn!("Wallpaper opacity must be between 0.0 and 1.0, got: {}", opacity);
                return None;
            }
        } else {
            log::warn!("Invalid opacity value: {}", arg);
            return None;
        }
    }

    // Background opacity command
    if line.starts_with("background-opacity ") {
        let arg = line[19..].trim();
        if let Ok(opacity) = arg.parse::<f32>() {
            if (0.0..=1.0).contains(&opacity) {
                return Some(TerminalCommand::BackgroundOpacity { opacity });
            } else {
                log::warn!("Background opacity must be between 0.0 and 1.0, got: {}", opacity);
                return None;
            }
        } else {
            log::warn!("Invalid opacity value: {}", arg);
            return None;
        }
    }

    None
}

/// Expand tilde (~) to home directory
fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            let mut home_path = std::path::PathBuf::from(home);
            home_path.push(&path[2..]); // Skip "~/"
            return home_path.to_string_lossy().to_string();
        }
    }
    path.to_string()
}

/// Format success message for command execution
pub fn format_success_message(cmd: &TerminalCommand) -> String {
    match cmd {
        TerminalCommand::Wallpaper { path: Some(p) } => {
            format!("✓ Wallpaper set: {}", p)
        }
        TerminalCommand::Wallpaper { path: None } => {
            "✓ Wallpaper cleared".to_string()
        }
        TerminalCommand::WallpaperOpacity { opacity } => {
            format!("✓ Wallpaper opacity set to {:.1}%", opacity * 100.0)
        }
        TerminalCommand::BackgroundOpacity { opacity } => {
            format!("✓ Background opacity set to {:.1}%", opacity * 100.0)
        }
    }
}

/// Format error message for command execution
pub fn format_error_message(cmd: &TerminalCommand, error: &str) -> String {
    match cmd {
        TerminalCommand::Wallpaper { path: Some(p) } => {
            format!("✗ Failed to load wallpaper '{}': {}", p, error)
        }
        TerminalCommand::Wallpaper { path: None } => {
            format!("✗ Failed to clear wallpaper: {}", error)
        }
        TerminalCommand::WallpaperOpacity { .. } => {
            format!("✗ Failed to set wallpaper opacity: {}", error)
        }
        TerminalCommand::BackgroundOpacity { .. } => {
            format!("✗ Failed to set background opacity: {}", error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wallpaper_command() {
        let cmd = parse_command("wallpaper /Users/sam/image.png");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("/Users/sam/image.png".to_string())
            })
        );
    }

    #[test]
    fn test_parse_wallpaper_clear() {
        let cmd = parse_command("wallpaper clear");
        assert_eq!(cmd, Some(TerminalCommand::Wallpaper { path: None }));
    }

    #[test]
    fn test_parse_wallpaper_opacity() {
        let cmd = parse_command("wallpaper-opacity 0.5");
        assert_eq!(
            cmd,
            Some(TerminalCommand::WallpaperOpacity { opacity: 0.5 })
        );
    }

    #[test]
    fn test_parse_wallpaper_opacity_invalid() {
        let cmd = parse_command("wallpaper-opacity 1.5");
        assert_eq!(cmd, None);
    }

    #[test]
    fn test_parse_background_opacity() {
        let cmd = parse_command("background-opacity 0.9");
        assert_eq!(
            cmd,
            Some(TerminalCommand::BackgroundOpacity { opacity: 0.9 })
        );
    }

    #[test]
    fn test_parse_unknown_command() {
        let cmd = parse_command("some-other-command");
        assert_eq!(cmd, None);
    }
}
