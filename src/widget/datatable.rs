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

use std::borrow::Cow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::HashMap;

use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell as RatatuiCell, Row, Table},
    Frame,
};

use crate::event::Event;
use crate::focus::EventResult;
use crate::KeyCode;

use super::{Control, ControlEvent};

// =============================================================================
// Column Types
// =============================================================================

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

// =============================================================================
// Column Definition
// =============================================================================

/// Configuration for a single table column.
#[derive(Debug, Clone)]
pub struct Column {
    /// Unique identifier for the column.
    id: String,
    /// Display title in the header.
    title: String,
    /// Data type for sorting.
    column_type: ColumnType,
    /// Text alignment.
    align: ColumnAlign,
    /// Width specification.
    width: ColumnWidth,
    /// Whether this column can be sorted.
    sortable: bool,
    /// Style for values in this column.
    value_style: Option<Style>,
    /// Style for the header cell.
    header_style: Option<Style>,
    /// Whether this column is visible.
    visible: bool,
}

impl Column {
    /// Create a new column with the given ID and title.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            column_type: ColumnType::default(),
            align: ColumnAlign::default(),
            width: ColumnWidth::default(),
            sortable: true,
            value_style: None,
            header_style: None,
            visible: true,
        }
    }

    /// Set the column type for sorting.
    pub fn with_type(mut self, column_type: ColumnType) -> Self {
        self.column_type = column_type;
        self
    }

    /// Set the text alignment.
    pub fn with_align(mut self, align: ColumnAlign) -> Self {
        self.align = align;
        self
    }

    /// Set the width specification.
    pub fn with_width(mut self, width: ColumnWidth) -> Self {
        self.width = width;
        self
    }

    /// Set whether this column is sortable.
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Set the style for values in this column.
    pub fn with_value_style(mut self, style: Style) -> Self {
        self.value_style = Some(style);
        self
    }

    /// Set the style for the header cell.
    pub fn with_header_style(mut self, style: Style) -> Self {
        self.header_style = Some(style);
        self
    }

    /// Hide this column.
    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }

    /// Get the column ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the column title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get the column type.
    pub fn column_type(&self) -> ColumnType {
        self.column_type
    }

    /// Get the column alignment.
    pub fn align(&self) -> ColumnAlign {
        self.align
    }

    /// Check if this column is sortable.
    pub fn is_sortable(&self) -> bool {
        self.sortable
    }

    /// Check if this column is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }
}

// =============================================================================
// Selection Mode
// =============================================================================

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

// =============================================================================
// Table Configuration
// =============================================================================

/// Configuration for DataTable appearance and behavior.
#[derive(Debug, Clone)]
pub struct DataTableConfig {
    /// Style for the header row.
    pub header_style: Style,
    /// Style for header text.
    pub header_text_style: Style,
    /// Whether to show the header row.
    pub show_header: bool,
    /// Bottom margin after header (spacing).
    pub header_margin: u16,

    /// Style for normal rows.
    pub row_style: Style,
    /// Style for alternating rows (zebra striping).
    pub alt_row_style: Option<Style>,

    /// Style for the selected row when table is focused.
    pub focused_row_style: Style,
    /// Style for the selected column header when focused.
    pub focused_column_style: Style,
    /// Style for the selected cell when focused.
    pub focused_cell_style: Style,

    /// Style for the selected row when table is NOT focused.
    pub unfocused_row_style: Style,
    /// Style for the selected column when unfocused.
    pub unfocused_column_style: Style,
    /// Style for the selected cell when unfocused.
    pub unfocused_cell_style: Style,

    /// Style for border when focused.
    pub focused_border_style: Style,
    /// Style for border when unfocused.
    pub unfocused_border_style: Style,

    /// Whether vertical scrolling is enabled.
    pub scrollable: bool,

    /// Symbol for ascending sort.
    pub sort_asc_symbol: String,
    /// Symbol for descending sort.
    pub sort_desc_symbol: String,

    /// Symbol shown before selected row.
    pub highlight_symbol: String,

