/// Search engine using Boyer-Moore-Horspool algorithm for fast substring search
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Cell;

/// Fast text search engine
pub struct SearchEngine {
    pattern: String,
    bad_char_table: [usize; 256],
}

impl SearchEngine {
    /// Create a new search engine with the given pattern
    pub fn new(pattern: &str) -> Self {
        let bad_char_table = Self::build_bad_char_table(pattern);
        Self {
            pattern: pattern.to_string(),
            bad_char_table,
        }
    }

    /// Build Boyer-Moore-Horspool skip table
    fn build_bad_char_table(pattern: &str) -> [usize; 256] {
        let mut table = [pattern.len(); 256];
        let pattern_bytes = pattern.as_bytes();
        
        for (i, &byte) in pattern_bytes.iter().enumerate().take(pattern.len().saturating_sub(1)) {
            table[byte as usize] = pattern.len() - 1 - i;
        }
        
        table
    }

    /// Find next match starting from given point
    pub fn find_next(&self, grid: &Grid<Cell>, start: Point) -> Option<Point> {
        if self.pattern.is_empty() {
            return None;
        }

        let num_lines = grid.screen_lines();
        let num_cols = grid.columns();
        
        // Convert grid to searchable text with position mapping
        let mut current_line = start.line.0;
        let mut current_col = start.column.0;

        while current_line < num_lines {
            let line_start_col = if current_line == start.line.0 { current_col } else { 0 };
            
            if let Some(match_col) = self.search_line(grid, current_line, line_start_col, num_cols) {
                return Some(Point::new(Line(current_line), Column(match_col)));
            }
            
            current_line += 1;
            current_col = 0;
        }

        None
    }

    /// Find previous match starting from given point
    pub fn find_prev(&self, grid: &Grid<Cell>, start: Point) -> Option<Point> {
        if self.pattern.is_empty() {
            return None;
        }

        let num_cols = grid.columns();
        let mut current_line = start.line.0;
        
        loop {
            let line_end_col = if current_line == start.line.0 { 
                start.column.0.saturating_sub(1)
            } else { 
                num_cols.saturating_sub(1)
            };
            
            if let Some(match_col) = self.search_line_reverse(grid, current_line, 0, line_end_col) {
                return Some(Point::new(Line(current_line), Column(match_col)));
            }
            
            if current_line == 0 {
                break;
            }
            current_line -= 1;
        }

        None
    }

    /// Find all matches in the grid
    pub fn find_all(&self, grid: &Grid<Cell>, max_matches: usize) -> Vec<Point> {
        if self.pattern.is_empty() {
            return Vec::new();
        }

        let mut matches = Vec::new();
        let num_lines = grid.screen_lines();
        let num_cols = grid.columns();

        for line in 0..num_lines {
            let mut col = 0;
            while col < num_cols {
                if let Some(match_col) = self.search_line(grid, line, col, num_cols) {
                    matches.push(Point::new(Line(line), Column(match_col)));
                    col = match_col + self.pattern.len();
                    
                    if matches.len() >= max_matches {
                        return matches;
                    }
                } else {
                    break;
                }
            }
        }

        matches
    }

    /// Search a single line using Boyer-Moore-Horspool
    fn search_line(&self, grid: &Grid<Cell>, line: usize, start_col: usize, end_col: usize) -> Option<usize> {
        let pattern_bytes = self.pattern.as_bytes();
        let pattern_len = pattern_bytes.len();
        
        if pattern_len == 0 || start_col + pattern_len > end_col {
            return None;
        }

        let mut pos = start_col;
        
        while pos + pattern_len <= end_col {
            let mut matched = true;
            
            // Check if pattern matches at current position
            for i in 0..pattern_len {
                let point = Point::new(Line(line), Column(pos + i));
                if let Some(cell) = grid.get(point) {
                    if cell.c.to_lowercase().next() != pattern_bytes[i].to_ascii_lowercase() as char {
                        matched = false;
                        
                        // Use bad character rule for skip
                        let last_char_point = Point::new(Line(line), Column(pos + pattern_len - 1));
                        if let Some(last_cell) = grid.get(last_char_point) {
                            let skip = self.bad_char_table[last_cell.c as usize];
                            pos += skip.max(1);
                        } else {
                            pos += 1;
                        }
                        break;
                    }
                } else {
                    matched = false;
                    pos += 1;
                    break;
                }
            }
            
            if matched {
                return Some(pos);
            }
        }

        None
    }

    /// Search a single line in reverse
    fn search_line_reverse(&self, grid: &Grid<Cell>, line: usize, start_col: usize, end_col: usize) -> Option<usize> {
        let pattern_bytes = self.pattern.as_bytes();
        let pattern_len = pattern_bytes.len();
        
        if pattern_len == 0 || end_col < pattern_len {
            return None;
        }

        let mut pos = end_col.saturating_sub(pattern_len - 1);
        
        loop {
            let mut matched = true;
            
            for i in 0..pattern_len {
                let point = Point::new(Line(line), Column(pos + i));
                if let Some(cell) = grid.get(point) {
                    if cell.c.to_lowercase().next() != pattern_bytes[i].to_ascii_lowercase() as char {
                        matched = false;
                        break;
                    }
                } else {
                    matched = false;
                    break;
                }
            }
            
            if matched {
                return Some(pos);
            }
            
            if pos == start_col {
                break;
            }
            pos = pos.saturating_sub(1);
        }

        None
    }

    /// Get the search pattern
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}
