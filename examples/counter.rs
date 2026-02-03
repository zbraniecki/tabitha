//! Counter example demonstrating the tabitha framework.
//!
//! This example shows:
//! - A MainUi component that handles keyboard events
//! - A background task that sends periodic tick messages
//! - Communication between task and UI via typed messages
//! - Runtime mouse capture toggling via AppContext
//!
//! Controls:
//! - Up/Down: Increment/decrement counter
//! - Space: Toggle auto-increment from background task
//! - m: Toggle mouse capture on/off
//! - q/Ctrl+C: Quit

use std::time::Duration;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, HasTerminal,
    KeyCode, MainUi, Task, TaskContext, TaskSender,
};

// =============================================================================
// Background Task
// =============================================================================

/// A background task that sends tick messages at a fixed interval.
struct TickerTask {
    interval: Duration,
}

impl TickerTask {
    fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

/// Message sent by the ticker task.
#[derive(Debug, Clone)]
enum TickerMessage {
    /// A tick occurred with the current count.
    Tick(u64),
}

impl Task for TickerTask {
    type Message = TickerMessage;
    type Error = std::convert::Infallible;

    async fn run(
        self,
        sender: TaskSender<Self::Message>,
        mut ctx: TaskContext,
    ) -> Result<(), Self::Error> {
        let mut count = 0u64;
        let mut interval = tokio::time::interval(self.interval);

        // Skip the first immediate tick
        interval.tick().await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    count += 1;
                    if sender.send(TickerMessage::Tick(count)).await.is_err() {
                        // Channel closed, app is shutting down
                        break;
                    }
                }
                _ = ctx.cancelled() => {
                    break;
                }
            }
        }
        Ok(())
    }
}

// =============================================================================
// UI Components
// =============================================================================

/// The main application UI.
struct CounterApp {
    /// Current counter value.
    counter: i64,
    /// Number of ticks received from background task.
    ticks: u64,
    /// Whether auto-increment is enabled.
    auto_increment: bool,
    /// Whether mouse capture is enabled (for display).
    mouse_enabled: bool,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            counter: 0,
            ticks: 0,
            auto_increment: false,
            mouse_enabled: true,
        }
    }
}

impl Component for CounterApp {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        // Split area: main content area at top, footer at bottom
        let outer_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),    // Main area (header + content)
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Draw border around main area
        let main_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let main_inner = main_block.inner(outer_chunks[0]);
        frame.render_widget(main_block, outer_chunks[0]);

        // Split main area: header at top, content below
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(5),    // Content
            ])
            .split(main_inner);

        // Header
        let header = Paragraph::new("Counter Example").style(Style::default().fg(Color::Cyan));
        frame.render_widget(header, main_chunks[0]);

        // Main content - counter display
        let counter_text = format!(
            "\nCounter: {}\n\nTicks from task: {}\nAuto-increment: {}\nMouse capture: {}",
            self.counter,
            self.ticks,
            if self.auto_increment { "ON" } else { "OFF" },
            if self.mouse_enabled { "ON" } else { "OFF" }
        );
        let content = Paragraph::new(counter_text).style(Style::default().fg(Color::White));
        frame.render_widget(content, main_chunks[1]);

        // Footer with controls
        let footer_text = "↑/↓: Inc/Dec | Space: Toggle auto | m: Toggle mouse | q: Quit";
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, outer_chunks[1]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Quit on Ctrl+C or Ctrl+Q
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Handle specific key presses
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.counter = self.counter.saturating_add(1);
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.counter = self.counter.saturating_sub(1);
                    EventResult::Handled
                }
                KeyCode::Char(' ') => {
                    self.auto_increment = !self.auto_increment;
                    EventResult::Handled
                }
                KeyCode::Char('m') => {
                    // Toggle mouse capture at runtime
                    let new_state = !ctx.mouse_capture_enabled();
                    if ctx.set_mouse_capture(new_state).is_ok() {
                        self.mouse_enabled = new_state;
                    }
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else if let Event::Mouse(_) = event {
            // Increment counter on any mouse event (to demonstrate mouse capture)
            self.counter = self.counter.saturating_add(1);
            EventResult::Handled
        } else {
            EventResult::Unhandled
        }
    }
}

impl MainUi for CounterApp {
    fn handle_task_message(
        &mut self,
        task_name: &str,
        message: Box<dyn std::any::Any + Send>,
        _ctx: &mut AppContext,
    ) -> bool {
        if task_name == "ticker" {
            if let Some(TickerMessage::Tick(count)) = message.downcast_ref::<TickerMessage>() {
                self.ticks = *count;
                if self.auto_increment {
                    self.counter = self.counter.saturating_add(1);
                }
                return true;
            }
        }
        false
    }
}

// =============================================================================
// Main
// =============================================================================

#[path = "_common/mod.rs"]
mod common;
use common::Args;

use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(CounterApp::new())
        .add_task("ticker", TickerTask::new(Duration::from_secs(1)))
        .mouse_capture(true) // Enable mouse capture (default)
        .enable_dev_console(args.dev)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
