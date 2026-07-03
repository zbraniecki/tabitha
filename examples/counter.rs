//! Counter example demonstrating the tabitha framework.
//!
//! This example shows:
//! - A MainUi component that handles keyboard events
//! - A background task that sends periodic tick messages
//! - Communication between task and UI via typed messages
//! - Runtime mouse capture toggling via AppContext
//! - Pausing/resuming background tasks
//! - Adjustable ticker interval
//!
//! Controls:
//! - Up/Down: Increment/decrement counter
//! - Space: Toggle auto-increment from background task
//! - p: Pause/resume background ticker task
//! - Shift+Up/Down: Adjust ticker interval (500µs, 1s, 2s, 3s, 4s, 5s)
//! - m: Toggle mouse capture on/off
//! - `: Toggle log viewer
//! - F12: Toggle debug panel
//! - q/Ctrl+C: Quit

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::KeyEventKind;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tabitha::{
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, HasTerminal,
    KeyCode, KeyModifiers, MainUi, Task, TaskContext, TaskSender,
};

// =============================================================================
// Background Task
// =============================================================================

/// Available ticker intervals in milliseconds.
const INTERVALS_MS: [u64; 6] = [1, 1000, 2000, 3000, 4000, 5000]; // 1ms (stress test), 1s, 2s, 3s, 4s, 5s

/// Shared state for controlling the ticker task.
#[derive(Debug, Clone)]
struct TickerControl {
    /// Whether the ticker is currently paused.
    paused: Arc<AtomicBool>,
    /// Current interval index into INTERVALS_MS.
    interval_index: Arc<AtomicU64>,
}

impl TickerControl {
    fn new() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            interval_index: Arc::new(AtomicU64::new(1)), // Start at 1s (index 1)
        }
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    fn toggle(&self) -> bool {
        let new_state = !self.is_paused();
        self.set_paused(new_state);
        new_state
    }

    fn get_interval_ms(&self) -> u64 {
        let index = self.interval_index.load(Ordering::Relaxed) as usize;
        INTERVALS_MS[index.min(INTERVALS_MS.len() - 1)]
    }

    fn get_interval(&self) -> Duration {
        Duration::from_millis(self.get_interval_ms())
    }

    fn increase_interval(&self) -> u64 {
        let current = self.interval_index.load(Ordering::Relaxed);
        let new_index = (current + 1).min((INTERVALS_MS.len() - 1) as u64);
        self.interval_index.store(new_index, Ordering::Relaxed);
        INTERVALS_MS[new_index as usize]
    }

    fn decrease_interval(&self) -> u64 {
        let current = self.interval_index.load(Ordering::Relaxed);
        let new_index = current.saturating_sub(1);
        self.interval_index.store(new_index, Ordering::Relaxed);
        INTERVALS_MS[new_index as usize]
    }
}

/// A background task that sends tick messages at a fixed interval.
struct TickerTask {
    control: TickerControl,
}

impl TickerTask {
    fn new(control: TickerControl) -> Self {
        Self { control }
    }
}

/// Message sent by the ticker task.
#[derive(Debug, Clone)]
enum TickerMessage {
    /// A tick occurred with the current count.
    Tick(u64),
    /// The pause state changed.
    PauseChanged(bool),
    /// The interval changed (in milliseconds).
    IntervalChanged(u64),
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
        let mut last_pause_state = self.control.is_paused();
        let mut last_interval_ms = self.control.get_interval_ms();
        let mut interval = tokio::time::interval(self.control.get_interval());
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Skip the first immediate tick
        interval.tick().await;

