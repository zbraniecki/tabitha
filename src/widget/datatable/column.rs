//! Column definition for DataTable.
//!
//! This module contains the Column struct and its configuration options.

use ratatui::style::Style;

use super::types::{ColumnAlign, ColumnType, ColumnWidth};

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

    /// Get the value style if set.
    pub(crate) fn value_style(&self) -> Option<Style> {
        self.value_style
    }

    /// Get the header style if set.
    pub(crate) fn header_style(&self) -> Option<Style> {
        self.header_style
    }

    /// Get the width specification.
    pub(crate) fn width(&self) -> &ColumnWidth {
        &self.width
    }
}
