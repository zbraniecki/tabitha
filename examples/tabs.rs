//! Tabs example demonstrating the tab system in tabitha.
//!
//! This example shows:
//! - Implementing components for tabs
//! - Registering tabs with the application
//! - Drawing tab bar and content using the new TabBar and TabContent widgets
//! - Navigating tabs using AppContext
//! - Enabling/disabling tabs at runtime
//!
//! Controls:
//! - Tab/Shift+Tab: Navigate between tabs
//! - 1-3: Select specific tabs
//! - d: Toggle disable on the Settings tab
//! - q/Ctrl+C: Quit

#[path = "_common/mod.rs"]
mod common;
use clap::Parser;
use common::Args;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, HasTabs, KeyCode,
    LifecycleContext, MainUi,
};

// Import the new widgets
use tabitha::widget::{TabBar, TabContent};

// =============================================================================
// Tabs
// =============================================================================

/// Home tab with a welcome message.
struct HomeTab;

impl Component for HomeTab {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let content = Paragraph::new(
            "Welcome to the Tabs Example!\n\n\
             This is the Home tab.\n\n\
             Use Tab/Shift+Tab to navigate between tabs.\n\
             Press 1, 2, or 3 to jump to specific tabs.\n\
             Press 'd' to toggle the Settings tab enabled/disabled.\n\n\
             This example demonstrates the new 'context as state, widget as view' pattern.",
        )
        .style(Style::default().fg(Color::White));
        frame.render_widget(content, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }
}

/// Dashboard tab with some stats.
struct DashboardTab {
    view_count: u32,
}

impl DashboardTab {
    fn new() -> Self {
        Self { view_count: 0 }
    }
}

impl Component for DashboardTab {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let content = Paragraph::new(format!(
            "Dashboard Statistics\n\n\
             View count: {}\n\n\
             This counter increments each time you switch to this tab.\n\n\
             The new TabBar and TabContent widgets provide a clean separation\n\
             between state (in context) and view (in widgets).",
            self.view_count
        ))
        .style(Style::default().fg(Color::White));
        frame.render_widget(content, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }

    fn on_mount(&mut self, _ctx: &mut LifecycleContext) {
        // Called when tab becomes active
        self.view_count += 1;
    }
}

/// Settings tab that can be disabled externally.
struct SettingsTab;

impl SettingsTab {
    fn new() -> Self {
        Self
    }
}

impl Component for SettingsTab {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let content = Paragraph::new(
            "Settings Panel\n\n\
             This tab can be disabled.\n\
             Press 'd' to toggle this tab's enabled state.\n\n\
             When disabled, you cannot navigate to this tab.\n\n\
             The TabBar widget automatically grays out disabled tabs.",
        )
        .style(Style::default().fg(Color::White));
        frame.render_widget(content, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }
}

// =============================================================================
// Main Application
// =============================================================================

/// The main application that manages tabs.
struct TabsApp {
    /// Reference to toggle settings tab (we need interior mutability in real app)
    settings_enabled: bool,
}

impl TabsApp {
    fn new() -> Self {
        Self {
            settings_enabled: true,
        }
    }
}

impl Component for TabsApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Split area: tabbar, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Tab bar (compact)
                Constraint::Min(5),    // Tab content with border
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Draw tab bar using the new TabBar widget
        // User controls where the tab bar is rendered
        TabBar::draw(frame, chunks[0], ctx);

        // Draw border around content area
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let content_inner = content_block.inner(chunks[1]);
        frame.render_widget(content_block, chunks[1]);

        // Draw active tab content using the new TabContent widget
        // User controls where the content is rendered
        TabContent::draw(frame, content_inner, ctx);

        // Footer with controls
        let settings_status = if self.settings_enabled {
            "enabled"
        } else {
            "disabled"
        };
        let footer_text = format!(
            "Tab/Shift+Tab: Navigate | 1-3: Jump | d: Toggle settings ({}) | q: Quit",
            settings_status
        );
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[2]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Quit on Ctrl+C or Ctrl+Q
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Let TabBar handle its standard navigation events first
        // This handles Tab, Shift+Tab, and 1-9
        if TabBar::handle_event(event, ctx).is_handled() {
            return EventResult::Handled;
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                // Toggle settings tab enabled/disabled
                KeyCode::Char('d') => {
                    let currently_enabled = ctx.tabs().is_enabled("settings");
                    ctx.tabs().set_enabled("settings", !currently_enabled);
                    self.settings_enabled = !currently_enabled;
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }
}

impl MainUi for TabsApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Build the application with tabs
    let app = AppBuilder::new()
        .main_ui(TabsApp::new())
        .add_tab("home", "Home", HomeTab)
        .add_tab("dashboard", "Dashboard", DashboardTab::new())
        .add_tab("settings", "Settings", SettingsTab::new())
        .mouse_capture(false) // Disable mouse for this example
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