        loop {
            // Check if pause state changed
            let current_pause_state = self.control.is_paused();
            if current_pause_state != last_pause_state {
                last_pause_state = current_pause_state;
                if sender
                    .send(TickerMessage::PauseChanged(current_pause_state))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            // Check if interval changed
            let current_interval_ms = self.control.get_interval_ms();
            if current_interval_ms != last_interval_ms {
                last_interval_ms = current_interval_ms;
                interval = tokio::time::interval(Duration::from_millis(current_interval_ms));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                if sender
                    .send(TickerMessage::IntervalChanged(current_interval_ms))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            tokio::select! {
                _ = interval.tick() => {
                    // Only send tick if not paused
                    if !self.control.is_paused() {
                        count += 1;
                        if sender.send(TickerMessage::Tick(count)).await.is_err() {
                            // Channel closed, app is shutting down
                            break;
                        }
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
    /// Control handle for the ticker task.
    ticker_control: TickerControl,
    /// Whether the ticker is currently paused.
    ticker_paused: bool,
    /// Current ticker interval in milliseconds.
    ticker_interval_ms: u64,
}

impl CounterApp {
    fn new() -> Self {
        Self {
            counter: 0,
            ticks: 0,
            auto_increment: false,
            mouse_enabled: true,
            ticker_control: TickerControl::new(),
            ticker_paused: false,
            ticker_interval_ms: 1000, // Start at 1s
        }
    }

    /// Get the control handle for the ticker task.
    fn ticker_control(&self) -> TickerControl {
        self.ticker_control.clone()
    }

    /// Format the interval for display.
    fn format_interval(&self) -> String {
        if self.ticker_interval_ms < 1000 {
            format!("{}ms", self.ticker_interval_ms)
        } else {
            format!("{}s", self.ticker_interval_ms / 1000)
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
            "\nCounter: {}\n\nTicks from task: {}\nAuto-increment: {}\nTicker: {} (interval: {})\nMouse capture: {}",
            self.counter,
            self.ticks,
            if self.auto_increment { "ON" } else { "OFF" },
            if self.ticker_paused {
                "PAUSED".to_string() 
            } else {
                "RUNNING".to_string() 
            },
            self.format_interval(),
            if self.mouse_enabled { "ON" } else { "OFF" }
        );
        let content = Paragraph::new(counter_text).style(Style::default().fg(Color::White));
        frame.render_widget(content, main_chunks[1]);

        // Footer with controls
        let footer_text = "↑/↓: Inc/Dec | Space: Toggle auto | p: Pause | ⇧↑/↓: Interval | m: Mouse | `: Logs | F12: Debug | q: Quit";
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
        if let Event::Key(key) = event && key.kind == KeyEventKind::Press {
            let has_shift = key.modifiers.contains(KeyModifiers::SHIFT);

            match key.code {
                KeyCode::Char('q') => {
                    ctx.quit();
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if has_shift {
                        // Shift+Up: Increase interval
                        self.ticker_interval_ms = self.ticker_control.increase_interval();
                        EventResult::Handled
                    } else {
                        self.counter = self.counter.saturating_add(1);
                        EventResult::Handled
                    }
                }
                KeyCode::Down => {
                    if has_shift {
                        // Shift+Down: Decrease interval
                        self.ticker_interval_ms = self.ticker_control.decrease_interval();
                        EventResult::Handled
                    } else {
                        self.counter = self.counter.saturating_sub(1);
                        EventResult::Handled
                    }
                }
                KeyCode::Char(' ') => {
                    self.auto_increment = !self.auto_increment;
                    EventResult::Handled
                }
                KeyCode::Char('p') => {
                    // Toggle ticker pause
                    self.ticker_paused = self.ticker_control.toggle();
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
                KeyCode::Char('`') => {
                    // Toggle log viewer
                    if let Some(mut overlays) = ctx.dev_overlays() {
                        overlays.toggle_log_viewer();
                    }
                    EventResult::Handled
                }
                KeyCode::F(12) => {
                    // Toggle debug panel
                    if let Some(mut overlays) = ctx.dev_overlays() {
                        overlays.toggle_debug_panel();
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
            match message.downcast_ref::<TickerMessage>() {
                Some(TickerMessage::Tick(count)) => {
                    self.ticks = *count;
                    if self.auto_increment {
                        self.counter = self.counter.saturating_add(1);
                    }
                    return true;
                }
                Some(TickerMessage::PauseChanged(paused)) => {
                    self.ticker_paused = *paused;
                    return true;
                }
                Some(TickerMessage::IntervalChanged(interval_ms)) => {
                    self.ticker_interval_ms = *interval_ms;
                    return true;
                }
                None => {}
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

    // Create the app first to get the control handle
    let app_state = CounterApp::new();
    let ticker_control = app_state.ticker_control();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(app_state)
        .add_task("ticker", TickerTask::new(ticker_control))
        .mouse_capture(true) // Enable mouse capture (default)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
