//! Debug panel for frame timing and animation debugging.
//!
//! The debug panel is a compact corner overlay that displays:
//! - Current frame number
//! - Last frame render time (milliseconds)
//! - Last 10 frame triggers (what caused redraws)
//!
//! It can be toggled independently of the log viewer and stays
//! open while navigating the application.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{AppBuilder, DebugPanel};
//!
//! // Toggle with your own keyboard shortcut in handle_event:
//! if event.is_key(KeyCode::F(12)) {
//!     ctx.dev_overlays().toggle_debug_panel();
//! }
//! ```

use std::collections::VecDeque;
use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::Theme;

/// Maximum number of frames to track in history.
const FRAME_HISTORY_SIZE: usize = 10;

/// Width of the debug panel in characters.
const PANEL_WIDTH: u16 = 40;

/// Height of the debug panel in characters.
const PANEL_HEIGHT: u16 = 14;

/// Information about what triggered a frame redraw.
#[derive(Debug, Clone)]
pub enum FrameTrigger {
    /// Redraw triggered by a keyboard event.
    Key(String),
    /// Redraw triggered by a mouse event.
    Mouse(String),
    /// Redraw triggered by terminal resize.
    Resize,
    /// Redraw triggered by animation tick.
    Tick,
    /// Redraw triggered by log update.
    LogUpdate,
    /// Redraw triggered by task message.
    TaskMessage(String),
    /// Redraw triggered by paste event.
    Paste,
    /// Redraw triggered by focus event.
    Focus(String),
    /// Unknown or other trigger.
    Other(String),
}

impl FrameTrigger {
    /// Get a short display string for the trigger.
    fn display(&self) -> String {
        match self {
            FrameTrigger::Key(key) => format!("Key({})", key),
            FrameTrigger::Mouse(kind) => format!("Mouse({})", kind),
            FrameTrigger::Resize => "Resize".to_string(),
            FrameTrigger::Tick => "Tick".to_string(),
            FrameTrigger::LogUpdate => "LogUpdate".to_string(),
            FrameTrigger::TaskMessage(name) => format!("Task({})", name),
            FrameTrigger::Paste => "Paste".to_string(),
            FrameTrigger::Focus(kind) => format!("Focus({})", kind),
            FrameTrigger::Other(s) => s.clone(),
        }
    }
}

/// Information about a single frame.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    /// Frame number (increments on each redraw).
    pub frame_number: u64,
    /// Time taken to render this frame (milliseconds).
    pub render_time_ms: u64,
    /// What triggered this frame redraw.
    pub trigger: FrameTrigger,
}

/// Debug panel for displaying frame timing and trigger information.
#[derive(Debug)]
pub struct DebugPanel {
    /// Whether the panel is currently visible.
    visible: bool,
    /// Current frame number.
    frame_counter: u64,
    /// History of recent frames.
    frame_history: VecDeque<FrameInfo>,
    /// Last frame render time.
    last_render_time: Duration,
}

impl Default for DebugPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugPanel {
    /// Create a new debug panel.
    pub fn new() -> Self {
        Self {
            visible: false,
            frame_counter: 0,
            frame_history: VecDeque::with_capacity(FRAME_HISTORY_SIZE),
            last_render_time: Duration::ZERO,
        }
    }

