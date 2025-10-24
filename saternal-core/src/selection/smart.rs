/// Smart selection patterns for URLs, paths, and IPs
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Cell;
use super::range::{SelectionRange, SelectionMode};

/// Expand selection to include the word at the given point
pub fn expand_word(grid: &Grid<Cell>, point: Point) -> Option<SelectionRange> {
    let num_cols = grid.columns();
    let num_lines = grid.screen_lines();
    
    if point.line.0 < 0 || point.line.0 >= num_lines as i32 || point.column.0 >= num_cols {
        return None;
    }

    // Get the line content
    let line_index = point.line;
    
    // Find word boundaries
    let mut start_col = point.column.0;
    let mut end_col = point.column.0;
    
    // Expand left
    while start_col > 0 {
        let p = Point::new(line_index, Column(start_col - 1));
        let cell = &grid[p];
        if is_word_char(cell.c) {
            start_col -= 1;
        } else {
            break;
        }
    }
    
    // Expand right
    while end_col < num_cols - 1 {
        let p = Point::new(line_index, Column(end_col + 1));
        let cell = &grid[p];
        if is_word_char(cell.c) {
            end_col += 1;
        } else {
            break;
        }
    }
    
    Some(SelectionRange::new(
        Point::new(line_index, Column(start_col)),
        Point::new(line_index, Column(end_col)),
        SelectionMode::Word,
    ))
}

/// Expand selection to include the entire line
pub fn expand_line(grid: &Grid<Cell>, point: Point) -> SelectionRange {
    let num_cols = grid.columns();
    
    SelectionRange::new(
        Point::new(point.line, Column(0)),
        Point::new(point.line, Column(num_cols.saturating_sub(1))),
        SelectionMode::Line,
    )
}

/// Check if character is part of a word (alphanumeric, underscore, hyphen)
#[inline]
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == ':'
}

/// Detect if selection looks like a URL and expand accordingly
pub fn expand_url(grid: &Grid<Cell>, point: Point) -> Option<SelectionRange> {
    // First expand as word
    let mut range = expand_word(grid, point)?;
    
    // Check if it contains URL-like patterns
    let text = extract_text(grid, range);
    if text.contains("://") || text.starts_with("www.") {
        // Expand until whitespace
        range = expand_until_whitespace(grid, range);
    }
    
    Some(range)
}

/// Expand range until whitespace is encountered
fn expand_until_whitespace(grid: &Grid<Cell>, range: SelectionRange) -> SelectionRange {
    let num_cols = grid.columns();
    let mut start = range.start;
    let mut end = range.end;
    
    // Expand left
    while start.column.0 > 0 {
        let p = Point::new(start.line, Column(start.column.0 - 1));
        let cell = &grid[p];
        if !cell.c.is_whitespace() {
            start.column = Column(start.column.0 - 1);
        } else {
            break;
        }
    }
    
    // Expand right
    while end.column.0 < num_cols - 1 {
        let p = Point::new(end.line, Column(end.column.0 + 1));
        let cell = &grid[p];
        if !cell.c.is_whitespace() {
            end.column = Column(end.column.0 + 1);
        } else {
            break;
        }
    }
    
    SelectionRange::new(start, end, SelectionMode::Word)
}

/// Extract text from grid for a given range
fn extract_text(grid: &Grid<Cell>, range: SelectionRange) -> String {
    let (start, end) = range.normalized();
    let mut text = String::new();
    
    for line in start.line.0..=end.line.0 {
        let line_start = if line == start.line.0 { start.column.0 } else { 0 };
        let line_end = if line == end.line.0 { end.column.0 } else { grid.columns() - 1 };
        
        for col in line_start..=line_end {
            let p = Point::new(Line(line), Column(col));
            let cell = &grid[p];
            text.push(cell.c);
        }
    }
    
    text
}
