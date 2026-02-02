//! Configuration for DataTable.
//!
//! This module contains the DataTableConfig struct for table appearance and behavior.

use ratatui::style::{Color, Modifier, Style};

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
