/// Text selection module - mouse and keyboard selection support
mod range;
mod smart;
pub mod renderer;

pub use range::{SelectionMode, SelectionRange};
pub use renderer::{SelectionRenderer, PaneViewport, calculate_pane_viewports};

use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::Point;
use alacritty_terminal::term::cell::Cell;

/// Selection manager handling user interactions
pub struct SelectionManager {
    range: Option<SelectionRange>,
    active: bool,
}

impl SelectionManager {
    pub fn new() -> Self {
        Self {
            range: None,
            active: false,
        }
    }

    /// Start a new selection
    pub fn start(&mut self, point: Point, mode: SelectionMode) {
        self.range = Some(SelectionRange::new(point, point, mode));
        self.active = true;
    }

    /// Update selection end point
    pub fn update(&mut self, point: Point) {
        if let Some(range) = &mut self.range {
            range.update_end(point);
        }
    }

    /// Finalize selection and return selected text
    pub fn finalize(&mut self, grid: &Grid<Cell>) -> Option<String> {
        self.active = false;
        self.get_text(grid)
    }

    /// Clear selection
    pub fn clear(&mut self) {
        self.range = None;
        self.active = false;
    }

    /// Get current selection range
    pub fn range(&self) -> Option<SelectionRange> {
        self.range
    }

    /// Check if selection is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get selected text from grid
    pub fn get_text(&self, grid: &Grid<Cell>) -> Option<String> {
        let range = self.range?;
        let (start, end) = range.normalized();
        
        let mut text = String::new();
        let max_col = grid.columns().saturating_sub(1);
        let max_line = (grid.screen_lines() as i32).saturating_sub(1);
        
        // Clamp line indices to valid range
        let start_line = start.line.0.max(0).min(max_line);
        let end_line = end.line.0.max(0).min(max_line);
        
        for line in start_line..=end_line {
            let line_start = if line == start_line { 
                start.column.0.min(max_col) 
            } else { 
                0 
            };
            let line_end = if line == end_line { 
                end.column.0.min(max_col) 
            } else { 
                max_col 
            };
            
            for col in line_start..=line_end {
                let point = Point::new(alacritty_terminal::index::Line(line), alacritty_terminal::index::Column(col));
                let cell = &grid[point];
                text.push(cell.c);
            }
            
            // Add newline between lines (except for last line)
            if line < end_line {
                text.push('\n');
            }
        }
        
        Some(text)
    }

    /// Expand selection to word boundaries (double-click)
    pub fn expand_word(&mut self, grid: &Grid<Cell>, point: Point) {
        if let Some(range) = smart::expand_word(grid, point) {
            self.range = Some(range);
            self.active = false;  // Finalized
        }
    }

    /// Expand selection to line boundaries (triple-click)
    pub fn expand_line(&mut self, grid: &Grid<Cell>, point: Point) {
        let range = smart::expand_line(grid, point);
        self.range = Some(range);
        self.active = false;  // Finalized
    }
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}