    /// Toggle the panel visibility.
    ///
    /// When shown after being hidden, frame history is cleared.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.clear();
        }
    }

    /// Show the panel.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hide the panel and clear history.
    pub fn hide(&mut self) {
        self.visible = false;
        self.clear();
    }

    /// Check if the panel is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Clear all frame history.
    pub fn clear(&mut self) {
        self.frame_counter = 0;
        self.frame_history.clear();
        self.last_render_time = Duration::ZERO;
    }

    /// Record a frame being rendered.
    ///
    /// This should be called after each draw operation.
    pub fn record_frame(&mut self, render_time: Duration, trigger: FrameTrigger) {
        self.frame_counter += 1;
        self.last_render_time = render_time;

        let frame_info = FrameInfo {
            frame_number: self.frame_counter,
            render_time_ms: render_time.as_millis() as u64,
            trigger,
        };

        if self.frame_history.len() >= FRAME_HISTORY_SIZE {
            self.frame_history.pop_back();
        }
        self.frame_history.push_front(frame_info);
    }

    /// Get the current frame number.
    pub fn frame_number(&self) -> u64 {
        self.frame_counter
    }

    /// Get the last render time.
    pub fn last_render_time(&self) -> Duration {
        self.last_render_time
    }

    /// Get the frame history.
    pub fn frame_history(&self) -> &VecDeque<FrameInfo> {
        &self.frame_history
    }

    /// Format a duration with appropriate units.
    fn format_duration(duration: Duration) -> String {
        let nanos = duration.as_nanos();
        if nanos < 1_000 {
            format!("{}ns", nanos)
        } else if nanos < 1_000_000 {
            format!("{}µs", duration.as_micros())
        } else {
            // Show milliseconds with one decimal place for better precision
            let millis = nanos as f64 / 1_000_000.0;
            format!("{:.1}ms", millis)
        }
    }

    /// Draw the debug panel.
    ///
    /// This draws the panel in the top-right corner of the screen.
    pub fn draw(&self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        if !self.visible {
            return;
        }

        let draw_start = std::time::Instant::now();

        // Calculate position: top-right corner
        let panel_x = area.x + area.width.saturating_sub(PANEL_WIDTH);
        let panel_area = Rect {
            x: panel_x,
            y: area.y,
            width: PANEL_WIDTH.min(area.width),
            height: PANEL_HEIGHT.min(area.height),
        };

        // Clear the background
        frame.render_widget(Clear, panel_area);

        // Draw border and background
        let block = Block::default()
            .title(" Debug ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(panel_area);
        frame.render_widget(block, panel_area);

        let before_content = std::time::Instant::now();

        // Build the content lines
        let mut lines: Vec<Line> = Vec::new();

        // Frame number and render time
        lines.push(Line::from(vec![
            Span::styled(
                format!("Frame #{} ", self.frame_counter),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({})", Self::format_duration(self.last_render_time)),
                Style::default().fg(Color::Yellow),
            ),
        ]));

        lines.push(Line::from(""));

        // History header
        lines.push(Line::from(vec![Span::styled(
            "Last triggers:",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )]));

        // Frame history
        for frame_info in self.frame_history.iter().take(10) {
            let trigger_text = frame_info.trigger.display();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  #{}", frame_info.frame_number),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(": "),
                Span::styled(trigger_text, Style::default().fg(Color::White)),
            ]));
        }

        let content_build_time = before_content.elapsed();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);

        let total_time = draw_start.elapsed();

        // Log slow draws
        if total_time.as_micros() > 100 {
            tracing::debug!(
                content_build_time = ?content_build_time,
                total_time = ?total_time,
                "debug_panel draw time"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_panel_default() {
        let panel = DebugPanel::default();
        assert!(!panel.is_visible());
        assert_eq!(panel.frame_number(), 0);
    }

    #[test]
    fn test_debug_panel_toggle() {
        let mut panel = DebugPanel::new();
        assert!(!panel.is_visible());

        panel.toggle();
        assert!(panel.is_visible());

        panel.toggle();
        assert!(!panel.is_visible());
        assert_eq!(panel.frame_number(), 0); // Cleared on hide
    }

    #[test]
    fn test_debug_panel_record_frame() {
        let mut panel = DebugPanel::new();
        panel.show();

        panel.record_frame(Duration::from_millis(5), FrameTrigger::Tick);
        assert_eq!(panel.frame_number(), 1);
        assert_eq!(panel.last_render_time().as_millis(), 5);

        panel.record_frame(Duration::from_millis(3), FrameTrigger::Resize);
        assert_eq!(panel.frame_number(), 2);
        assert_eq!(panel.frame_history().len(), 2);
    }

    #[test]
    fn test_debug_panel_history_limit() {
        let mut panel = DebugPanel::new();
        panel.show();

        for i in 0..20 {
            panel.record_frame(
                Duration::from_millis(1),
                FrameTrigger::Other(format!("test{}", i)),
            );
        }

        assert_eq!(panel.frame_history().len(), FRAME_HISTORY_SIZE);
    }

    #[test]
    fn test_frame_trigger_display() {
        assert_eq!(FrameTrigger::Tick.display(), "Tick");
        assert_eq!(FrameTrigger::Resize.display(), "Resize");
        assert_eq!(
            FrameTrigger::Key("Enter".to_string()).display(),
            "Key(Enter)"
        );
        assert_eq!(
            FrameTrigger::Mouse("LeftClick".to_string()).display(),
            "Mouse(LeftClick)"
        );
    }
}
