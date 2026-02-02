//! Type definitions for DataTable.
//!
//! This module contains enums and basic types used throughout the DataTable widget.

use ratatui::layout::Constraint;

/// Defines how a column's data should be interpreted for sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnType {
    /// Plain text, sorted lexicographically.
    #[default]
    Text,
    /// Integer numbers (parsed from string).
    Integer,
    /// Floating-point numbers.
    Float,
    /// Percentage values (e.g., "45%", "0.45").
    Percent,
    /// File/byte sizes (e.g., "1.5 GB", "500 KB").
    Size,
    /// Date/time (ISO 8601 format, sorted lexicographically).
    DateTime,
    /// Duration (e.g., "2h 30m").
    Duration,
}

/// Text alignment within a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Specifies how a column's width should be calculated.
#[derive(Debug, Clone)]
pub enum ColumnWidth {
    /// Fixed character width.
    Fixed(u16),
    /// Percentage of available table width.
    Percent(u8),
    /// Minimum width (will expand if space available).
    Min(u16),
    /// Maximum width (will shrink if needed).
    Max(u16),
    /// Fill remaining space with a flex factor.
    Fill(u16),
    /// Auto-size based on content (with optional max).
    Auto { max: Option<u16> },
}

impl Default for ColumnWidth {
    fn default() -> Self {
        ColumnWidth::Auto { max: None }
    }
}

impl ColumnWidth {
    /// Convert to ratatui Constraint for layout calculation.
    pub fn to_constraint(&self) -> Constraint {
        match self {
            ColumnWidth::Fixed(w) => Constraint::Length(*w),
            ColumnWidth::Percent(p) => Constraint::Percentage(*p as u16),
            ColumnWidth::Min(m) => Constraint::Min(*m),
            ColumnWidth::Max(m) => Constraint::Max(*m),
            ColumnWidth::Fill(factor) => Constraint::Fill(*factor),
            ColumnWidth::Auto { max } => match max {
                Some(m) => Constraint::Max(*m),
                None => Constraint::Min(5),
            },
        }
    }
}

/// Sort direction for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    /// Toggle between ascending and descending.
    pub fn toggle(&self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

/// Tracks the current sort state of the table.
#[derive(Debug, Clone)]
pub struct SortState {
    /// The column ID being sorted by.
    pub column_id: String,
    /// The sort direction.
    pub direction: SortDirection,
}

/// How selection works in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// No selection allowed.
    None,
    /// Only rows can be selected.
    #[default]
    Row,
    /// Only columns can be selected (header focus).
    Column,
    /// Both row and column can be selected (cell selection).
    Cell,
}
