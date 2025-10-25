use serde::{Deserialize, Serialize};

/// Color palette for terminal theming
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColorPalette {
    /// Background color (RGBA, 0.0-1.0)
    pub background: [f32; 4],
    /// Foreground text color
    pub foreground: [f32; 4],
    /// Cursor color
    pub cursor: [f32; 4],
    /// Selection background color
    pub selection_bg: [f32; 4],
    /// ANSI colors (0-15: black, red, green, yellow, blue, magenta, cyan, white + bright variants)
    pub ansi_colors: [[f32; 4]; 16],
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::tokyo_night()
    }
}

impl ColorPalette {
    /// Tokyo Night theme (beautiful dark theme)
    pub fn tokyo_night() -> Self {
        Self {
            // Background: Deep dark blue-purple
            // Alpha = 1.0 in texture; final opacity controlled by shader's background_opacity uniform
            background: [0.09, 0.09, 0.13, 1.0], // #16161D, fully opaque in texture
            // Foreground: Light blue-white
            foreground: [0.76, 0.78, 0.84, 1.0],  // #C0CAF5
            // Cursor: Bright white with slight transparency
            cursor: [1.0, 1.0, 1.0, 0.8],
            // Selection: Subtle blue with transparency
            selection_bg: [0.2, 0.25, 0.35, 0.5],
            // ANSI colors optimized for Tokyo Night
            ansi_colors: [
                // Normal colors (0-7)
                [0.09, 0.11, 0.16, 1.0],  // 0: Black - #15161E
                [0.95, 0.40, 0.49, 1.0],  // 1: Red - #F7768E
                [0.60, 0.82, 0.58, 1.0],  // 2: Green - #9ECE6A
                [0.89, 0.80, 0.46, 1.0],  // 3: Yellow - #E0CC75
                [0.49, 0.68, 0.97, 1.0],  // 4: Blue - #7DACF8
                [0.74, 0.56, 0.98, 1.0],  // 5: Magenta - #BB9AF7
                [0.45, 0.84, 0.89, 1.0],  // 6: Cyan - #73DAE3
                [0.67, 0.71, 0.78, 1.0],  // 7: White - #ACB5C6
                
                // Bright colors (8-15)
                [0.34, 0.36, 0.43, 1.0],  // 8: Bright Black - #565A6E
                [0.96, 0.54, 0.60, 1.0],  // 9: Bright Red - #F58A99
                [0.72, 0.87, 0.69, 1.0],  // 10: Bright Green - #B8DEB0
                [0.92, 0.86, 0.63, 1.0],  // 11: Bright Yellow - #EBDCA1
                [0.63, 0.77, 0.98, 1.0],  // 12: Bright Blue - #A1C4FA
                [0.82, 0.70, 0.99, 1.0],  // 13: Bright Magenta - #D1B3FC
                [0.63, 0.89, 0.93, 1.0],  // 14: Bright Cyan - #A1E3ED
                [0.76, 0.78, 0.84, 1.0],  // 15: Bright White - #C0CAF5
            ],
        }
    }

    /// Convert hex color to normalized RGBA (helper for future theme loading)
    #[allow(dead_code)]
    pub fn hex_to_rgba(hex: &str, alpha: f32) -> [f32; 4] {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return [0.0, 0.0, 0.0, alpha];
        }
        
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32 / 255.0;
        
        [r, g, b, alpha]
    }

    /// Get ANSI color by index (0-15)
    pub fn get_ansi_color(&self, index: u8) -> [f32; 4] {
        if (index as usize) < self.ansi_colors.len() {
            self.ansi_colors[index as usize]
        } else {
            self.foreground // Fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_rgba() {
        let color = ColorPalette::hex_to_rgba("#FF8800", 1.0);
        assert!((color[0] - 1.0).abs() < 0.01); // Red channel
        assert!((color[1] - 0.533).abs() < 0.01); // Green channel (~136/255)
        assert!((color[2] - 0.0).abs() < 0.01); // Blue channel
        assert_eq!(color[3], 1.0); // Alpha
    }

    #[test]
    fn test_tokyo_night_theme() {
        let theme = ColorPalette::tokyo_night();
        assert_eq!(theme.ansi_colors.len(), 16);
        // Verify background is dark
        assert!(theme.background[0] < 0.2);
        assert!(theme.background[1] < 0.2);
        assert!(theme.background[2] < 0.2);
    }

    #[test]
    fn test_get_ansi_color() {
        let theme = ColorPalette::default();
        let red = theme.get_ansi_color(1);
        let bright_red = theme.get_ansi_color(9);
        // Bright red should be lighter than normal red
        assert!(bright_red[0] >= red[0]);
    }
}