    /// Spacing between columns.
    pub column_spacing: u16,
}

impl Default for DataTableConfig {
    fn default() -> Self {
        Self {
            header_style: Style::default().add_modifier(Modifier::BOLD),
            header_text_style: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            show_header: true,
            header_margin: 1,

            row_style: Style::default().fg(Color::White),
            alt_row_style: None,

            focused_row_style: Style::default().bg(Color::Yellow).fg(Color::Black),
            focused_column_style: Style::default().bg(Color::Cyan).fg(Color::Black),
            focused_cell_style: Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),

            unfocused_row_style: Style::default().bg(Color::DarkGray).fg(Color::White),
            unfocused_column_style: Style::default().bg(Color::DarkGray).fg(Color::White),
            unfocused_cell_style: Style::default().bg(Color::DarkGray).fg(Color::White),

            focused_border_style: Style::default().fg(Color::Yellow),
            unfocused_border_style: Style::default().fg(Color::DarkGray),

            scrollable: true,

            sort_asc_symbol: "▲".to_string(),
            sort_desc_symbol: "▼".to_string(),

            highlight_symbol: "▶ ".to_string(),

            column_spacing: 1,
        }
    }
}

// =============================================================================
// Table Events
// =============================================================================

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

// =============================================================================
// TableRow Trait
// =============================================================================

/// Trait for types that can be displayed as table rows.
pub trait TableRow: Send {
    /// Get the value for a specific column by column ID.
    fn get_value(&self, column_id: &str) -> Option<Cow<'_, str>>;

    /// Get a unique identifier for this row (for tracking selection across sorts).
    fn row_id(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// Get a custom style override for this specific row.
    fn row_style(&self) -> Option<Style> {
        None
    }

    /// Get a custom style override for a specific cell.
    fn cell_style(&self, _column_id: &str) -> Option<Style> {
        None
    }
}

// =============================================================================
// SimpleRow Implementation
// =============================================================================

/// A simple row implementation using a HashMap.
#[derive(Debug, Clone, Default)]
pub struct SimpleRow {
    id: Option<String>,
    values: HashMap<String, String>,
    style: Option<Style>,
    cell_styles: HashMap<String, Style>,
}

impl SimpleRow {
    /// Create a new empty row.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the row ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set a value for a column.
    pub fn with_value(mut self, column_id: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(column_id.into(), value.into());
        self
    }

    /// Set the row style.
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Set a cell style for a specific column.
    pub fn with_cell_style(mut self, column_id: impl Into<String>, style: Style) -> Self {
        self.cell_styles.insert(column_id.into(), style);
        self
    }
}

impl TableRow for SimpleRow {
    fn get_value(&self, column_id: &str) -> Option<Cow<'_, str>> {
        self.values
            .get(column_id)
            .map(|s| Cow::Borrowed(s.as_str()))
    }

    fn row_id(&self) -> Option<Cow<'_, str>> {
        self.id.as_ref().map(|s| Cow::Borrowed(s.as_str()))
    }

    fn row_style(&self) -> Option<Style> {
        self.style
    }

    fn cell_style(&self, column_id: &str) -> Option<Style> {
        self.cell_styles.get(column_id).copied()
    }
}

// =============================================================================
// DataTable Widget
// =============================================================================

/// A data-driven table widget with sorting, selection, and scrolling.
pub struct DataTable<R: TableRow = SimpleRow> {
    /// Unique identifier for focus tracking.
    id: String,
    /// Column definitions in display order.
    columns: Vec<Column>,
    /// Row data.
    rows: Vec<R>,
    /// Currently selected row index (for Row/Cell selection modes).
    selected_row: Option<usize>,
    /// Currently selected column index (for Column/Cell selection modes).
    selected_column: Option<usize>,
    /// Current sort state.
    sort_state: Option<SortState>,
    /// Current filter string (case-insensitive match against any column).
    filter: String,
    /// Filtered and sorted row indices (maps display index to data index).
    sorted_indices: Vec<usize>,
    /// Selection mode.
    selection_mode: SelectionMode,
    /// Configuration.
    config: DataTableConfig,
    /// Optional title for the border.
    title: Option<String>,
    /// Vertical scroll offset (Cell for interior mutability during draw).
    scroll_offset: Cell<usize>,
    /// Cached visible height for page up/down calculations.
    last_visible_height: Cell<usize>,
    /// Pending events to be consumed by parent.
    events: Vec<DataTableEvent>,
}

