//! Core DataTable implementation.
//!
//! This module contains the DataTable struct with its data management,
//! selection, sorting, filtering, and scrolling capabilities.

use std::cell::Cell;

use super::column::Column;
use super::config::DataTableConfig;
use super::events::DataTableEvent;
use super::row::{SimpleRow, TableRow};
use super::sorting::compare_values;
use super::types::{SelectionMode, SortDirection, SortState};

/// A data-driven table widget with sorting, selection, and scrolling.
pub struct DataTable<R: TableRow = SimpleRow> {
    /// Unique identifier for focus tracking.
    pub(crate) id: String,
    /// Column definitions in display order.
    pub(crate) columns: Vec<Column>,
    /// Row data.
    pub(crate) rows: Vec<R>,
    /// Currently selected row index (for Row/Cell selection modes).
    pub(crate) selected_row: Option<usize>,
    /// Currently selected column index (for Column/Cell selection modes).
    pub(crate) selected_column: Option<usize>,
    /// Current sort state.
    pub(crate) sort_state: Option<SortState>,
    /// Current filter string (case-insensitive match against any column).
    pub(crate) filter: String,
    /// Filtered and sorted row indices (maps display index to data index).
    pub(crate) sorted_indices: Vec<usize>,
    /// Selection mode.
    pub(crate) selection_mode: SelectionMode,
    /// Configuration.
    pub(crate) config: DataTableConfig,
    /// Optional title for the border.
    pub(crate) title: Option<String>,
    /// Vertical scroll offset (Cell for interior mutability during draw).
    pub(crate) scroll_offset: Cell<usize>,
    /// Cached visible height for page up/down calculations.
    pub(crate) last_visible_height: Cell<usize>,
    /// Pending events to be consumed by parent.
    pub(crate) events: Vec<DataTableEvent>,
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
        let column = match self.columns.iter().find(|c| c.id() == column_id) {
            Some(c) if c.is_sortable() => c,
            _ => return,
        };

        // Remember selected row's data index to restore after sort
        let selected_data_idx = self.selected_row_index();

        // Sort the indices based on column values
        let column_type = column.column_type();
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
            if let Some(value) = row.get_value(col.id()) {
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

    pub(crate) fn rebuild_sorted_indices(&mut self) {
        // Start with all indices, then filter
        self.sorted_indices = (0..self.rows.len())
            .filter(|&i| self.row_matches_filter(&self.rows[i]))
            .collect();
        // Re-apply sort if one is active
        if let Some(state) = self.sort_state.clone() {
            self.sort_by(&state.column_id, state.direction);
        }
    }

    pub(crate) fn emit(&mut self, event: DataTableEvent) {
        self.events.push(event);
    }

    pub(crate) fn visible_columns(&self) -> Vec<&Column> {
        self.columns.iter().filter(|c| c.is_visible()).collect()
    }

    /// Calculate visible row range based on area height and scroll offset.
    pub(crate) fn visible_row_range(&self, area_height: u16) -> std::ops::Range<usize> {
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
    pub(crate) fn ensure_selection_visible(&self, area_height: u16) {
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
