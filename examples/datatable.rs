//! DataTable example demonstrating the data-driven table widget.
//!
//! This example shows:
//! - Creating columns with different types and alignments
//! - Populating rows with data
//! - Row selection and navigation
//! - Sorting by columns
//! - Filtering with a search textbox
//! - Focus-aware styling
//!
//! Controls:
//! - Up/Down or j/k: Navigate rows
//! - Left/Right or h/l: Navigate columns (in Cell mode)
//! - s: Toggle sort on selected column
//! - Enter: Activate selected row
//! - /: Activate filter input (type to filter both tables)
//! - Escape: Clear filter / exit filter mode
//! - Tab: Switch between tables
//! - q/Ctrl+C: Quit

#[path = "_common/mod.rs"]
mod common;
use clap::Parser;
use common::Args;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Column, ColumnAlign, ColumnType, ColumnWidth, Component,
    Control, DataTable, DataTableConfig, DataTableEvent, DrawContext, Event, EventResult, KeyCode,
    MainUi, SelectionMode, SimpleRow, TextBox, TextBoxConfig, TextBoxEvent,
};

// =============================================================================
// Main Application
// =============================================================================

struct DataTableApp {
    /// File list table
    files_table: DataTable<SimpleRow>,
    /// Process list table
    processes_table: DataTable<SimpleRow>,
    /// Filter input textbox
    filter_input: TextBox,
    /// Which table is focused (0 = files, 1 = processes)
    focused_table: usize,
    /// Whether filter input is active
    filter_active: bool,
    /// Status message
    status: String,
}