impl<R: TableRow> DataTable<R> {
    /// Create a new DataTable with the given focus ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            selected_row: None,
            selected_column: None,
            sort_state: None,
            filter: String::new(),
            sorted_indices: Vec::new(),
            selection_mode: SelectionMode::Row,
            config: DataTableConfig::default(),
            title: None,
            scroll_offset: Cell::new(0),
            last_visible_height: Cell::new(10),
            events: Vec::new(),
        }
    }

    // === Builder Methods ===

    /// Set the columns.
    pub fn with_columns(mut self, columns: impl IntoIterator<Item = Column>) -> Self {
        self.columns = columns.into_iter().collect();
        self
    }

    /// Set the rows.
    pub fn with_rows(mut self, rows: impl IntoIterator<Item = R>) -> Self {
        self.rows = rows.into_iter().collect();
        self.rebuild_sorted_indices();
        self
    }

    /// Set the selection mode.
    pub fn with_selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: DataTableConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set whether scrolling is enabled.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.config.scrollable = scrollable;
        self
    }

    /// Get the table's ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    // === Data Management ===

    /// Set the row data.
    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = R>) {
        self.rows = rows.into_iter().collect();
        self.rebuild_sorted_indices();
        // Clamp selection to valid range
        if let Some(row) = self.selected_row {
            if row >= self.rows.len() {
                self.selected_row = if self.rows.is_empty() {
                    None
                } else {
                    Some(self.rows.len() - 1)
                };
            }
        }
    }

    /// Add a single row.
    pub fn push_row(&mut self, row: R) {
        self.rows.push(row);
        self.rebuild_sorted_indices();
    }

    /// Clear all rows.
    pub fn clear_rows(&mut self) {
        self.rows.clear();
        self.sorted_indices.clear();
        self.selected_row = None;
    }

    /// Get the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get a reference to a row by data index.
    pub fn get_row(&self, index: usize) -> Option<&R> {
        self.rows.get(index)
    }

    /// Get the currently selected row data index.
    pub fn selected_row_index(&self) -> Option<usize> {
        self.selected_row
            .and_then(|display_idx| self.sorted_indices.get(display_idx).copied())
    }

    /// Get a reference to the currently selected row.
    pub fn selected_row(&self) -> Option<&R> {
        self.selected_row_index().and_then(|i| self.rows.get(i))
    }

    /// Get the currently selected column index.
    pub fn selected_column_index(&self) -> Option<usize> {
        self.selected_column
    }

    // === Selection ===

    /// Select a row by display index.
    pub fn select_row(&mut self, index: usize) {
        if index < self.sorted_indices.len() {
            self.selected_row = Some(index);
            let data_idx = self.sorted_indices[index];
            self.emit(DataTableEvent::RowSelected(data_idx));
            self.emit(DataTableEvent::SelectionChanged {
                row: Some(data_idx),
                column: self.selected_column,
            });
        }
    }

    /// Select the next row.
    pub fn select_next_row(&mut self) {
        if self.sorted_indices.is_empty() {
            return;
        }
        let new_idx = match self.selected_row {
            Some(idx) => (idx + 1).min(self.sorted_indices.len() - 1),
            None => 0,
        };
        self.select_row(new_idx);
    }

    /// Select the previous row.
    pub fn select_prev_row(&mut self) {
        if self.sorted_indices.is_empty() {
            return;
        }
        let new_idx = match self.selected_row {
            Some(idx) => idx.saturating_sub(1),
            None => 0,
        };
        self.select_row(new_idx);
    }

    /// Move selection to the first row.
    pub fn select_first_row(&mut self) {
        if !self.sorted_indices.is_empty() {
            self.select_row(0);
        }
    }

    /// Move selection to the last row.
    pub fn select_last_row(&mut self) {
        if !self.sorted_indices.is_empty() {
            self.select_row(self.sorted_indices.len() - 1);
        }
    }

    /// Select a column by index.
    pub fn select_column(&mut self, index: usize) {
        let visible_count = self.visible_columns().len();
        if index < visible_count {
            self.selected_column = Some(index);
            self.emit(DataTableEvent::SelectionChanged {
                row: self.selected_row_index(),
                column: Some(index),
            });
        }
    }

    /// Select the next column.
    pub fn select_next_column(&mut self) {
        let visible_count = self.visible_columns().len();
        if visible_count == 0 {
            return;
        }
        let new_idx = match self.selected_column {
            Some(idx) => (idx + 1).min(visible_count - 1),
            None => 0,
        };
        self.select_column(new_idx);
    }

    /// Select the previous column.
    pub fn select_prev_column(&mut self) {
        let visible_count = self.visible_columns().len();
        if visible_count == 0 {
            return;
        }
        let new_idx = match self.selected_column {
            Some(idx) => idx.saturating_sub(1),
            None => 0,
        };
        self.select_column(new_idx);
    }

    // === Sorting ===

    /// Sort by a column ID.
    pub fn sort_by(&mut self, column_id: &str, direction: SortDirection) {
        // Find the column
        let column = match self.columns.iter().find(|c| c.id == column_id) {
            Some(c) if c.sortable => c,
            _ => return,
        };

        // Remember selected row's data index to restore after sort
        let selected_data_idx = self.selected_row_index();

        // Sort the indices based on column values
        let column_type = column.column_type;
        let col_id = column_id.to_string();

        self.sorted_indices.sort_by(|&a, &b| {
            let val_a = self.rows[a].get_value(&col_id);
            let val_b = self.rows[b].get_value(&col_id);

            let cmp = compare_values(val_a.as_deref(), val_b.as_deref(), column_type);

            match direction {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });

        self.sort_state = Some(SortState {
            column_id: column_id.to_string(),
            direction,
        });

        // Restore selection to the same data row
        if let Some(data_idx) = selected_data_idx {
            self.selected_row = self.sorted_indices.iter().position(|&i| i == data_idx);
        }

        self.emit(DataTableEvent::ColumnSorted {
            column_id: column_id.to_string(),
            direction,
        });
    }

    /// Toggle sort on a column (or set initial sort).
    pub fn toggle_sort(&mut self, column_id: &str) {
        let new_direction = match &self.sort_state {
            Some(state) if state.column_id == column_id => state.direction.toggle(),
            _ => SortDirection::Ascending,
        };
        self.sort_by(column_id, new_direction);
    }

    /// Clear sorting (return to original order).
    pub fn clear_sort(&mut self) {
        self.sort_state = None;
        self.rebuild_sorted_indices();
    }

    // === Filtering ===

    /// Set the filter string. Rows are shown if any column contains the filter string (case-insensitive).
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.rebuild_sorted_indices();
        // Reset selection if current selection is no longer visible
        if let Some(selected) = self.selected_row {
            if selected >= self.sorted_indices.len() {
                self.selected_row = if self.sorted_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        self.scroll_offset.set(0);
    }

    /// Get the current filter string.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Clear the filter.
    pub fn clear_filter(&mut self) {
        self.set_filter("");
    }

    /// Check if a row matches the current filter.
    fn row_matches_filter(&self, row: &R) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        let filter_lower = self.filter.to_lowercase();
        for col in &self.columns {
            if let Some(value) = row.get_value(&col.id) {
                if value.to_lowercase().contains(&filter_lower) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the number of filtered rows (visible rows after filter).
    pub fn filtered_row_count(&self) -> usize {
        self.sorted_indices.len()
    }

    // === Scrolling ===

    /// Page down (move selection by visible area height).
    pub fn page_down(&mut self) {
        if self.sorted_indices.is_empty() {
            return;
        }
        let visible_rows = self.last_visible_height.get();
        let new_idx = match self.selected_row {
            Some(idx) => (idx + visible_rows).min(self.sorted_indices.len() - 1),
            None => visible_rows.min(self.sorted_indices.len() - 1),
        };
        self.select_row(new_idx);
    }

    /// Page up (move selection by visible area height).
    pub fn page_up(&mut self) {
        if self.sorted_indices.is_empty() {
            return;
        }
        let visible_rows = self.last_visible_height.get();
        let new_idx = match self.selected_row {
            Some(idx) => idx.saturating_sub(visible_rows),
            None => 0,
        };
        self.select_row(new_idx);
    }

    // === Internal Helpers ===

    fn rebuild_sorted_indices(&mut self) {
        // Start with all indices, then filter
        self.sorted_indices = (0..self.rows.len())
            .filter(|&i| self.row_matches_filter(&self.rows[i]))
            .collect();
        // Re-apply sort if one is active
        if let Some(state) = self.sort_state.clone() {
            self.sort_by(&state.column_id, state.direction);
        }
    }

    fn emit(&mut self, event: DataTableEvent) {
        self.events.push(event);
    }

    fn visible_columns(&self) -> Vec<&Column> {
        self.columns.iter().filter(|c| c.visible).collect()
    }

    /// Calculate visible row range based on area height and scroll offset.
    fn visible_row_range(&self, area_height: u16) -> std::ops::Range<usize> {
        let header_height = if self.config.show_header {
            1 + self.config.header_margin as usize
        } else {
            0
        };
        // Account for borders (top and bottom)
        let border_height = 2;
        let available_height = (area_height as usize).saturating_sub(header_height + border_height);
        self.last_visible_height.set(available_height);

        let scroll = self.scroll_offset.get();
        let start = scroll;
        let end = (start + available_height).min(self.sorted_indices.len());
        start..end
    }

    /// Ensure the selected row is visible, adjusting scroll if needed.
    fn ensure_selection_visible(&self, area_height: u16) {
        if !self.config.scrollable {
            return;
        }

        let Some(selected) = self.selected_row else {
            return;
        };

        let header_height = if self.config.show_header {
            1 + self.config.header_margin as usize
        } else {
            0
        };
        let border_height = 2;
        let visible_rows = (area_height as usize).saturating_sub(header_height + border_height);

        if visible_rows == 0 {
            return;
        }

        let mut scroll = self.scroll_offset.get();

        if selected < scroll {
            scroll = selected;
        } else if selected >= scroll + visible_rows {
            scroll = selected.saturating_sub(visible_rows - 1);
        }

        self.scroll_offset.set(scroll);
    }
}

// =============================================================================
// Control Implementation
// =============================================================================

impl<R: TableRow + 'static> Control for DataTable<R> {
    type Event = DataTableEvent;

    fn draw(&self, frame: &mut Frame, area: Rect, focused: bool) {
        // Ensure selection is visible
        self.ensure_selection_visible(area.height);

        // Determine styles based on focus
        let border_style = if focused {
            self.config.focused_border_style
        } else {
            self.config.unfocused_border_style
        };

        // Create block with optional title
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        if let Some(ref title) = self.title {
            block = block.title(format!(" {} ", title));
        }

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Get visible columns
        let visible_cols = self.visible_columns();
        if visible_cols.is_empty() {
            return;
        }

        // Calculate column widths
        let widths: Vec<Constraint> = visible_cols
            .iter()
            .map(|col| col.width.to_constraint())
            .collect();

        // Build header row with sort indicators
        let header_cells: Vec<RatatuiCell> = visible_cols
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let mut title = col.title.clone();

                // Add sort indicator
                if let Some(ref state) = self.sort_state {
                    if state.column_id == col.id {
                        let indicator = match state.direction {
                            SortDirection::Ascending => &self.config.sort_asc_symbol,
                            SortDirection::Descending => &self.config.sort_desc_symbol,
                        };
                        title = format!("{} {}", title, indicator);
                    }
                }

                // Apply column highlight if selected
                let style = if self.selection_mode == SelectionMode::Column
                    || self.selection_mode == SelectionMode::Cell
                {
                    if self.selected_column == Some(idx) {
                        if focused {
                            self.config.focused_column_style
                        } else {
                            self.config.unfocused_column_style
                        }
                    } else {
                        col.header_style.unwrap_or(self.config.header_text_style)
                    }
                } else {
                    col.header_style.unwrap_or(self.config.header_text_style)
                };

                RatatuiCell::from(Line::from(title)).style(style)
            })
            .collect();

        let header = Row::new(header_cells)
            .style(self.config.header_style)
            .bottom_margin(self.config.header_margin);

        // Get visible row range
        let row_range = self.visible_row_range(area.height);

        // Build data rows
        let rows: Vec<Row> = row_range
            .clone()
            .map(|display_idx| {
                let data_idx = self.sorted_indices[display_idx];
                let row_data = &self.rows[data_idx];
                let is_selected = self.selected_row == Some(display_idx);

                // Determine row base style
                let row_base_style = row_data.row_style().unwrap_or_else(|| {
                    self.config
                        .alt_row_style
                        .filter(|_| display_idx % 2 == 1)
                        .unwrap_or(self.config.row_style)
                });

                // Apply selection highlight
                let row_style = if is_selected
                    && (self.selection_mode == SelectionMode::Row
                        || self.selection_mode == SelectionMode::Cell)
                {
                    if focused {
                        self.config.focused_row_style
                    } else {
                        self.config.unfocused_row_style
                    }
                } else {
                    row_base_style
                };

                // Build cells
                let cells: Vec<RatatuiCell> = visible_cols
                    .iter()
                    .enumerate()
                    .map(|(col_idx, col)| {
                        let raw_value = row_data
                            .get_value(&col.id)
                            .map(|v| v.into_owned())
                            .unwrap_or_default();

                        // Determine cell style (cell > row > column)
                        let cell_style = if self.selection_mode == SelectionMode::Cell
                            && is_selected
                            && self.selected_column == Some(col_idx)
                        {
                            if focused {
                                self.config.focused_cell_style
                            } else {
                                self.config.unfocused_cell_style
                            }
                        } else if is_selected
                            && (self.selection_mode == SelectionMode::Row
                                || self.selection_mode == SelectionMode::Cell)
                        {
                            row_style
                        } else {
                            row_data
                                .cell_style(&col.id)
                                .or(col.value_style)
                                .unwrap_or(row_style)
                        };

                        RatatuiCell::from(Line::from(raw_value)).style(cell_style)
                    })
                    .collect();

                Row::new(cells)
            })
            .collect();

        // Create the ratatui Table
        let mut table = Table::new(rows, widths).column_spacing(self.config.column_spacing);

        if self.config.show_header {
            table = table.header(header);
        }

        frame.render_widget(table, inner);
    }

    fn handle_event(&mut self, event: &Event) -> EventResult {
        if let Event::Key(key) = event {
            match key.code {
                // Row navigation
                KeyCode::Up | KeyCode::Char('k') => match self.selection_mode {
                    SelectionMode::Row | SelectionMode::Cell => {
                        self.select_prev_row();
                        EventResult::Handled
                    }
                    _ => EventResult::Unhandled,
                },
                KeyCode::Down | KeyCode::Char('j') => match self.selection_mode {
                    SelectionMode::Row | SelectionMode::Cell => {
                        self.select_next_row();
                        EventResult::Handled
                    }
                    _ => EventResult::Unhandled,
                },

                // Column navigation
                KeyCode::Left | KeyCode::Char('h') => match self.selection_mode {
                    SelectionMode::Column | SelectionMode::Cell => {
                        self.select_prev_column();
                        EventResult::Handled
                    }
                    _ => EventResult::Unhandled,
                },
                KeyCode::Right | KeyCode::Char('l') => match self.selection_mode {
                    SelectionMode::Column | SelectionMode::Cell => {
                        self.select_next_column();
                        EventResult::Handled
                    }
                    _ => EventResult::Unhandled,
                },

                // Page navigation
                KeyCode::PageDown => {
                    self.page_down();
                    EventResult::Handled
                }
                KeyCode::PageUp => {
                    self.page_up();
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select_first_row();
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select_last_row();
                    EventResult::Handled
                }

                // Activation
                KeyCode::Enter => {
                    if let Some(idx) = self.selected_row {
                        if idx < self.sorted_indices.len() {
                            self.emit(DataTableEvent::RowActivated(self.sorted_indices[idx]));
                        }
                    }
                    EventResult::Handled
                }

                // Sort toggle
                KeyCode::Char('s')
                    if self.selection_mode == SelectionMode::Column
                        || self.selection_mode == SelectionMode::Cell =>
                {
                    if let Some(col_idx) = self.selected_column {
                        let visible_cols = self.visible_columns();
                        if let Some(col) = visible_cols.get(col_idx) {
                            let col_id = col.id.clone();
                            self.toggle_sort(&col_id);
                        }
                    }
                    EventResult::Handled
                }

                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }

    fn take_events(&mut self) -> Vec<DataTableEvent> {
        std::mem::take(&mut self.events)
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    fn on_focus(&mut self) {
        self.emit(DataTableEvent::FocusGained);
        // Select first row if nothing selected
        if self.selected_row.is_none()
            && !self.rows.is_empty()
            && self.selection_mode != SelectionMode::None
        {
            self.selected_row = Some(0);
        }
        // Select first column if in column/cell mode
        if self.selected_column.is_none()
            && (self.selection_mode == SelectionMode::Column
                || self.selection_mode == SelectionMode::Cell)
        {
            self.selected_column = Some(0);
        }
    }

    fn on_blur(&mut self) {
        self.emit(DataTableEvent::FocusLost);
    }
}

// =============================================================================
// Value Comparison
// =============================================================================

/// Compare two optional string values based on column type.
fn compare_values(a: Option<&str>, b: Option<&str>, column_type: ColumnType) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => match column_type {
            ColumnType::Text => a.cmp(b),

            ColumnType::Integer => {
                let parsed_a = a.trim().replace(",", "").parse::<i64>().ok();
                let parsed_b = b.trim().replace(",", "").parse::<i64>().ok();
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Float => {
                let parsed_a = a.trim().replace(",", "").parse::<f64>().ok();
                let parsed_b = b.trim().replace(",", "").parse::<f64>().ok();
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Percent => {
                // Parse "45%" or "0.45" style percentages
                let parse_percent = |s: &str| -> Option<f64> {
                    let s = s.trim();
                    if let Some(stripped) = s.strip_suffix('%') {
                        stripped.trim().parse::<f64>().ok()
                    } else {
                        s.parse::<f64>().ok().map(|v| v * 100.0)
                    }
                };
                let parsed_a = parse_percent(a);
                let parsed_b = parse_percent(b);
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Size => {
                // Parse "1.5 GB", "500 KB", etc.
                let parse_size = |s: &str| -> Option<u64> {
                    let s = s.trim().to_uppercase();
                    let multipliers = [
                        ("TB", 1024u64.pow(4)),
                        ("GB", 1024u64.pow(3)),
                        ("MB", 1024u64.pow(2)),
                        ("KB", 1024u64),
                        ("B", 1u64),
                    ];
                    for (suffix, mult) in multipliers {
                        if s.ends_with(suffix) {
                            let num_str = s[..s.len() - suffix.len()].trim();
                            if let Ok(num) = num_str.parse::<f64>() {
                                return Some((num * mult as f64) as u64);
                            }
                        }
                    }
                    // Try parsing as raw bytes
                    s.parse::<u64>().ok()
                };
                let parsed_a = parse_size(a);
                let parsed_b = parse_size(b);
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::DateTime | ColumnType::Duration => {
                // Fall back to lexicographic comparison
                // ISO 8601 dates sort correctly lexicographically
                a.cmp(b)
            }
        },
    }
}
