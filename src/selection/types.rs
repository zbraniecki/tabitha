//! Core selection types.
//!
//! This module defines the fundamental types for text selection:
//! - `SelectionPos`: Position within a selection region (col, row)
//! - `SelectionRange`: Anchor+cursor selection range with document-order methods
//! - `MouseSelectionPhase`: State machine for mouse selection

/// Position within a selection region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionPos {
    pub col: usize,
    pub row: usize,
}

impl SelectionPos {
    pub fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }
}

/// Anchor+cursor selection range.
///
/// The anchor is where the selection started, the cursor is where it extends to.
/// Use `ordered()` to get (start, end) in document order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub anchor: SelectionPos,
    pub cursor: SelectionPos,
}

impl SelectionRange {
    pub fn new(anchor: SelectionPos, cursor: SelectionPos) -> Self {
        Self { anchor, cursor }
    }

    /// Returns (start, end) in document order (top-to-bottom, left-to-right).
    pub fn ordered(&self) -> (SelectionPos, SelectionPos) {
        if self.anchor.row < self.cursor.row {
            (self.anchor, self.cursor)
        } else if self.anchor.row > self.cursor.row {
            (self.cursor, self.anchor)
        } else {
            // Same row, compare columns
            if self.anchor.col <= self.cursor.col {
                (self.anchor, self.cursor)
            } else {
                (self.cursor, self.anchor)
            }
        }
    }

    /// Check if a position is within the selection range.
    pub fn contains(&self, pos: SelectionPos) -> bool {
        let (start, end) = self.ordered();

        if pos.row < start.row || pos.row > end.row {
            return false;
        }

        if pos.row == start.row && pos.row == end.row {
            // Single row selection
            return pos.col >= start.col && pos.col < end.col;
        }

        if pos.row == start.row {
            return pos.col >= start.col;
        }

        if pos.row == end.row {
            return pos.col < end.col;
        }

        // Middle row
        true
    }

    /// Check if this selection is empty (anchor equals cursor).
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

/// Mouse selection state machine phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseSelectionPhase {
    /// No active selection.
    Idle,
    /// Mouse button is held down, selection is being extended.
    Dragging,
    /// Mouse button released, selection is finalized.
    Finalized,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_pos_new() {
        let pos = SelectionPos::new(5, 10);
        assert_eq!(pos.col, 5);
        assert_eq!(pos.row, 10);
    }

    #[test]
    fn test_selection_range_ordered_same_row_forward() {
        let anchor = SelectionPos::new(2, 5);
        let cursor = SelectionPos::new(8, 5);
        let range = SelectionRange::new(anchor, cursor);

        let (start, end) = range.ordered();
        assert_eq!(start.col, 2);
        assert_eq!(end.col, 8);
    }

    #[test]
    fn test_selection_range_ordered_same_row_backward() {
        let anchor = SelectionPos::new(8, 5);
        let cursor = SelectionPos::new(2, 5);
        let range = SelectionRange::new(anchor, cursor);

        let (start, end) = range.ordered();
        assert_eq!(start.col, 2);
        assert_eq!(end.col, 8);
    }

    #[test]
    fn test_selection_range_ordered_multi_row() {
        let anchor = SelectionPos::new(10, 5);
        let cursor = SelectionPos::new(2, 3);
        let range = SelectionRange::new(anchor, cursor);

        let (start, end) = range.ordered();
        assert_eq!(start.row, 3);
        assert_eq!(end.row, 5);
    }

    #[test]
    fn test_selection_range_contains() {
        let range = SelectionRange::new(SelectionPos::new(2, 1), SelectionPos::new(8, 3));

        // Inside the selection
        assert!(range.contains(SelectionPos::new(5, 2))); // Middle row
        assert!(range.contains(SelectionPos::new(2, 1))); // Start boundary (first row, first col)
        assert!(range.contains(SelectionPos::new(15, 2))); // Any col on middle row
        assert!(range.contains(SelectionPos::new(10, 1))); // After end col on first row (included)

        // Outside the selection
        assert!(!range.contains(SelectionPos::new(0, 0))); // Before start
        assert!(!range.contains(SelectionPos::new(1, 1))); // Before start col on first row
        assert!(!range.contains(SelectionPos::new(0, 4))); // After last row
        assert!(!range.contains(SelectionPos::new(8, 3))); // End boundary (exclusive)
        assert!(!range.contains(SelectionPos::new(15, 3))); // After end col on last row
    }

    #[test]
    fn test_selection_range_is_empty() {
        let pos = SelectionPos::new(5, 10);
        let empty_range = SelectionRange::new(pos, pos);
        assert!(empty_range.is_empty());

        let non_empty = SelectionRange::new(SelectionPos::new(5, 10), SelectionPos::new(6, 10));
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_mouse_selection_phase() {
        assert_ne!(MouseSelectionPhase::Idle, MouseSelectionPhase::Dragging);
        assert_ne!(
            MouseSelectionPhase::Dragging,
            MouseSelectionPhase::Finalized
        );
    }
}