impl DataTableApp {
    fn new() -> Self {
        // Create file list table
        let file_columns = vec![
            Column::new("name", "Name")
                .with_width(ColumnWidth::Min(20))
                .with_align(ColumnAlign::Left),
            Column::new("size", "Size")
                .with_type(ColumnType::Size)
                .with_width(ColumnWidth::Fixed(12))
                .with_align(ColumnAlign::Right)
                .with_value_style(Style::default().fg(Color::Cyan)),
            Column::new("modified", "Modified")
                .with_type(ColumnType::DateTime)
                .with_width(ColumnWidth::Fixed(20))
                .with_align(ColumnAlign::Left)
                .with_value_style(Style::default().add_modifier(Modifier::DIM)),
            Column::new("type", "Type")
                .with_width(ColumnWidth::Fixed(10))
                .sortable(false),
        ];

        let file_rows = vec![
            SimpleRow::new()
                .with_value("name", "document.pdf")
                .with_value("size", "2.5 MB")
                .with_value("modified", "2024-01-15 10:30")
                .with_value("type", "PDF"),
            SimpleRow::new()
                .with_value("name", "photo.jpg")
                .with_value("size", "4.2 MB")
                .with_value("modified", "2024-01-14 15:45")
                .with_value("type", "Image"),
            SimpleRow::new()
                .with_value("name", "video.mp4")
                .with_value("size", "1.8 GB")
                .with_value("modified", "2024-01-13 09:20")
                .with_value("type", "Video"),
            SimpleRow::new()
                .with_value("name", "notes.txt")
                .with_value("size", "12 KB")
                .with_value("modified", "2024-01-16 08:00")
                .with_value("type", "Text"),
            SimpleRow::new()
                .with_value("name", "archive.zip")
                .with_value("size", "156 MB")
                .with_value("modified", "2024-01-10 14:30")
                .with_value("type", "Archive"),
            SimpleRow::new()
                .with_value("name", "spreadsheet.xlsx")
                .with_value("size", "890 KB")
                .with_value("modified", "2024-01-12 11:15")
                .with_value("type", "Excel"),
            SimpleRow::new()
                .with_value("name", "presentation.pptx")
                .with_value("size", "15 MB")
                .with_value("modified", "2024-01-11 16:45")
                .with_value("type", "PowerPoint"),
            SimpleRow::new()
                .with_value("name", "database.db")
                .with_value("size", "450 MB")
                .with_value("modified", "2024-01-09 13:00")
                .with_value("type", "Database"),
        ];

        let files_table = DataTable::new("files")
            .with_title("Files")
            .with_columns(file_columns)
            .with_rows(file_rows)
            .with_selection_mode(SelectionMode::Row);

        // Create process list table
        let process_columns = vec![
            Column::new("pid", "PID")
                .with_type(ColumnType::Integer)
                .with_width(ColumnWidth::Fixed(8))
                .with_align(ColumnAlign::Right),
            Column::new("name", "Process")
                .with_width(ColumnWidth::Min(15))
                .with_align(ColumnAlign::Left),
            Column::new("cpu", "CPU %")
                .with_type(ColumnType::Percent)
                .with_width(ColumnWidth::Fixed(8))
                .with_align(ColumnAlign::Right)
                .with_value_style(Style::default().fg(Color::Yellow)),
            Column::new("memory", "Memory")
                .with_type(ColumnType::Size)
                .with_width(ColumnWidth::Fixed(10))
                .with_align(ColumnAlign::Right)
                .with_value_style(Style::default().fg(Color::Green)),
            Column::new("status", "Status")
                .with_width(ColumnWidth::Fixed(10))
                .sortable(false),
        ];

        let process_rows = vec![
            SimpleRow::new()
                .with_value("pid", "1234")
                .with_value("name", "firefox")
                .with_value("cpu", "12.5%")
                .with_value("memory", "1.2 GB")
                .with_value("status", "Running"),
            SimpleRow::new()
                .with_value("pid", "5678")
                .with_value("name", "code")
                .with_value("cpu", "8.3%")
                .with_value("memory", "890 MB")
                .with_value("status", "Running"),
            SimpleRow::new()
                .with_value("pid", "9012")
                .with_value("name", "slack")
                .with_value("cpu", "3.1%")
                .with_value("memory", "456 MB")
                .with_value("status", "Running"),
            SimpleRow::new()
                .with_value("pid", "3456")
                .with_value("name", "docker")
                .with_value("cpu", "0.5%")
                .with_value("memory", "234 MB")
                .with_value("status", "Running"),
            SimpleRow::new()
                .with_value("pid", "7890")
                .with_value("name", "postgres")
                .with_value("cpu", "2.1%")
                .with_value("memory", "512 MB")
                .with_value("status", "Running"),
            SimpleRow::new()
                .with_value("pid", "2345")
                .with_value("name", "node")
                .with_value("cpu", "15.2%")
                .with_value("memory", "320 MB")
                .with_value("status", "Running"),
        ];

        let process_config = DataTableConfig {
            alt_row_style: Some(Style::default().fg(Color::Gray)),
            ..Default::default()
        };

        let processes_table = DataTable::new("processes")
            .with_title("Processes")
            .with_columns(process_columns)
            .with_rows(process_rows)
            .with_selection_mode(SelectionMode::Cell)
            .with_config(process_config);

        // Create filter textbox
        let filter_config = TextBoxConfig {
            focused_border_style: Style::default().fg(Color::Yellow),
            unfocused_border_style: Style::default().fg(Color::DarkGray),
            placeholder_style: Style::default().fg(Color::DarkGray),
            ..Default::default()
        };
        let filter_input = TextBox::new("filter")
            .with_placeholder("Type to filter...")
            .with_title("Filter (Esc to close)")
            .with_config(filter_config);

        Self {
            files_table,
            processes_table,
            filter_input,
            focused_table: 0,
            filter_active: false,
            status: "Ready. Press '/' to filter, Tab to switch tables.".to_string(),
        }
    }

    fn focused_table_mut(&mut self) -> &mut DataTable<SimpleRow> {
        if self.focused_table == 0 {
            &mut self.files_table
        } else {
            &mut self.processes_table
        }
    }

    fn apply_filter(&mut self, filter: &str) {
        self.files_table.set_filter(filter);
        self.processes_table.set_filter(filter);
        let files_count = self.files_table.filtered_row_count();
        let procs_count = self.processes_table.filtered_row_count();
        if filter.is_empty() {
            self.status = "Filter cleared.".to_string();
        } else {
            self.status = format!(
                "Filter '{}': {} files, {} processes",
                filter, files_count, procs_count
            );
        }
    }

