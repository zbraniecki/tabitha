//! Focus tables example demonstrating focus navigation in tabitha.
//!
//! This example shows:
//! - Two tabs with the second tab containing two side-by-side tables
//! - Up/Down to navigate rows within the focused table
//! - Left/Right to switch focus between tables
//! - Active table has selected row highlighted in an active color
//! - Inactive table has selected row shown in a dimmed color
//!
//! Controls:
//! - Tab: Switch between tabs
//! - Left/Right: Switch focus between tables (in Data tab)
//! - Up/Down: Navigate rows in the focused table
//! - q/Ctrl+C: Quit

#[path = "_common/mod.rs"]
mod common;
use clap::Parser;
use common::Args;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};
use tabitha::widget::{TabBar, TabContent};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, HasTabs, KeyCode,
    MainUi,
};

// =============================================================================
// Welcome Tab
// =============================================================================

struct WelcomeTab;

impl Component for WelcomeTab {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let content = Paragraph::new(
            "Welcome to the Focus Tables Example!\n\n\
             This example demonstrates focus navigation between widgets.\n\n\
             Press Tab to switch to the 'Data' tab, which contains\n\
             two tables side by side.\n\n\
             In the Data tab:\n\
             • Left/Right arrows: Switch focus between tables\n\
             • Up/Down arrows: Navigate rows in the focused table\n\
             • The focused table shows selected row in yellow\n\
             • The unfocused table shows selected row in gray",
        )
        .style(Style::default().fg(Color::White));
        frame.render_widget(content, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }
}

// =============================================================================
// Data Tab with Two Tables
// =============================================================================

/// A simple table widget that can be focused.
struct FocusableTable {
    title: String,
    items: Vec<(String, String, String)>,
    selected: usize,
}

impl FocusableTable {
    fn new(title: &str, items: Vec<(String, String, String)>) -> Self {
        Self {
            title: title.to_string(),
            items,
            selected: 0,
        }
    }

    fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + self.items.len() - 1) % self.items.len();
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        // Define colors based on focus state
        let (border_color, selected_style, header_style) = if is_focused {
            (
                Color::Yellow,
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                Color::DarkGray,
                Style::default().bg(Color::DarkGray).fg(Color::Black),
                Style::default().fg(Color::DarkGray),
            )
        };

        // Create rows
        let rows: Vec<Row> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, (col1, col2, col3))| {
                let style = if i == self.selected {
                    selected_style
                } else {
                    Style::default().fg(Color::White)
                };
                Row::new(vec![col1.clone(), col2.clone(), col3.clone()]).style(style)
            })
            .collect();

        // Create table
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ],
        )
        .header(
            Row::new(vec!["Name", "Value", "Status"])
                .style(header_style)
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(format!(
                    " {} {} ",
                    self.title,
                    if is_focused { "●" } else { "○" }
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .row_highlight_style(selected_style);

        // Render the table (selection is handled via row styling)
        frame.render_widget(table, area);
    }
}

/// The Data tab containing two side-by-side tables.
struct DataTab {
    left_table: FocusableTable,
    right_table: FocusableTable,
    /// Which table is focused: 0 = left, 1 = right
    focused_table: usize,
}

impl DataTab {
    fn new() -> Self {
        let left_items = vec![
            ("Alpha".to_string(), "100".to_string(), "Active".to_string()),
            ("Beta".to_string(), "200".to_string(), "Pending".to_string()),
            ("Gamma".to_string(), "300".to_string(), "Active".to_string()),
            (
                "Delta".to_string(),
                "400".to_string(),
                "Inactive".to_string(),
            ),
            (
                "Epsilon".to_string(),
                "500".to_string(),
                "Active".to_string(),
            ),
        ];

        let right_items = vec![
            (
                "Server-1".to_string(),
                "Online".to_string(),
                "OK".to_string(),
            ),
            (
                "Server-2".to_string(),
                "Offline".to_string(),
                "Error".to_string(),
            ),
            (
                "Server-3".to_string(),
                "Online".to_string(),
                "Warning".to_string(),
            ),
            (
                "Server-4".to_string(),
                "Online".to_string(),
                "OK".to_string(),
            ),
        ];

        Self {
            left_table: FocusableTable::new("Items", left_items),
            right_table: FocusableTable::new("Servers", right_items),
            focused_table: 0,
        }
    }

    fn focused_table_mut(&mut self) -> &mut FocusableTable {
        if self.focused_table == 0 {
            &mut self.left_table
        } else {
            &mut self.right_table
        }
    }
}

impl Component for DataTab {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        // Create layout for two side-by-side tables
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Draw tables with focus state
        self.left_table
            .draw(frame, chunks[0], self.focused_table == 0);
        self.right_table
            .draw(frame, chunks[1], self.focused_table == 1);
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        if let Event::Key(key) = event {
            match key.code {
                // Switch focus between tables
                KeyCode::Left => {
                    if self.focused_table > 0 {
                        self.focused_table = 0;
                    }
                    EventResult::Handled
                }
                KeyCode::Right => {
                    if self.focused_table < 1 {
                        self.focused_table = 1;
                    }
                    EventResult::Handled
                }
                // Navigate within focused table
                KeyCode::Up => {
                    self.focused_table_mut().select_prev();
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.focused_table_mut().select_next();
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }
}

// =============================================================================
// Main Application
// =============================================================================

struct FocusTablesApp;

impl FocusTablesApp {
    fn new() -> Self {
        Self
    }
}

impl Component for FocusTablesApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Split area: tabbar, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar with border
                Constraint::Min(5),    // Tab content with border
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Draw border around tabbar
        let tabbar_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let tabbar_inner = tabbar_block.inner(chunks[0]);
        frame.render_widget(tabbar_block, chunks[0]);

        // Draw tab bar using TabBar widget
        TabBar::draw(frame, tabbar_inner, ctx);

        // Draw border around content area
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let content_inner = content_block.inner(chunks[1]);
        frame.render_widget(content_block, chunks[1]);

        // Draw active tab content using TabContent widget
        TabContent::draw(frame, content_inner, ctx);

        // Footer with controls
        let tabs_ctx = ctx.tabs();
        let active_tab = tabs_ctx.active_id().unwrap_or("unknown");
        let footer_text = if active_tab == "data" {
            "Tab: Switch tabs | ←/→: Switch table focus | ↑/↓: Navigate rows | q: Quit"
        } else {
            "Tab: Switch tabs | q: Quit"
        };
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Handle quit
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                // Tab navigation between tabs
                KeyCode::Tab => {
                    ctx.tabs().select_next();
                    EventResult::Handled
                }
                // Arrow keys - let the active tab handle these
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                    // Return Unhandled so the event bubbles... but wait,
                    // in our current model MainUi gets events first.
                    // We need to explicitly forward to the tab.
                    // For now, we don't handle these at MainUi level,
                    // but the tab's handle_event is called by TabManager.
                    EventResult::Unhandled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }
}

impl MainUi for FocusTablesApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Build the application with tabs
    let app = AppBuilder::new()
        .main_ui(FocusTablesApp::new())
        .add_tab("welcome", "Welcome", WelcomeTab)
        .add_tab("data", "Data", DataTab::new())
        .mouse_capture(false)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
