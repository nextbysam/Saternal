/// UI overlay box renderer
/// Renders boxes with box-drawing characters on top of terminal content

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use crate::renderer::theme::ColorPalette;

/// Box drawing characters
const TOP_LEFT: char = '╭';
const TOP_RIGHT: char = '╮';
const BOTTOM_LEFT: char = '╰';
const BOTTOM_RIGHT: char = '╯';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';
const T_RIGHT: char = '├';  // Left edge with line to right
const T_LEFT: char = '┤';   // Right edge with line to left

/// Represents a UI box to be rendered
pub struct UIBox {
    pub title: String,
    pub lines: Vec<String>,
    pub border_color: AnsiColor,
    pub show_divider: bool,
    pub divider_after_line: Option<usize>,
}

impl UIBox {
    /// Create a new UI box
    pub fn new(title: String) -> Self {
        Self {
            title,
            lines: Vec::new(),
            border_color: AnsiColor::Named(NamedColor::Blue),
            show_divider: false,
            divider_after_line: None,
        }
    }

    /// Add a line to the box
    pub fn add_line(&mut self, line: String) {
        self.lines.push(line);
    }

    /// Add multiple lines
    pub fn add_lines(&mut self, lines: Vec<String>) {
        self.lines.extend(lines);
    }

    /// Set border color
    pub fn with_border_color(mut self, color: AnsiColor) -> Self {
        self.border_color = color;
        self
    }

    /// Set divider position
    pub fn with_divider_after(mut self, line_index: usize) -> Self {
        self.show_divider = true;
        self.divider_after_line = Some(line_index);
        self
    }

    /// Calculate the box width (longest line + padding)
    pub fn width(&self) -> usize {
        let title_width = self.title.chars().count() + 4; // "─ title ─"
        let content_width = self.lines.iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0) + 4; // 2 chars padding on each side
        
        title_width.max(content_width).max(40) // Minimum 40 chars
    }

    /// Calculate the box height
    pub fn height(&self) -> usize {
        let mut h = 2; // Top and bottom borders
        h += self.lines.len();
        if self.show_divider {
            h += 1;
        }
        h
    }

    /// Render the box to a grid of cells
    /// Returns a 2D vector of cells that can be overlaid on the terminal
    pub fn render(&self, _palette: &ColorPalette) -> Vec<Vec<Cell>> {
        let width = self.width();
        let mut rows: Vec<Vec<Cell>> = Vec::new();

        // Top border with title
        let mut top_row = Vec::new();
        top_row.push(self.create_border_cell(TOP_LEFT));
        top_row.push(self.create_border_cell(HORIZONTAL));
        
        // Add title
        let title_with_spaces = format!(" {} ", self.title);
        for ch in title_with_spaces.chars() {
            top_row.push(self.create_border_cell(ch));
        }
        
        // Fill rest with horizontal lines
        let remaining = width - top_row.len() - 1;
        for _ in 0..remaining {
            top_row.push(self.create_border_cell(HORIZONTAL));
        }
        top_row.push(self.create_border_cell(TOP_RIGHT));
        rows.push(top_row);

        // Content lines
        for (i, line) in self.lines.iter().enumerate() {
            let mut content_row = Vec::new();
            content_row.push(self.create_border_cell(VERTICAL));
            content_row.push(self.create_content_cell(' '));
            
            // Add line content
            for ch in line.chars() {
                content_row.push(self.create_content_cell(ch));
            }
            
            // Pad to width
            let remaining = width - content_row.len() - 2;
            for _ in 0..remaining {
                content_row.push(self.create_content_cell(' '));
            }
            
            content_row.push(self.create_content_cell(' '));
            content_row.push(self.create_border_cell(VERTICAL));
            rows.push(content_row);

            // Add divider if needed
            if self.show_divider && self.divider_after_line == Some(i) {
                let mut divider_row = Vec::new();
                divider_row.push(self.create_border_cell(T_RIGHT));
                for _ in 0..(width - 2) {
                    divider_row.push(self.create_border_cell(HORIZONTAL));
                }
                divider_row.push(self.create_border_cell(T_LEFT));
                rows.push(divider_row);
            }
        }

        // Bottom border
        let mut bottom_row = Vec::new();
        bottom_row.push(self.create_border_cell(BOTTOM_LEFT));
        for _ in 0..(width - 2) {
            bottom_row.push(self.create_border_cell(HORIZONTAL));
        }
        bottom_row.push(self.create_border_cell(BOTTOM_RIGHT));
        rows.push(bottom_row);

        rows
    }

    /// Create a cell for border characters
    fn create_border_cell(&self, ch: char) -> Cell {
        let mut cell = Cell::default();
        cell.c = ch;
        cell.fg = self.border_color;
        cell.flags.insert(Flags::BOLD);
        cell
    }

    /// Create a cell for content
    fn create_content_cell(&self, ch: char) -> Cell {
        let mut cell = Cell::default();
        cell.c = ch;
        cell
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_dimensions() {
        let mut box_ui = UIBox::new("Test".to_string());
        box_ui.add_line("Hello World".to_string());
        
        assert!(box_ui.width() >= 40);
        assert_eq!(box_ui.height(), 3); // Top + content + bottom
    }

    #[test]
    fn test_box_with_divider() {
        let mut box_ui = UIBox::new("Test".to_string());
        box_ui.add_line("Line 1".to_string());
        box_ui.add_line("Line 2".to_string());
        box_ui = box_ui.with_divider_after(0);
        
        assert_eq!(box_ui.height(), 5); // Top + line1 + divider + line2 + bottom
    }
}
