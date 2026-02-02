//! Developer console for debugging tabitha applications.
//!
//! The developer console is a toggleable overlay that displays log messages
//! and debug information. It can be enabled via the `--dev` CLI flag and
//! toggled with the `~` key.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{AppBuilder, DevConsole};
//!
//! let app = AppBuilder::new()
//!     .main_ui(MyApp::new())
//!     .enable_dev_console(true)
//!     .build()?;
//! ```

use std::collections::VecDeque;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::theme::Theme;

/// Maximum number of log lines to keep in the console buffer.
const DEFAULT_BUFFER_SIZE: usize = 1000;

/// A line in the developer console.
#[derive(Debug, Clone)]
pub struct LogLine {
    /// Timestamp when the line was logged.
    pub timestamp: std::time::SystemTime,
    /// The log level.
    pub level: tracing::Level,
    /// The message content.
    pub message: String,
    /// Optional target/module path.
    pub target: Option<String>,
}

impl LogLine {
    /// Create a new log line.
    pub fn new(level: tracing::Level, message: impl Into<String>) -> Self {
        Self {
            timestamp: std::time::SystemTime::now(),
            level,
            message: message.into(),
            target: None,
        }
    }

    /// Set the target/module path.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Format the timestamp for display.
    fn format_timestamp(&self) -> String {
        let duration = self
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let nanos = duration.subsec_nanos();
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60,
            nanos / 1_000_000
        )
    }

    /// Get the color for this log level.
    fn level_color(&self) -> Color {
        match self.level {
            tracing::Level::TRACE => Color::DarkGray,
            tracing::Level::DEBUG => Color::Blue,
            tracing::Level::INFO => Color::Green,
            tracing::Level::WARN => Color::Yellow,
            tracing::Level::ERROR => Color::Red,
        }
    }
}

/// Developer console for displaying log messages.
#[derive(Debug)]
pub struct DevConsole {
    /// Whether the console is currently visible.
    visible: bool,
    /// Ring buffer of log lines.
    buffer: VecDeque<LogLine>,
    /// Maximum buffer size.
    max_size: usize,
    /// Current scroll position (0 = bottom).
    scroll: usize,
    /// Whether to auto-scroll to bottom on new messages.
    auto_scroll: bool,
}

impl Default for DevConsole {
    fn default() -> Self {
        Self::new()
    }
}

impl DevConsole {
    /// Create a new developer console.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BUFFER_SIZE)
    }

    /// Create a developer console with a specific buffer capacity.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            visible: false,
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            scroll: 0,
            auto_scroll: true,
        }
    }

    /// Toggle the console visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Show the console.
    pub fn show(&mut self) {
        self.visible = true;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Hide the console.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Check if the console is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Log a message to the console.
    pub fn log(&mut self, level: tracing::Level, message: impl Into<String>) {
        let line = LogLine::new(level, message);
        self.push_line(line);
    }

    /// Log a trace message.
    pub fn trace(&mut self, message: impl Into<String>) {
        self.log(tracing::Level::TRACE, message);
    }

    /// Log a debug message.
    pub fn debug(&mut self, message: impl Into<String>) {
        self.log(tracing::Level::DEBUG, message);
    }

    /// Log an info message.
    pub fn info(&mut self, message: impl Into<String>) {
        self.log(tracing::Level::INFO, message);
    }

    /// Log a warning message.
    pub fn warn(&mut self, message: impl Into<String>) {
        self.log(tracing::Level::WARN, message);
    }

    /// Log an error message.
    pub fn error(&mut self, message: impl Into<String>) {
        self.log(tracing::Level::ERROR, message);
    }

    /// Push a log line to the buffer.
    fn push_line(&mut self, line: LogLine) {
        if self.buffer.len() >= self.max_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(line);

        if self.auto_scroll && self.visible {
            self.scroll_to_bottom();
        }
    }

    /// Push a log line to the buffer (public version).
    pub fn push(&mut self, line: LogLine) {
        self.push_line(line);
    }

    /// Scroll up by N lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.auto_scroll = false;
        self.scroll = self.scroll.saturating_add(lines).min(self.buffer.len());
    }

    /// Scroll down by N lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll = self.scroll.saturating_sub(lines);
        if self.scroll == 0 {
            self.auto_scroll = true;
        }
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll = 0;
        self.auto_scroll = true;
    }

    /// Clear the console buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.scroll = 0;
    }

    /// Get the number of lines in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Draw the developer console.
    ///
    /// This draws the console as an overlay in the top portion of the screen.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        if !self.visible {
            return;
        }

        // Use top 40% of the screen
        let console_height = (area.height as f32 * 0.4) as u16;
        let console_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: console_height,
        };

        // Clear the background
        frame.render_widget(Clear, console_area);

        // Draw border and background
        let block = Block::default()
            .title(" Developer Console (~ to close)")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(console_area);
        frame.render_widget(block, console_area);

        // Build the text content
        let lines: Vec<Line> = self
            .buffer
            .iter()
            .rev()
            .skip(self.scroll)
            .take(inner.height as usize)
            .rev()
            .map(|log| {
                let timestamp = log.format_timestamp();
                let level_style = Style::default()
                    .fg(log.level_color())
                    .add_modifier(Modifier::BOLD);

                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", timestamp),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("{:5} ", log.level), level_style),
                    Span::raw(&log.message),
                ])
            })
            .collect();

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        frame.render_widget(paragraph, inner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_console_default() {
        let console = DevConsole::default();
        assert!(!console.is_visible());
        assert!(console.is_empty());
    }

    #[test]
    fn test_dev_console_toggle() {
        let mut console = DevConsole::new();
        assert!(!console.is_visible());

        console.toggle();
        assert!(console.is_visible());

        console.toggle();
        assert!(!console.is_visible());
    }

    #[test]
    fn test_dev_console_logging() {
        let mut console = DevConsole::with_capacity(10);

        console.info("Test message");
        assert_eq!(console.len(), 1);

        console.error("Error message");
        assert_eq!(console.len(), 2);
    }

    #[test]
    fn test_dev_console_buffer_limit() {
        let mut console = DevConsole::with_capacity(5);

        for i in 0..10 {
            console.info(format!("Message {}", i));
        }

        assert_eq!(console.len(), 5);
    }

    #[test]
    fn test_dev_console_clear() {
        let mut console = DevConsole::new();
        console.info("Test");
        console.clear();
        assert!(console.is_empty());
    }
}
