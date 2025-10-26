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
/// Simply looks for command keywords anywhere in the line (handles prompts automatically)
pub fn parse_command(line: &str) -> Option<TerminalCommand> {
    let line = line.trim();
    log::info!("🔍 PARSING COMMAND: '{}'", line);

    // Wallpaper command - find "wallpaper " anywhere in line
    if let Some(pos) = line.find("wallpaper ") {
        let arg = line[pos + 10..].trim();

        // First check: empty argument is not a valid command
        if arg.is_empty() {
            return None;
        }

        // Second check: "clear" means remove wallpaper
        if arg == "clear" {
            return Some(TerminalCommand::Wallpaper { path: None });
        }

        // Third check: expand tilde and validate resulting path
        let expanded_path = expand_tilde(arg);
        if expanded_path.is_empty() {
            return None;
        }

        return Some(TerminalCommand::Wallpaper {
            path: Some(expanded_path),
        });
    }

    // Wallpaper opacity command - find anywhere in line
    if let Some(pos) = line.find("wallpaper-opacity ") {
        let arg = line[pos + 18..].trim();
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

    // Background opacity command - find anywhere in line
    if let Some(pos) = line.find("background-opacity ") {
        let arg = line[pos + 19..].trim();
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
    if !path.starts_with('~') {
        return path.to_string();
    }

    // Get home directory (check USERPROFILE on Windows if HOME not set)
    let home = std::env::var_os("HOME")
        .or_else(|| {
            #[cfg(windows)]
            {
                std::env::var_os("USERPROFILE")
            }
            #[cfg(not(windows))]
            {
                None
            }
        });

    let Some(home) = home else {
        // No home directory available, return unchanged
        return path.to_string();
    };

    let mut home_path = std::path::PathBuf::from(home);

    // Handle exact "~" - return home directory
    if path == "~" {
        return home_path.to_string_lossy().to_string();
    }

    // Handle "~/" or "~\" (Windows)
    if path.starts_with("~/") {
        // Safe: we know path starts with "~/" (2 chars minimum)
        if let Some(remainder) = path.get(2..) {
            home_path.push(remainder);
        }
        return home_path.to_string_lossy().to_string();
    }

    #[cfg(windows)]
    if path.starts_with("~\\") {
        // Safe: we know path starts with "~\" (2 chars minimum)
        if let Some(remainder) = path.get(2..) {
            home_path.push(remainder);
        }
        return home_path.to_string_lossy().to_string();
    }

    // Anything else like "~user" - leave unchanged (user expansion not supported)
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

    // Prompt stripping tests
    #[test]
    fn test_strip_prompt_zsh() {
        let cmd = parse_command("sam@Sams-MacBook-Pro saternal % wallpaper beautiful.png");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("beautiful.png".to_string())
            })
        );
    }

    #[test]
    fn test_strip_prompt_bash() {
        let cmd = parse_command("user@host ~/dir $ wallpaper image.jpg");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("image.jpg".to_string())
            })
        );
    }

    #[test]
    fn test_strip_prompt_sh() {
        let cmd = parse_command("> wallpaper test.png");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("test.png".to_string())
            })
        );
    }

    #[test]
    fn test_strip_prompt_with_opacity() {
        let cmd = parse_command("user@host $ wallpaper-opacity 0.5");
        assert_eq!(
            cmd,
            Some(TerminalCommand::WallpaperOpacity { opacity: 0.5 })
        );
    }

    #[test]
    fn test_no_prompt() {
        // Should still work without prompt
        let cmd = parse_command("wallpaper /path/to/file.png");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("/path/to/file.png".to_string())
            })
        );
    }
}