    fn process_table_events(&mut self, events: Vec<DataTableEvent>) {
        for event in events {
            match event {
                DataTableEvent::RowActivated(idx) => {
                    self.status = format!("Activated row {}", idx);
                }
                DataTableEvent::ColumnSorted {
                    column_id,
                    direction,
                } => {
                    self.status = format!("Sorted by '{}' {:?}", column_id, direction);
                }
                DataTableEvent::RowSelected(idx) => {
                    self.status = format!("Selected row {}", idx);
                }
                _ => {}
            }
        }
    }

    fn process_filter_events(&mut self) {
        for event in self.filter_input.take_events() {
            match event {
                TextBoxEvent::Changed(text) => {
                    self.apply_filter(&text);
                }
                TextBoxEvent::Submit(_) => {
                    // Exit filter mode on Enter
                    self.filter_active = false;
                    self.filter_input.on_blur();
                }
                _ => {}
            }
        }
    }
}

impl Component for DataTableApp {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        // Split into header, main content, filter, and footer
        let outer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Tables
                Constraint::Length(3), // Filter input
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Header
        let header = Paragraph::new("DataTable Widget Demo")
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(header, outer_chunks[0]);

        // Split main area into two tables side by side
        let table_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(outer_chunks[1]);

        // Draw tables with focus state (unfocused if filter is active)
        let files_focused = !self.filter_active && self.focused_table == 0;
        let procs_focused = !self.filter_active && self.focused_table == 1;
        self.files_table.draw(frame, table_chunks[0], files_focused);
        self.processes_table
            .draw(frame, table_chunks[1], procs_focused);

        // Filter input
        self.filter_input
            .draw(frame, outer_chunks[2], self.filter_active);

        // Footer with status and controls
        let mode_str = if self.focused_table == 0 {
            "Row"
        } else {
            "Cell"
        };
        let footer_text = format!(
            "{} | Mode: {} | /: filter | Tab: switch | s: sort | q: quit",
            self.status, mode_str
        );
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, outer_chunks[3]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Quit on Ctrl+C or Ctrl+Q
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Handle filter mode
        if self.filter_active {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Esc => {
                        // Exit filter mode
                        self.filter_active = false;
                        self.filter_input.on_blur();
                        self.status = "Filter mode exited.".to_string();
                        return EventResult::Handled;
                    }
                    _ => {
                        // Forward to filter input
                        let result = self.filter_input.handle_event(event);
                        self.process_filter_events();
                        return result;
                    }
                }
            }
            return EventResult::Unhandled;
        }

        // Normal mode
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                // Activate filter with '/'
                KeyCode::Char('/') => {
                    self.filter_active = true;
                    self.filter_input.on_focus();
                    self.status = "Type to filter both tables...".to_string();
                    EventResult::Handled
                }
                // Clear filter with Escape
                KeyCode::Esc => {
                    self.files_table.clear_filter();
                    self.processes_table.clear_filter();
                    self.filter_input.clear();
                    self.status = "Filter cleared.".to_string();
                    EventResult::Handled
                }
                // Tab to switch between tables
                KeyCode::Tab => {
                    // Blur current table
                    self.focused_table_mut().on_blur();
                    // Switch focus
                    self.focused_table = 1 - self.focused_table;
                    // Focus new table
                    self.focused_table_mut().on_focus();
                    self.status = if self.focused_table == 0 {
                        "Files table focused (Row mode)".to_string()
                    } else {
                        "Processes table focused (Cell mode)".to_string()
                    };
                    EventResult::Handled
                }
                // Forward other keys to focused table
                _ => {
                    let result = self.focused_table_mut().handle_event(event);
                    let events = self.focused_table_mut().take_events();
                    self.process_table_events(events);
                    result
                }
            }
        } else {
            EventResult::Unhandled
        }
    }
}

impl MainUi for DataTableApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    let app = AppBuilder::new()
        .main_ui(DataTableApp::new())
        .mouse_capture(false)
        .with_log_receiver(log_rx)
        .build()?;

    app.run().await?;

    Ok(())
}
