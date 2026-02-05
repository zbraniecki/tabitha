//! Sidebar example demonstrating the sidebar system in tabitha.
//!
//! This example shows:
//! - Creating and configuring a sidebar with fixed content
//! - Tabs that control only the main content area
//! - Sidebar with independent todo list
//! - Toggling sidebar visibility
//! - Animated width changes
//! - Different sidebar positions (left/right)
//! - TabBar on top, sidebar/content split in middle, footbar at bottom
//!
//! Controls:
//! - b: Toggle sidebar visibility
//! - Tab/Shift+Tab: Navigate between tabs
//! - 1-3: Select specific tab
//! - [ ]: Decrease/increase sidebar width
//! - l/r: Move sidebar to left/right side
//! - t: Toggle layout mode (tabbar full width vs inside content)
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
use std::time::{Duration, Instant};
use tabitha::widget::{TabBar, TabContent};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, KeyCode, MainUi,
    Side, Sidebar, SidebarState,
};

// =============================================================================
// Todo List - Fixed sidebar content
// =============================================================================

struct TodoList {
    items: Vec<(bool, &'static str)>,
}

impl TodoList {
    fn new() -> Self {
        Self {
            items: vec![
                (true, "Review sidebar PR"),
                (false, "Add animation support"),
                (true, "Update documentation"),
                (false, "Write tests"),
                (false, "Release v0.1.0"),
            ],
        }
    }
}

impl Component for TodoList {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let text = self
            .items
            .iter()
            .map(|(done, item)| {
                if *done {
                    format!("[x] {}", item)
                } else {
                    format!("[ ] {}", item)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White));
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }
}

// =============================================================================
// Content Panels - Used as tabs (main content area)
// =============================================================================

struct ContentPanel {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    title: &'static str,
    text: &'static str,
}

impl ContentPanel {
    fn new(id: &str, title: &'static str, text: &'static str) -> Self {
        Self {
            id: id.to_string(),
            title,
            text,
        }
    }
}

impl Component for ContentPanel {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let paragraph = Paragraph::new(self.text).style(Style::default().fg(Color::White));
        frame.render_widget(paragraph, area);
    }

    fn handle_event(&mut self, _event: &Event, _ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }
}

// =============================================================================
// Main Application
// =============================================================================

/// Layout mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    /// Tabbar full width on top
    TabbarFullWidth,
    /// Tabbar inside content area
    TabbarInContent,
}

/// The main application with sidebar
struct SidebarApp {
    sidebar_state: SidebarState,
    todo_list: TodoList,
    width_level: u8,
    layout_mode: LayoutMode,
    last_tick: Instant,
}

impl SidebarApp {
    fn new() -> Self {
        let sidebar_state = SidebarState::new(Side::Left)
            .with_width(Constraint::Percentage(25))
            .with_animations(true)
            .with_animation_duration(Duration::from_millis(200));

        Self {
            sidebar_state,
            todo_list: TodoList::new(),
            width_level: 1, // 0=15%, 1=25%, 2=35%
            layout_mode: LayoutMode::TabbarFullWidth,
            last_tick: Instant::now(),
        }
    }

    fn get_width_constraint(&self) -> Constraint {
        match self.width_level {
            0 => Constraint::Percentage(15),
            1 => Constraint::Percentage(25),
            2 => Constraint::Percentage(35),
            _ => Constraint::Percentage(25),
        }
    }

    fn update_sidebar_width(&mut self) {
        let constraint = self.get_width_constraint();
        self.sidebar_state.set_to_width(constraint);
    }

    fn toggle_layout_mode(&mut self) {
        self.layout_mode = match self.layout_mode {
            LayoutMode::TabbarFullWidth => LayoutMode::TabbarInContent,
            LayoutMode::TabbarInContent => LayoutMode::TabbarFullWidth,
        };
    }
}

