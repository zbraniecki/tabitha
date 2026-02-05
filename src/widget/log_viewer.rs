//! Log viewer for debugging tabitha applications.
//!
//! The log viewer is a toggleable overlay that displays log messages.
//! It can be toggled with your own keyboard shortcut and stays
//! open while navigating the application.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{AppBuilder, LogViewer};
//!
//! // Toggle with your own keyboard shortcut in handle_event:
//! if event.is_key(KeyCode::Char('~')) {
//!     ctx.dev_overlays().toggle_log_viewer();
//! }
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

/// Maximum number of log lines to keep in the buffer.
const DEFAULT_BUFFER_SIZE: usize = 1000;

/// A line in the log viewer.
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

/// Log viewer for displaying log messages.
#[derive(Debug)]
pub struct LogViewer {
    /// Whether the viewer is currently visible.
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

impl Default for LogViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewer {
    /// Create a new log viewer.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BUFFER_SIZE)
    }

    /// Create a log viewer with a specific buffer capacity.
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            visible: false,
            buffer: VecDeque::with_capacity(max_size),
            max_size,
            scroll: 0,
            auto_scroll: true,
        }
    }

    /// Toggle the viewer visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Show the viewer.
    pub fn show(&mut self) {
        self.visible = true;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    /// Hide the viewer.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Check if the viewer is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Log a message to the viewer.
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

    /// Clear the viewer buffer.
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

    /// Draw the log viewer.
    ///
    /// This draws the viewer as an overlay in the top portion of the screen.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        if !self.visible {
            return;
        }

        // Use top 40% of the screen
        let viewer_height = (area.height as f32 * 0.4) as u16;
        let viewer_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: viewer_height,
        };

        // Clear the background
        frame.render_widget(Clear, viewer_area);

        // Draw border and background
        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(viewer_area);
        frame.render_widget(block, viewer_area);

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
    fn test_log_viewer_default() {
        let viewer = LogViewer::default();
        assert!(!viewer.is_visible());
        assert!(viewer.is_empty());
    }

    #[test]
    fn test_log_viewer_toggle() {
        let mut viewer = LogViewer::new();
        assert!(!viewer.is_visible());

        viewer.toggle();
        assert!(viewer.is_visible());

        viewer.toggle();
        assert!(!viewer.is_visible());
    }

    #[test]
    fn test_log_viewer_logging() {
        let mut viewer = LogViewer::with_capacity(10);

        viewer.info("Test message");
        assert_eq!(viewer.len(), 1);

        viewer.error("Error message");
        assert_eq!(viewer.len(), 2);
    }

    #[test]
    fn test_log_viewer_buffer_limit() {
        let mut viewer = LogViewer::with_capacity(5);

        for i in 0..10 {
            viewer.info(format!("Message {}", i));
        }

        assert_eq!(viewer.len(), 5);
    }

    #[test]
    fn test_log_viewer_clear() {
        let mut viewer = LogViewer::new();
        viewer.info("Test");
        viewer.clear();
        assert!(viewer.is_empty());
    }
}
