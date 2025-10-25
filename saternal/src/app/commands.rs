use anyhow::Result;

/// Terminal commands that can be executed by the user
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalCommand {
    /// Set wallpaper from a file path, or clear it
    Wallpaper { path: Option<String> },
    /// Set wallpaper opacity (0.0-1.0)
    WallpaperOpacity { opacity: f32 },
    /// Set background opacity (0.0-1.0)
    BackgroundOpacity { opacity: f32 },
}

/// Parse a command from user input
///
/// Commands are prefixed with a special marker to distinguish them from normal terminal input.
/// Current supported commands:
/// - `wallpaper <path>` - Set wallpaper image
/// - `wallpaper clear` - Remove wallpaper
/// - `wallpaper-opacity <0.0-1.0>` - Set wallpaper opacity
/// - `background-opacity <0.0-1.0>` - Set background opacity
pub fn parse_command(input: &str) -> Option<TerminalCommand> {
    let trimmed = input.trim();

    // Check if it looks like a command (starts with a command keyword)
    if let Some(cmd) = trimmed.strip_prefix("wallpaper ") {
        let arg = cmd.trim();
        if arg == "clear" {
            return Some(TerminalCommand::Wallpaper { path: None });
        } else if !arg.is_empty() {
            // Expand ~ to home directory
            let expanded_path = if arg.starts_with('~') {
                if let Some(home) = std::env::var_os("HOME") {
                    arg.replacen("~", &home.to_string_lossy(), 1)
                } else {
                    arg.to_string()
                }
            } else {
                arg.to_string()
            };
            return Some(TerminalCommand::Wallpaper {
                path: Some(expanded_path),
            });
        }
    } else if let Some(opacity_str) = trimmed.strip_prefix("wallpaper-opacity ") {
        if let Ok(opacity) = opacity_str.trim().parse::<f32>() {
            if (0.0..=1.0).contains(&opacity) {
                return Some(TerminalCommand::WallpaperOpacity { opacity });
            }
        }
    } else if let Some(opacity_str) = trimmed.strip_prefix("background-opacity ") {
        if let Ok(opacity) = opacity_str.trim().parse::<f32>() {
            if (0.0..=1.0).contains(&opacity) {
                return Some(TerminalCommand::BackgroundOpacity { opacity });
            }
        }
    }

    None
}

/// Format a success message for a command
pub fn format_success(cmd: &TerminalCommand) -> String {
    match cmd {
        TerminalCommand::Wallpaper { path: Some(p) } => {
            format!("✓ Wallpaper set: {}\r\n", p)
        }
        TerminalCommand::Wallpaper { path: None } => {
            "✓ Wallpaper cleared\r\n".to_string()
        }
        TerminalCommand::WallpaperOpacity { opacity } => {
            format!("✓ Wallpaper opacity set to {:.1}%\r\n", opacity * 100.0)
        }
        TerminalCommand::BackgroundOpacity { opacity } => {
            format!("✓ Background opacity set to {:.1}%\r\n", opacity * 100.0)
        }
    }
}

/// Format an error message for a command
pub fn format_error(error: &anyhow::Error) -> String {
    format!("✗ Command failed: {}\r\n", error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wallpaper_set() {
        let cmd = parse_command("wallpaper /path/to/image.png");
        assert_eq!(
            cmd,
            Some(TerminalCommand::Wallpaper {
                path: Some("/path/to/image.png".to_string())
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
        assert_eq!(cmd, Some(TerminalCommand::WallpaperOpacity { opacity: 0.5 }));
    }

    #[test]
    fn test_parse_background_opacity() {
        let cmd = parse_command("background-opacity 0.8");
        assert_eq!(
            cmd,
            Some(TerminalCommand::BackgroundOpacity { opacity: 0.8 })
        );
    }

    #[test]
    fn test_parse_invalid_opacity() {
        // Out of range
        assert_eq!(parse_command("wallpaper-opacity 1.5"), None);
        assert_eq!(parse_command("wallpaper-opacity -0.1"), None);

        // Invalid format
        assert_eq!(parse_command("wallpaper-opacity abc"), None);
    }

    #[test]
    fn test_parse_non_command() {
        assert_eq!(parse_command("ls -la"), None);
        assert_eq!(parse_command("echo hello"), None);
    }
}
