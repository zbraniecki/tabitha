//! Spinner widget for displaying loading/busy states.
//!
//! Supports three animation modes:
//! - Full: Rotating braille spinner (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
//! - Reduced: Simple on/off blink
//! - None: Static character
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{Spinner, SpinnerConfig};
//!
//! let mut spinner = Spinner::new();
//!
//! // In draw method:
//! spinner.draw(frame, area, false);
//!
//! // In tick method (for animation):
//! spinner.tick(ctx.control_animations().as_mut().unwrap());
//! ```

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::Span,
    Frame,
};
use std::time::Duration;

use crate::animation::{AnimationMode, ControlAnimationContext};
use crate::event::Event;
use crate::focus::EventResult;
use crate::widget::{Control, ControlEvent};

/// Events emitted by Spinner (empty for now)
#[derive(Debug, Clone)]
pub enum SpinnerEvent {}

impl ControlEvent for SpinnerEvent {}

/// Configuration for Spinner appearance
#[derive(Debug, Clone)]
pub struct SpinnerConfig {
    /// Style for the spinner character
    pub style: Style,
    /// Full animation characters (braille patterns)
    pub frames: Vec<char>,
    /// Static character for no-animation mode
    pub static_char: char,
    /// Frame duration for full animation
    pub frame_duration: Duration,
    /// Blink duration for reduced animation
    pub blink_duration: Duration,
}

impl Default for SpinnerConfig {
    fn default() -> Self {
        Self {
            style: Style::default().fg(Color::Cyan),
            frames: vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'],
            static_char: '⠋',
            frame_duration: Duration::from_millis(80),
            blink_duration: Duration::from_millis(500),
        }
    }
}

impl SpinnerConfig {
    /// Create config with a specific color
    pub fn with_color(mut self, color: Color) -> Self {
        self.style = Style::default().fg(color);
        self
    }
}

/// Internal state for spinner animation
#[derive(Debug)]
struct SpinnerState {
    /// Current frame index
    frame_idx: usize,
    /// Accumulated time for animation
    accumulated_time: Duration,
    /// Visibility for blink mode (reduced animation)
    visible: bool,
}

impl SpinnerState {
    fn new() -> Self {
        Self {
            frame_idx: 0,
            accumulated_time: Duration::ZERO,
            visible: true,
        }
    }

    fn reset(&mut self) {
        self.frame_idx = 0;
        self.accumulated_time = Duration::ZERO;
        self.visible = true;
    }
}

/// Spinner widget for indicating loading/busy states
///
/// Displays an animated spinner that adapts to the animation mode:
/// - Full: Smooth rotation through braille patterns
/// - Reduced: Simple on/off blink
/// - None: Static character
#[derive(Debug)]
pub struct Spinner {
    config: SpinnerConfig,
    state: SpinnerState,
}

impl Spinner {
    /// Create a new spinner with default configuration
    pub fn new() -> Self {
        Self {
            config: SpinnerConfig::default(),
            state: SpinnerState::new(),
        }
    }

    /// Create a spinner with custom configuration
    pub fn with_config(config: SpinnerConfig) -> Self {
        Self {
            config,
            state: SpinnerState::new(),
        }
    }

    /// Set the spinner color
    pub fn with_color(mut self, color: Color) -> Self {
        self.config.style = Style::default().fg(color);
        self
    }

    /// Reset animation to initial state
    pub fn reset(&mut self) {
        self.state.reset();
    }

    /// Get current character to display
    fn current_char(&self) -> char {
        if self.state.visible {
            self.config.frames[self.state.frame_idx % self.config.frames.len()]
        } else {
            ' ' // Invisible when blinking off
        }
    }

    /// Update animation for full mode
    fn tick_full(&mut self, elapsed: Duration) -> bool {
        self.state.accumulated_time += elapsed;
        let frame_count = self.config.frames.len();
        let total_duration = self.config.frame_duration * frame_count as u32;

        // Use modulo to properly cycle through frames
        let cycle_time = self.state.accumulated_time.as_millis() % total_duration.as_millis();
        let progress = cycle_time as f64 / total_duration.as_millis() as f64;
        let new_idx = (progress * frame_count as f64) as usize % frame_count;

        if new_idx != self.state.frame_idx {
            self.state.frame_idx = new_idx;
            true
        } else {
            false
        }
    }

    /// Update animation for reduced mode
    fn tick_reduced(&mut self, elapsed: Duration) -> bool {
        self.state.accumulated_time += elapsed;
        let cycle_duration = self.config.blink_duration * 2;
        let cycle_pos =
            self.state.accumulated_time.as_millis() as u64 % cycle_duration.as_millis() as u64;

        let new_visible = cycle_pos < self.config.blink_duration.as_millis() as u64;

        if new_visible != self.state.visible {
            self.state.visible = new_visible;
            true
        } else {
            false
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Control for Spinner {
    type Event = SpinnerEvent;

    fn draw(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let char_to_show = if self.state.visible {
            self.current_char()
        } else {
            self.config.static_char
        };

        let span = Span::styled(char_to_show.to_string(), self.config.style);
        frame.render_widget(ratatui::widgets::Paragraph::new(span), area);
    }

    fn handle_event(&mut self, _event: &Event) -> EventResult {
        // Spinner doesn't handle input events
        EventResult::Unhandled
    }

    fn tick(&mut self, ctx: &mut ControlAnimationContext<'_>) -> bool {
        if !ctx.is_enabled() {
            return false;
        }

        let mode = ctx.mode();

        if mode == AnimationMode::None {
            // Static - no animation
            return false;
        }

        // Use a fixed time step for consistent animation
        let base_elapsed = Duration::from_millis(16); // ~60fps
        let speed = ctx.speed_multiplier();
        let elapsed = Duration::from_nanos((base_elapsed.as_nanos() as f32 * speed) as u64);

        if mode == AnimationMode::Reduced {
            self.tick_reduced(elapsed)
        } else {
            self.tick_full(elapsed)
        }
    }

    fn take_events(&mut self) -> Vec<Self::Event> {
        Vec::new()
    }

    fn has_events(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_new() {
        let spinner = Spinner::new();
        assert!(spinner.state.visible); // Visible by default
        assert_eq!(spinner.state.frame_idx, 0);
    }

    #[test]
    fn test_spinner_with_color() {
        let spinner = Spinner::new().with_color(Color::Red);
        assert_eq!(spinner.config.style.fg, Some(Color::Red));
    }

    #[test]
    fn test_spinner_reset() {
        let mut spinner = Spinner::new();
        spinner.state.frame_idx = 5;
        spinner.state.visible = false;
        spinner.reset();
        assert_eq!(spinner.state.frame_idx, 0);
        assert!(spinner.state.visible);
    }
}
