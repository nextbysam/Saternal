/// Selection range logic for terminal text selection
use alacritty_terminal::index::Point;

/// Selection mode determining granularity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Normal,   // Character-by-character
    Word,     // Whole words (double-click)
    Line,     // Whole lines (triple-click)
}

/// Selection range in terminal grid coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: Point,
    pub end: Point,
    pub mode: SelectionMode,
}

impl SelectionRange {
    /// Create a new selection range
    pub fn new(start: Point, end: Point, mode: SelectionMode) -> Self {
        Self { start, end, mode }
    }

    /// Get the normalized range (start always before end)
    #[inline]
    pub fn normalized(&self) -> (Point, Point) {
        if self.is_forward() {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// Check if selection is forward (start to end, top-left to bottom-right)
    #[inline]
    pub fn is_forward(&self) -> bool {
        self.start.line < self.end.line 
            || (self.start.line == self.end.line && self.start.column <= self.end.column)
    }

    /// Check if a point is within the selection
    #[inline]
    pub fn contains(&self, point: Point) -> bool {
        let (start, end) = self.normalized();
        
        // Single line selection
        if start.line == end.line {
            return point.line == start.line 
                && point.column >= start.column 
                && point.column <= end.column;
        }
        
        // Multi-line selection
        if point.line == start.line {
            point.column >= start.column
        } else if point.line == end.line {
            point.column <= end.column
        } else {
            point.line > start.line && point.line < end.line
        }
    }

    /// Update the end point of the selection
    pub fn update_end(&mut self, end: Point) {
        self.end = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::index::{Column, Line};

    #[test]
    fn test_normalized() {
        let range = SelectionRange::new(
            Point::new(Line(1), Column(5)),
            Point::new(Line(0), Column(3)),
            SelectionMode::Normal,
        );
        let (start, end) = range.normalized();
        assert_eq!(start, Point::new(Line(0), Column(3)));
        assert_eq!(end, Point::new(Line(1), Column(5)));
    }

    #[test]
    fn test_contains() {
        let range = SelectionRange::new(
            Point::new(Line(1), Column(2)),
            Point::new(Line(3), Column(5)),
            SelectionMode::Normal,
        );
        
        assert!(range.contains(Point::new(Line(1), Column(2))));
        assert!(range.contains(Point::new(Line(2), Column(0))));
        assert!(range.contains(Point::new(Line(3), Column(5))));
        assert!(!range.contains(Point::new(Line(0), Column(0))));
        assert!(!range.contains(Point::new(Line(4), Column(0))));
    }
}
