//! Events for DataTable.
//!
//! This module contains the DataTableEvent enum for table interactions.

use super::types::SortDirection;
use crate::widget::ControlEvent;

/// Events emitted by DataTable.
#[derive(Debug, Clone)]
pub enum DataTableEvent {
    /// A row was selected. Contains the row index.
    RowSelected(usize),
    /// A row was activated (e.g., Enter pressed). Contains the row index.
    RowActivated(usize),
    /// A column was selected for sorting. Contains column ID and direction.
    ColumnSorted {
        column_id: String,
        direction: SortDirection,
    },
    /// Selection changed. Contains optional row index and optional column index.
    SelectionChanged {
        row: Option<usize>,
        column: Option<usize>,
    },
    /// Scroll position changed. Contains new offset.
    ScrollChanged(usize),
    /// Focus was gained.
    FocusGained,
    /// Focus was lost.
    FocusLost,
}

impl ControlEvent for DataTableEvent {}
