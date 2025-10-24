use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use super::theme::ColorPalette;

/// Convert ANSI terminal color to RGB tuple
pub(crate) fn ansi_to_rgb(color: &AnsiColor) -> (u8, u8, u8) {
    match color {
        AnsiColor::Named(named) => match named {
            NamedColor::Black => (0, 0, 0),
            NamedColor::Red => (205, 49, 49),
            NamedColor::Green => (13, 188, 121),
            NamedColor::Yellow => (229, 229, 16),
            NamedColor::Blue => (36, 114, 200),
            NamedColor::Magenta => (188, 63, 188),
            NamedColor::Cyan => (17, 168, 205),
            NamedColor::White => (229, 229, 229),
            NamedColor::BrightBlack => (102, 102, 102),
            NamedColor::BrightRed => (241, 76, 76),
            NamedColor::BrightGreen => (35, 209, 139),
            NamedColor::BrightYellow => (245, 245, 67),
            NamedColor::BrightBlue => (59, 142, 234),
            NamedColor::BrightMagenta => (214, 112, 214),
            NamedColor::BrightCyan => (41, 184, 219),
            NamedColor::BrightWhite => (255, 255, 255),
            NamedColor::Foreground => (229, 229, 229),
            _ => (229, 229, 229),
        },
        AnsiColor::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => {
            // Basic 256-color palette approximation
            match idx {
                0..=7 => ansi_to_rgb(&AnsiColor::Named(match idx {
                    0 => NamedColor::Black,
                    1 => NamedColor::Red,
                    2 => NamedColor::Green,
                    3 => NamedColor::Yellow,
                    4 => NamedColor::Blue,
                    5 => NamedColor::Magenta,
                    6 => NamedColor::Cyan,
                    7 => NamedColor::White,
                    _ => NamedColor::White,
                })),
                8..=15 => ansi_to_rgb(&AnsiColor::Named(match idx - 8 {
                    0 => NamedColor::BrightBlack,
                    1 => NamedColor::BrightRed,
                    2 => NamedColor::BrightGreen,
                    3 => NamedColor::BrightYellow,
                    4 => NamedColor::BrightBlue,
                    5 => NamedColor::BrightMagenta,
                    6 => NamedColor::BrightCyan,
                    7 => NamedColor::BrightWhite,
                    _ => NamedColor::White,
                })),
                _ => (229, 229, 229), // Default to white
            }
        },
    }
}

/// Convert ANSI terminal color to RGB using ColorPalette
pub(crate) fn ansi_to_rgb_with_palette(color: &AnsiColor, palette: &ColorPalette) -> (u8, u8, u8) {
    match color {
        AnsiColor::Named(named) => {
            let idx = match named {
                NamedColor::Black => 0,
                NamedColor::Red => 1,
                NamedColor::Green => 2,
                NamedColor::Yellow => 3,
                NamedColor::Blue => 4,
                NamedColor::Magenta => 5,
                NamedColor::Cyan => 6,
                NamedColor::White => 7,
                NamedColor::BrightBlack => 8,
                NamedColor::BrightRed => 9,
                NamedColor::BrightGreen => 10,
                NamedColor::BrightYellow => 11,
                NamedColor::BrightBlue => 12,
                NamedColor::BrightMagenta => 13,
                NamedColor::BrightCyan => 14,
                NamedColor::BrightWhite => 15,
                NamedColor::Foreground => {
                    let fg = palette.foreground;
                    return ((fg[0] * 255.0) as u8, (fg[1] * 255.0) as u8, (fg[2] * 255.0) as u8);
                }
                _ => {
                    let fg = palette.foreground;
                    return ((fg[0] * 255.0) as u8, (fg[1] * 255.0) as u8, (fg[2] * 255.0) as u8);
                }
            };
            let color = palette.get_ansi_color(idx);
            ((color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8)
        }
        AnsiColor::Spec(rgb) => (rgb.r, rgb.g, rgb.b),
        AnsiColor::Indexed(idx) => {
            if *idx < 16 {
                let color = palette.get_ansi_color(*idx);
                ((color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8)
            } else {
                // For colors 16-255, use the spec or default to foreground
                let fg = palette.foreground;
                ((fg[0] * 255.0) as u8, (fg[1] * 255.0) as u8, (fg[2] * 255.0) as u8)
            }
        }
    }
}