impl Component for SidebarApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        match self.layout_mode {
            LayoutMode::TabbarFullWidth => {
                // Layout: TabBar on top, sidebar/content split in middle, footer at bottom
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3), // TabBar
                        Constraint::Min(0),    // Main area
                        Constraint::Length(3), // Footer
                    ])
                    .split(area);

                // Draw TabBar at top (full width)
                TabBar::draw(frame, chunks[0], ctx);

                // Split main area between sidebar and content
                let [content_area, sidebar_area] = Sidebar::new(&self.sidebar_state, chunks[1]);

                // Draw content area (tabs content)
                self.draw_content(frame, content_area, ctx);

                // Draw sidebar (fixed todo list)
                self.draw_sidebar(frame, sidebar_area, ctx);

                // Draw footer at bottom
                self.draw_footer(frame, chunks[2]);
            }
            LayoutMode::TabbarInContent => {
                // Layout: Footer at bottom, remaining area split between sidebar and content
                let outer_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),    // Main area
                        Constraint::Length(3), // Footer
                    ])
                    .split(area);

                // Split main area between sidebar and content
                let [content_area, sidebar_area] =
                    Sidebar::new(&self.sidebar_state, outer_chunks[0]);

                // Split content area: TabBar on top, content below
                let content_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(0)])
                    .split(content_area);

                // Draw TabBar inside content area
                TabBar::draw(frame, content_chunks[0], ctx);

                // Draw content below TabBar
                self.draw_content(frame, content_chunks[1], ctx);

                // Draw sidebar (fixed todo list)
                self.draw_sidebar(frame, sidebar_area, ctx);

                // Draw footer at bottom
                self.draw_footer(frame, outer_chunks[1]);
            }
        }
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Quit on Ctrl+C or Ctrl+Q
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Let TabBar handle its navigation first (Tab/Shift+Tab, 1-9)
        if TabBar::handle_event(event, ctx).is_handled() {
            return EventResult::Handled;
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                // Toggle sidebar visibility
                KeyCode::Char('b') => {
                    self.sidebar_state.toggle();
                    EventResult::Handled
                }
                // Decrease width
                KeyCode::Char('[') => {
                    if self.width_level > 0 {
                        self.width_level -= 1;
                        self.update_sidebar_width();
                    }
                    EventResult::Handled
                }
                // Increase width
                KeyCode::Char(']') => {
                    if self.width_level < 2 {
                        self.width_level += 1;
                        self.update_sidebar_width();
                    }
                    EventResult::Handled
                }
                // Move to left
                KeyCode::Char('l') => {
                    self.sidebar_state.set_side(Side::Left);
                    EventResult::Handled
                }
                // Move to right
                KeyCode::Char('r') => {
                    self.sidebar_state.set_side(Side::Right);
                    EventResult::Handled
                }
                // Toggle layout mode
                KeyCode::Char('t') => {
                    self.toggle_layout_mode();
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext) {
        // Update sidebar animations with elapsed time since last tick
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;
        self.sidebar_state.tick(elapsed);
    }
}

impl SidebarApp {
    fn draw_content(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let block = Block::default()
            .title(" Content ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Use TabContent widget to render the active tab
        TabContent::draw(frame, inner, ctx);
    }

    fn draw_sidebar(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if !self.sidebar_state.is_visible() {
            return;
        }

        let sidebar_block = Block::default()
            .title(" Status ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        let sidebar_inner = sidebar_block.inner(area);
        frame.render_widget(sidebar_block, area);

        // Build status text
        let status_text = format!(
            "Sidebar: {}\n\
            Side: {:?}\n\
            Width: {}\n\
            Layout: {:?}\n\n\
            Todo List:",
            if self.sidebar_state.is_visible() {
                "open"
            } else {
                "closed"
            },
            self.sidebar_state.side(),
            match self.width_level {
                0 => "Narrow (15%)",
                1 => "Medium (25%)",
                2 => "Wide (35%)",
                _ => "Medium",
            },
            self.layout_mode,
        );

        let status_para = Paragraph::new(status_text).style(Style::default().fg(Color::White));
        frame.render_widget(status_para, sidebar_inner);

        // Draw todo list below status if there's room
        let todo_area = Rect {
            x: sidebar_inner.x,
            y: sidebar_inner.y + 7, // Offset for status text
            width: sidebar_inner.width,
            height: sidebar_inner.height.saturating_sub(7),
        };

        if todo_area.height > 0 {
            self.todo_list.draw(frame, todo_area, ctx);
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let controls = "b: Toggle Sidebar | Tab: Tab | 1-3: Tab | [ ]: Width | l/r: Side | t: Layout | q: Quit";

        let footer = Paragraph::new(controls)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, area);
    }
}

impl MainUi for SidebarApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    println!("Sidebar Example - Controls:");
    println!("  b: Toggle sidebar visibility");
    println!("  Tab/Shift+Tab: Navigate between tabs");
    println!("  1-3: Select specific tab");
    println!("  [ ]: Decrease/increase sidebar width");
    println!("  l/r: Move sidebar to left/right");
    println!("  t: Toggle layout mode");
    println!("  q: Quit");
    println!();

    // Build the application with tabs for the main content area
    let app = AppBuilder::new()
        .main_ui(SidebarApp::new())
        .mouse_capture(false)
        .tick_rate(Duration::from_millis(16)) // ~60fps for smooth animations
        .enable_dev_console(args.dev)
        .with_log_receiver(log_rx)
        .add_tab(
            "home",
            "Home",
            ContentPanel::new(
                "home",
                "Home",
                "Welcome to the Sidebar Demo!\n\n\
                This example demonstrates:\n\
                • Sidebar with fixed Todo List\n\
                • Tabs control main content only\n\
                • Animated sidebar transitions\n\
                • Configurable width and position\n\n\
                The sidebar stays the same regardless of which tab is active.",
            ),
        )
        .add_tab(
            "files",
            "Files",
            ContentPanel::new(
                "files",
                "Files",
                "📁 src/\n  📄 lib.rs\n  📄 main.rs\n  📄 sidebar.rs\n\
                📁 examples/\n  📄 sidebar.rs\n  📄 tabs.rs\n\
                📄 Cargo.toml\n📄 README.md\n\n\
                The sidebar still shows the Todo List!",
            ),
        )
        .add_tab(
            "settings",
            "Settings",
            ContentPanel::new(
                "settings",
                "Settings",
                "Settings Panel\n\n\
                Current Configuration:\n\
                • Side: Adjustable (l/r keys)\n\
                • Width: Adjustable ([ ] keys)\n\
                • Layout Mode: Toggle (t key)\n\n\
                Sidebar Todo List remains visible.",
            ),
        )
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
