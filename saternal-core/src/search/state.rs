/// Search state management
use super::engine::SearchEngine;
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::Point;
use alacritty_terminal::term::cell::Cell;

/// Search direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

/// Search state manager
pub struct SearchState {
    active: bool,
    pattern: String,
    engine: Option<SearchEngine>,
    current_match: Option<Point>,
    all_matches: Vec<Point>,
    direction: SearchDirection,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            active: false,
            pattern: String::new(),
            engine: None,
            current_match: None,
            all_matches: Vec::new(),
            direction: SearchDirection::Forward,
        }
    }

    /// Activate search mode
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate search mode
    pub fn deactivate(&mut self) {
        self.active = false;
        self.pattern.clear();
        self.engine = None;
        self.current_match = None;
        self.all_matches.clear();
    }

    /// Check if search is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Update search pattern
    pub fn update_pattern(&mut self, pattern: &str, grid: &Grid<Cell>) {
        self.pattern = pattern.to_string();
        
        if pattern.is_empty() {
            self.engine = None;
            self.all_matches.clear();
            self.current_match = None;
        } else {
            self.engine = Some(SearchEngine::new(pattern));
            self.refresh_matches(grid);
        }
    }

    /// Refresh all matches in the grid
    fn refresh_matches(&mut self, grid: &Grid<Cell>) {
        if let Some(engine) = &self.engine {
            self.all_matches = engine.find_all(grid, 1000);  // Cap at 1000 matches
            
            // Set current match to first result
            if !self.all_matches.is_empty() {
                self.current_match = Some(self.all_matches[0]);
            } else {
                self.current_match = None;
            }
        }
    }

    /// Find next match
    pub fn next_match(&mut self, grid: &Grid<Cell>) -> Option<Point> {
        let engine = self.engine.as_ref()?;
        
        let start = if let Some(current) = self.current_match {
            // Move past current match
            let mut next = current;
            next.column.0 += 1;
            if next.column.0 >= grid.columns() {
                next.line.0 += 1;
                next.column.0 = 0;
            }
            next
        } else {
            Point::new(alacritty_terminal::index::Line(0), alacritty_terminal::index::Column(0))
        };

        if let Some(match_point) = engine.find_next(grid, start) {
            self.current_match = Some(match_point);
            self.direction = SearchDirection::Forward;
            Some(match_point)
        } else {
            // Wrap around to beginning
            let wrapped = engine.find_next(grid, Point::new(
                alacritty_terminal::index::Line(0),
                alacritty_terminal::index::Column(0)
            ));
            if wrapped.is_some() {
                self.current_match = wrapped;
                self.direction = SearchDirection::Forward;
            }
            wrapped
        }
    }

    /// Find previous match
    pub fn prev_match(&mut self, grid: &Grid<Cell>) -> Option<Point> {
        let engine = self.engine.as_ref()?;
        
        let start = if let Some(current) = self.current_match {
            current
        } else {
            Point::new(
                alacritty_terminal::index::Line(grid.screen_lines().saturating_sub(1) as i32),
                alacritty_terminal::index::Column(grid.columns().saturating_sub(1))
            )
        };

        if let Some(match_point) = engine.find_prev(grid, start) {
            self.current_match = Some(match_point);
            self.direction = SearchDirection::Backward;
            Some(match_point)
        } else {
            // Wrap around to end
            let wrapped = engine.find_prev(grid, Point::new(
                alacritty_terminal::index::Line(grid.screen_lines().saturating_sub(1) as i32),
                alacritty_terminal::index::Column(grid.columns().saturating_sub(1))
            ));
            if wrapped.is_some() {
                self.current_match = wrapped;
                self.direction = SearchDirection::Backward;
            }
            wrapped
        }
    }

    /// Get current search pattern
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Get current match position
    pub fn current_match(&self) -> Option<Point> {
        self.current_match
    }

    /// Get all match positions (for highlighting)
    pub fn matches(&self) -> &[Point] {
        &self.all_matches
    }

    /// Get match count
    pub fn match_count(&self) -> usize {
        self.all_matches.len()
    }

    /// Get current match index (1-based for display)
    pub fn current_match_index(&self) -> Option<usize> {
        let current = self.current_match?;
        self.all_matches.iter().position(|&p| p == current).map(|i| i + 1)
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}
