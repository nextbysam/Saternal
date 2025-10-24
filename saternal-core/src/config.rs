use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::renderer::cursor::CursorConfig;
use crate::renderer::theme::ColorPalette;

/// Configuration for Saternal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub window: WindowConfig,
    pub hotkey: HotkeyConfig,
    pub appearance: AppearanceConfig,
    pub terminal: TerminalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Window width as percentage of screen width (0.0-1.0)
    pub width_percentage: f64,
    /// Window height as percentage of screen height (0.0-1.0)
    pub height_percentage: f64,
    /// Animation duration in milliseconds
    pub animation_duration_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Hotkey to toggle the terminal (e.g., "cmd+`")
    pub toggle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Color palette for theming
    #[serde(default)]
    pub palette: ColorPalette,
    /// Font family
    pub font_family: String,
    /// Font size in points
    pub font_size: f32,
    /// Background opacity (0.0-1.0)
    pub opacity: f32,
    /// Enable background blur
    pub blur: bool,
    /// Cursor configuration
    #[serde(default)]
    pub cursor: CursorConfig,
    /// DPI scale override (None = auto-detect from system)
    /// Useful for edge cases like VNC, VMs, or unusual display setups
    #[serde(default)]
    pub dpi_scale_override: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Default shell command
    pub shell: String,
    /// Scrollback lines
    pub scrollback_lines: usize,
    /// Enable ligatures
    pub ligatures: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window: WindowConfig {
                width_percentage: 1.0,
                height_percentage: 0.5,
                animation_duration_ms: 180,
            },
            hotkey: HotkeyConfig {
                toggle: "cmd+`".to_string(),
            },
            appearance: AppearanceConfig {
                palette: ColorPalette::default(),
                font_family: "JetBrains Mono".to_string(),
                font_size: 14.0,
                opacity: 0.95,
                blur: true,
                cursor: CursorConfig::default(),
            },
            terminal: TerminalConfig {
                shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()),
                scrollback_lines: 10_000,
                ligatures: true,
            },
        }
    }
}

impl Config {
    /// Load configuration from file, or create default if not exists
    pub fn load(path: Option<PathBuf>) -> anyhow::Result<Self> {
        let config_path = path.unwrap_or_else(|| {
            let mut p = dirs::config_dir().expect("No config directory");
            p.push("saternal");
            p.push("config.toml");
            p
        });

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let contents = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, contents)?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self, path: Option<PathBuf>) -> anyhow::Result<()> {
        let config_path = path.unwrap_or_else(|| {
            let mut p = dirs::config_dir().expect("No config directory");
            p.push("saternal");
            p.push("config.toml");
            p
        });

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }
}

// Helper function to get home directory
fn dirs() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

mod dirs {
    use std::path::PathBuf;

    pub fn config_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| {
            let mut path = PathBuf::from(home);
            path.push(".config");
            path
        })
    }
}
