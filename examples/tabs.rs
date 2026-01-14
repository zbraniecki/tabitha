//! Tabs example demonstrating the tab system in tabitha.
//!
//! This example shows:
//! - Implementing the Tab trait for custom tabs
//! - Registering tabs with the application
//! - Drawing tab bar and content using DrawContext
//! - Navigating tabs using AppContext
//! - Enabling/disabling tabs at runtime
//!
//! Controls:
//! - Tab/Shift+Tab: Navigate between tabs
//! - 1-3: Select specific tabs
//! - d: Toggle disable on the Settings tab
//! - q/Ctrl+C: Quit

use tabitha::{
    AppBuilder, AppContext, Component, DrawContext, Event, EventResult, KeyCode, KeyModifiers,
    MainUi, Tab,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

// =============================================================================
// Tabs
// =============================================================================

/// Home tab with a welcome message.
struct HomeTab;

impl Tab for HomeTab {
    fn id(&self) -> &str {
        "home"
    }

    fn title(&self) -> &str {
        "Home"
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let content = Paragraph::new(
            "Welcome to the Tabs Example!\n\n\
             This is the Home tab.\n\n\
             Use Tab/Shift+Tab to navigate between tabs.\n\
             Press 1, 2, or 3 to jump to specific tabs.\n\
             Press 'd' to toggle the Settings tab enabled/disabled.",
        )
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title("Home")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(content, area);
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

impl Tab for DashboardTab {
    fn id(&self) -> &str {
        "dashboard"
    }

    fn title(&self) -> &str {
        "Dashboard"
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let content = Paragraph::new(format!(
            "Dashboard Statistics\n\n\
             View count: {}\n\n\
             This counter increments each time you switch to this tab.",
            self.view_count
        ))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title("Dashboard")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        );
        frame.render_widget(content, area);
    }

    fn on_activate(&mut self) {
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

impl Tab for SettingsTab {
    fn id(&self) -> &str {
        "settings"
    }

    fn title(&self) -> &str {
        "Settings"
    }

    fn draw(&self, frame: &mut Frame, area: Rect) {
        let content = Paragraph::new(
            "Settings Panel\n\n\
             This tab can be disabled.\n\
             Press 'd' to toggle this tab's enabled state.\n\n\
             When disabled, you cannot navigate to this tab.",
        )
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title("Settings")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        frame.render_widget(content, area);
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
        // Split area: tab bar at top, content in middle, footer at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Tab bar
                Constraint::Min(5),    // Tab content
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Draw tab bar
        ctx.tabs().draw_tabbar(frame, chunks[0]);

        // Draw active tab content
        ctx.tabs().draw_content(frame, chunks[1]);

        // Footer with controls
        let settings_status = if self.settings_enabled {
            "enabled"
        } else {
            "disabled"
        };
        let footer_text = format!(
            "Tab/Shift+Tab: Navigate | 1-3: Jump to tab | d: Toggle settings ({}) | q: Quit",
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

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                // Tab navigation
                KeyCode::Tab => {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        ctx.tabs().select_prev();
                    } else {
                        ctx.tabs().select_next();
                    }
                    EventResult::Handled
                }
                KeyCode::BackTab => {
                    ctx.tabs().select_prev();
                    EventResult::Handled
                }
                // Direct tab selection
                KeyCode::Char('1') => {
                    ctx.tabs().select(0);
                    EventResult::Handled
                }
                KeyCode::Char('2') => {
                    ctx.tabs().select(1);
                    EventResult::Handled
                }
                KeyCode::Char('3') => {
                    ctx.tabs().select(2);
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
    // Build the application with tabs
    let app = AppBuilder::new()
        .main_ui(TabsApp::new())
        .add_tab(HomeTab)
        .add_tab(DashboardTab::new())
        .add_tab(SettingsTab::new())
        .mouse_capture(false) // Disable mouse for this example
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
