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
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::event::{Event, KeyCode};
use crate::focus::EventResult;
use crate::theme::Theme;

/// Maximum number of log lines to keep in the buffer.
const DEFAULT_BUFFER_SIZE: usize = 1000;

/// Width of the level filter block in the bottom bar.
const LEVEL_BLOCK_WIDTH: u16 = 12;

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

    /// Get the background color for the level filter block.
    fn level_bg_color(level: tracing::Level) -> Color {
        match level {
            tracing::Level::TRACE => Color::DarkGray,
            tracing::Level::DEBUG => Color::Blue,
            tracing::Level::INFO => Color::Green,
            tracing::Level::WARN => Color::Yellow,
            tracing::Level::ERROR => Color::Red,
        }
    }

    /// Get the display text for the level filter block.
    fn level_display_text(level: tracing::Level) -> &'static str {
        match level {
            tracing::Level::TRACE => "   TRACE   ",
            tracing::Level::DEBUG => "   DEBUG   ",
            tracing::Level::INFO => "   INFO    ",
            tracing::Level::WARN => "   WARN    ",
            tracing::Level::ERROR => "   ERROR   ",
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
    /// Current level filter (defaults to DEBUG, TRACE shows all).
    level_filter: tracing::Level,
    /// Text filter for searching log messages.
    text_filter: String,
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
            level_filter: tracing::Level::DEBUG,
            text_filter: String::new(),
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

    /// Handle keyboard events when the viewer is visible.
    /// Returns EventResult::Handled if the event was consumed.
    pub fn handle_event(&mut self, event: &Event) -> EventResult {
        if !self.visible {
            return EventResult::Unhandled;
        }

        match event {
            Event::Key(key_event) => {
                match key_event.code {
                    // Tab cycles forward through level filters
                    KeyCode::Tab => {
                        self.cycle_level_filter();
                        EventResult::Handled
                    }
                    // BackTab (Shift+Tab) cycles backwards
                    KeyCode::BackTab => {
                        self.cycle_level_filter_backwards();
                        EventResult::Handled
                    }
                    // Escape or backtick closes the viewer
                    KeyCode::Esc | KeyCode::Char('`') => {
                        if self.text_filter.is_empty() {
                            self.hide();
                        } else {
                            self.text_filter.clear();
                        }
                        EventResult::Handled
                    }
                    // Backspace removes last character from filter
                    KeyCode::Backspace => {
                        self.text_filter.pop();
                        self.scroll_to_bottom();
                        EventResult::Handled
                    }
                    // Up arrow scrolls up
                    KeyCode::Up => {
                        self.scroll_up(1);
                        EventResult::Handled
                    }
                    // Down arrow scrolls down
                    KeyCode::Down => {
                        self.scroll_down(1);
                        EventResult::Handled
                    }
                    // Page Up scrolls up by page
                    KeyCode::PageUp => {
                        self.scroll_up(10);
                        EventResult::Handled
                    }
                    // Page Down scrolls down by page
                    KeyCode::PageDown => {
                        self.scroll_down(10);
                        EventResult::Handled
                    }
                    // Enter processes commands if filter starts with "/"
                    KeyCode::Enter => {
                        if self.text_filter.starts_with('/') {
                            self.process_command();
                        }
                        EventResult::Handled
                    }
                    // Character input adds to text filter
                    KeyCode::Char(c) => {
                        self.text_filter.push(c);
                        self.scroll_to_bottom();
                        EventResult::Handled
                    }
                    _ => EventResult::Unhandled,
                }
            }
            _ => EventResult::Unhandled,
        }
    }

    /// Cycle through level filters: DEBUG -> INFO -> WARN -> ERROR -> TRACE -> DEBUG
    fn cycle_level_filter(&mut self) {
        self.level_filter = match self.level_filter {
            tracing::Level::DEBUG => tracing::Level::INFO,
            tracing::Level::INFO => tracing::Level::WARN,
            tracing::Level::WARN => tracing::Level::ERROR,
            tracing::Level::ERROR => tracing::Level::TRACE,
            tracing::Level::TRACE => tracing::Level::DEBUG,
        };
        self.scroll_to_bottom();
    }

    /// Cycle backwards through level filters: DEBUG -> TRACE -> ERROR -> WARN -> INFO -> DEBUG
    fn cycle_level_filter_backwards(&mut self) {
        self.level_filter = match self.level_filter {
            tracing::Level::DEBUG => tracing::Level::TRACE,
            tracing::Level::TRACE => tracing::Level::ERROR,
            tracing::Level::ERROR => tracing::Level::WARN,
            tracing::Level::WARN => tracing::Level::INFO,
            tracing::Level::INFO => tracing::Level::DEBUG,
        };
        self.scroll_to_bottom();
    }

    /// Get filtered logs based on level and text filters.
    /// Level filter shows UP TO that level (ERROR shows ERROR+WARN+INFO+DEBUG+TRACE)
    fn get_filtered_logs(&self) -> Vec<&LogLine> {
        self.buffer
            .iter()
            .filter(|log| {
                // Apply level filter - show logs that are <= filter level (more or equally severe)
                // In tracing, the ordering is: ERROR < WARN < INFO < DEBUG < TRACE
                // (ERROR is "smallest"/most severe; TRACE is "largest"/least severe)
                // So ERROR filter shows all logs (ERROR is most severe, includes everything)
                // WARN filter shows WARN, INFO, DEBUG, TRACE (excludes ERROR)
                // DEBUG filter shows DEBUG, INFO, WARN, ERROR (excludes TRACE)
                if log.level > self.level_filter {
                    // Log is less severe than filter, filter it out
                    return false;
                }
                // Skip text filter if it starts with "/" (command mode)
                if !self.text_filter.is_empty() && !self.text_filter.starts_with('/') {
                    let filter_lower = self.text_filter.to_lowercase();
                    if !log.message.to_lowercase().contains(&filter_lower) {
                        return false;
                    }
                }
                true
            })
            .collect()
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
        let filtered = self.get_filtered_logs();
        let max_scroll = filtered.len().saturating_sub(1);
        self.scroll = self.scroll.saturating_add(lines).min(max_scroll);
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
        self.text_filter.clear();
        self.level_filter = tracing::Level::DEBUG;
    }

    /// Process a command from the filter text.
    fn process_command(&mut self) {
        let command = self.text_filter.trim();
        match command {
            "/clear" => {
                self.buffer.clear();
                self.scroll = 0;
                self.text_filter.clear();
            }
            _ => {
                // Unknown command, just clear the filter
                self.text_filter.clear();
            }
        }
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

        // Draw border and background around the entire viewer (including bottom bar)
        let block = Block::default()
            .title(" Logs ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let inner_area = block.inner(viewer_area);
        frame.render_widget(block, viewer_area);

        // Split inner area into content and bottom bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner_area);

        let content_area = chunks[0];
        let bottom_bar_area = chunks[1];

        // Get filtered logs
        let filtered = self.get_filtered_logs();
        let total_filtered = filtered.len();

        // Build the text content from filtered logs
        let content_height = content_area.height as usize;
        let lines: Vec<Line> = filtered
            .iter()
            .rev()
            .skip(self.scroll)
            .take(content_height)
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
        frame.render_widget(paragraph, content_area);

        // Draw bottom bar with level filter and text filter
        self.draw_bottom_bar(frame, bottom_bar_area, total_filtered);
    }

    /// Draw the bottom bar with level filter block and text filter.
    fn draw_bottom_bar(&self, frame: &mut Frame, area: Rect, total_filtered: usize) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(LEVEL_BLOCK_WIDTH), Constraint::Min(0)])
            .split(area);

        let level_area = chunks[0];
        let filter_area = chunks[1];

        // Draw level filter block with background color
        let level_bg = LogLine::level_bg_color(self.level_filter);
        let level_text = LogLine::level_display_text(self.level_filter);
        let level_style = Style::default().bg(level_bg).fg(Color::Black);

        let level_line = Line::from(vec![Span::styled(level_text, level_style)]);
        let level_widget = Paragraph::new(level_line);
        frame.render_widget(level_widget, level_area);

        // Draw text filter input
        let filter_text = if self.text_filter.is_empty() {
            "_".to_string()
        } else {
            format!("{}_", self.text_filter)
        };

        let filter_style = Style::default().fg(Color::White);
        let filter_line = Line::from(vec![
            Span::styled(
                format!(" [{}]", total_filtered),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(" "),
            Span::styled(filter_text, filter_style),
        ]);
        let filter_widget = Paragraph::new(filter_line);
        frame.render_widget(filter_widget, filter_area);
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

    #[test]
    fn test_level_filter_cycle() {
        let mut viewer = LogViewer::new();

        // Start with DEBUG (default)
        assert_eq!(viewer.level_filter, tracing::Level::DEBUG);

        // Cycle through filters: DEBUG -> INFO -> WARN -> ERROR -> TRACE -> DEBUG
        viewer.cycle_level_filter();
        assert_eq!(viewer.level_filter, tracing::Level::INFO);

        viewer.cycle_level_filter();
        assert_eq!(viewer.level_filter, tracing::Level::WARN);

        viewer.cycle_level_filter();
        assert_eq!(viewer.level_filter, tracing::Level::ERROR);

        viewer.cycle_level_filter();
        assert_eq!(viewer.level_filter, tracing::Level::TRACE);

        // Back to DEBUG
        viewer.cycle_level_filter();
        assert_eq!(viewer.level_filter, tracing::Level::DEBUG);
    }

    #[test]
    fn test_level_filter_cycle_backwards() {
        let mut viewer = LogViewer::new();

        // Start with DEBUG (default)
        assert_eq!(viewer.level_filter, tracing::Level::DEBUG);

        // Cycle backwards: DEBUG -> TRACE -> ERROR -> WARN -> INFO -> DEBUG
        viewer.cycle_level_filter_backwards();
        assert_eq!(viewer.level_filter, tracing::Level::TRACE);

        viewer.cycle_level_filter_backwards();
        assert_eq!(viewer.level_filter, tracing::Level::ERROR);

        viewer.cycle_level_filter_backwards();
        assert_eq!(viewer.level_filter, tracing::Level::WARN);

        viewer.cycle_level_filter_backwards();
        assert_eq!(viewer.level_filter, tracing::Level::INFO);

        // Back to DEBUG
        viewer.cycle_level_filter_backwards();
        assert_eq!(viewer.level_filter, tracing::Level::DEBUG);
    }

    #[test]
    fn test_text_filter() {
        let mut viewer = LogViewer::with_capacity(10);

        // Set level to show all logs (TRACE)
        viewer.level_filter = tracing::Level::TRACE;

        viewer.info("Database connection established");
        viewer.error("Database query failed");
        viewer.debug("Cache miss for key");
        viewer.info("User login successful");

        // Test filtering by text
        viewer.text_filter = "database".to_string();
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 2);

        // Test case-insensitive filtering
        viewer.text_filter = "DATABASE".to_string();
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_command_mode() {
        let mut viewer = LogViewer::with_capacity(10);

        // Add some logs
        viewer.info("Message 1");
        viewer.info("Message 2");
        viewer.info("Message 3");

        // Set to show all
        viewer.level_filter = tracing::Level::TRACE;

        // Command mode (starts with /) should not filter
        viewer.text_filter = "/cle".to_string();
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 3); // All logs shown

        // Clear command when executed should clear logs
        viewer.text_filter = "/clear".to_string();
        viewer.process_command();
        assert!(viewer.is_empty());
        assert!(viewer.text_filter.is_empty());
    }

    #[test]
    fn test_combined_filters() {
        let mut viewer = LogViewer::with_capacity(10);

        viewer.info("Database connection established");
        viewer.error("Database query failed");
        viewer.error("Network timeout");
        viewer.debug("Cache miss for key");

        // Filter by WARN level (shows WARN and more severe: ERROR)
        // and "database" text - should match ERROR log only
        viewer.level_filter = tracing::Level::WARN;
        viewer.text_filter = "database".to_string();

        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "Database query failed");
    }

    #[test]
    fn test_level_filter_up_to() {
        let mut viewer = LogViewer::with_capacity(10);

        // Add logs at all levels
        // In tracing: ERROR (most severe) < WARN < INFO < DEBUG < TRACE (least severe)
        viewer.error("Error message");
        viewer.warn("Warning message");
        viewer.info("Info message");
        viewer.debug("Debug message");
        viewer.trace("Trace message");

        // Filter to ERROR - shows ERROR only (most severe)
        viewer.level_filter = tracing::Level::ERROR;
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 1);

        // Filter to WARN - shows WARN, ERROR (more or equally severe than WARN)
        viewer.level_filter = tracing::Level::WARN;
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 2);

        // Filter to INFO - shows INFO, WARN, ERROR (more or equally severe than INFO)
        viewer.level_filter = tracing::Level::INFO;
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 3);

        // Filter to DEBUG - shows DEBUG, INFO, WARN, ERROR (excludes TRACE)
        viewer.level_filter = tracing::Level::DEBUG;
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 4);

        // Filter to TRACE - shows ALL logs (TRACE is least severe, includes everything)
        viewer.level_filter = tracing::Level::TRACE;
        let filtered = viewer.get_filtered_logs();
        assert_eq!(filtered.len(), 5);
    }

    #[test]
    fn test_level_display_text() {
        assert_eq!(
            LogLine::level_display_text(tracing::Level::ERROR),
            "   ERROR   "
        );
        assert_eq!(
            LogLine::level_display_text(tracing::Level::WARN),
            "   WARN    "
        );
        assert_eq!(
            LogLine::level_display_text(tracing::Level::INFO),
            "   INFO    "
        );
        assert_eq!(
            LogLine::level_display_text(tracing::Level::DEBUG),
            "   DEBUG   "
        );
        assert_eq!(
            LogLine::level_display_text(tracing::Level::TRACE),
            "   TRACE   "
        );
    }

    #[test]
    fn test_level_bg_colors() {
        use ratatui::style::Color;

        assert_eq!(LogLine::level_bg_color(tracing::Level::ERROR), Color::Red);
        assert_eq!(LogLine::level_bg_color(tracing::Level::WARN), Color::Yellow);
        assert_eq!(LogLine::level_bg_color(tracing::Level::INFO), Color::Green);
        assert_eq!(LogLine::level_bg_color(tracing::Level::DEBUG), Color::Blue);
        assert_eq!(
            LogLine::level_bg_color(tracing::Level::TRACE),
            Color::DarkGray
        );
    }

    #[test]
    fn test_level_ordering() {
        // Check how tracing::Level orders values
        // In tracing: ERROR (most severe) < WARN < INFO < DEBUG < TRACE (least severe)
        // This is opposite of what you might expect - "smaller" means "more severe"
        assert!(
            tracing::Level::ERROR < tracing::Level::WARN,
            "ERROR should be less than WARN"
        );
        assert!(
            tracing::Level::WARN < tracing::Level::INFO,
            "WARN should be less than INFO"
        );
        assert!(
            tracing::Level::INFO < tracing::Level::DEBUG,
            "INFO should be less than DEBUG"
        );
        assert!(
            tracing::Level::DEBUG < tracing::Level::TRACE,
            "DEBUG should be less than TRACE"
        );
    }
}
