//! Data-driven table widget with sorting, selection, and scrolling.
//!
//! This module provides a flexible table widget that supports:
//! - Column definitions with types, alignment, and styling
//! - Row and column selection modes
//! - Sorting by any column (ascending/descending)
//! - Vertical scrolling for large datasets
//! - Focus-aware styling
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{DataTable, Column, ColumnType, ColumnAlign, ColumnWidth, SimpleRow};
//! use ratatui::style::{Color, Style};
//!
//! let columns = vec![
//!     Column::new("name", "Name")
//!         .with_width(ColumnWidth::Min(15)),
//!     Column::new("size", "Size")
//!         .with_type(ColumnType::Size)
//!         .with_align(ColumnAlign::Right),
//! ];
//!
//! let rows = vec![
//!     SimpleRow::new()
//!         .with_value("name", "Document.pdf")
//!         .with_value("size", "1.5 MB"),
//! ];
//!
//! let table = DataTable::new("my_table")
//!     .with_columns(columns)
//!     .with_rows(rows);
//! ```

// Submodules
mod column;
mod config;
mod events;
mod rendering;
mod row;
mod sorting;
mod table;
mod types;

// Public re-exports
pub use column::Column;
pub use config::DataTableConfig;
pub use events::DataTableEvent;
pub use row::{SimpleRow, TableRow};
pub use table::DataTable;
pub use types::{ColumnAlign, ColumnType, ColumnWidth, SelectionMode, SortDirection, SortState};
