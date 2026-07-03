//! Rendering and event handling for DataTable.
//!
//! This module implements the Control trait for DataTable.

use crossterm::event::KeyEventKind;
use ratatui::{
    layout::Rect,
    text::Line,
    widgets::{Block, Borders, Cell as RatatuiCell, Row, Table},
    Frame,
};

use crate::event::Event;
use crate::focus::EventResult;
use crate::widget::Control;
use crate::KeyCode;

use super::events::DataTableEvent;
use super::row::TableRow;
use super::table::DataTable;
use super::types::SelectionMode;
use super::types::SortDirection;

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
        let widths: Vec<ratatui::layout::Constraint> = visible_cols
            .iter()
            .map(|col| col.width().to_constraint())
            .collect();

        // Build header row with sort indicators
        let header_cells: Vec<RatatuiCell> = visible_cols
            .iter()
            .enumerate()
            .map(|(idx, col)| {
                let mut title = col.title().to_string();

                // Add sort indicator
                if let Some(ref state) = self.sort_state {
                    if state.column_id == col.id() {
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
                        col.header_style().unwrap_or(self.config.header_text_style)
                    }
                } else {
                    col.header_style().unwrap_or(self.config.header_text_style)
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
                let row_base_style = if let Some(style) = row_data.row_style() {
                    style
                } else if let Some(style) = self.config.alt_row_style {
                    if display_idx % 2 == 1 {
                        style
                    } else {
                        self.config.row_style
                    }
                } else {
                    self.config.row_style
                };

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
                            .get_value(col.id())
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
                                .cell_style(col.id())
                                .or(col.value_style())
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
        if let Event::Key(key) = event && key.kind == KeyEventKind::Press {
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
                            let col_id = col.id().to_string();
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
