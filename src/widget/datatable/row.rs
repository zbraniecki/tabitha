//! Row trait and implementations for DataTable.
//!
//! This module contains the TableRow trait and SimpleRow implementation.

use std::borrow::Cow;
use std::collections::HashMap;

use ratatui::style::Style;

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
