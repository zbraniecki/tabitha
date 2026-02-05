//! Progress bar widget for displaying progress or busy states.
//!
//! Supports:
//! - Determinate progress (0-100%) with smooth partial block rendering
//! - Indeterminate animations: BackAndForth, Marquee, Pulse
//! - Theme integration (accent color for filled, secondary for unfilled)
//! - Optional labels with configurable position
//! - Animation speed control via AnimationController
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{ProgressBar, ProgressBarConfig, IndeterminateStyle, LabelPosition};
//! use tabitha::theme::Theme;
//!
//! // Determinate progress bar
//! let mut upload_progress = ProgressBar::new()
//!     .with_value(45.0)
//!     .with_label("Uploading...")
//!     .with_label_position(LabelPosition::Left);
//!
//! // Indeterminate "thinking" indicator
//! let mut thinking = ProgressBar::indeterminate()
//!     .with_label("AI is thinking...")
//!     .with_indeterminate_style(IndeterminateStyle::BackAndForth);
//!
//! // In draw method:
//! upload_progress.draw(frame, area, false);
//!
//! // In tick method (for animation):
//! upload_progress.tick(ctx.control_animations().as_mut().unwrap());
//! ```

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    Frame,
};
use std::time::Duration;

use crate::animation::{AnimationMode, ControlAnimationContext};
use crate::event::Event;
use crate::focus::EventResult;
use crate::theme::Theme;
use crate::widget::{Control, ControlEvent};

/// Animation style for indeterminate progress
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndeterminateStyle {
    /// Ping-pong animation (back and forth like opencode)
    #[default]
    BackAndForth,
    /// Continuous scrolling animation
    Marquee,
    /// Pulsing fade effect
    Pulse,
}

/// Position for progress bar label
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelPosition {
    /// No label
    #[default]
    None,
    /// Label on the left
    Left,
    /// Label on the right
    Right,
    /// Label centered over the bar
    Center,
}

/// Events emitted by ProgressBar (empty for now)
#[derive(Debug, Clone)]
pub enum ProgressBarEvent {}

impl ControlEvent for ProgressBarEvent {}

/// Configuration for ProgressBar appearance
#[derive(Debug, Clone)]
pub struct ProgressBarConfig {
    /// Style for filled portion (default: theme.accent)
    pub filled_style: Style,
    /// Style for unfilled portion (default: theme.secondary)
    pub unfilled_style: Style,
    /// Style for label text (default: theme.foreground)
    pub label_style: Style,
    /// Whether to show percentage (default: true for determinate)
    pub show_percentage: bool,
    /// Label position (default: None)
    pub label_position: LabelPosition,
    /// Animation duration for indeterminate mode (default: 800ms)
    pub animation_duration: Duration,
    /// Indeterminate animation style (default: BackAndForth)
    pub indeterminate_style: IndeterminateStyle,
    /// Full block character (default: '█')
    pub full_block: char,
    /// Partial block characters for determinate mode (1/8 to 7/8): ▏▎▍▌▋▊▉
    pub partial_blocks: [char; 7],
}

impl ProgressBarConfig {
    /// Create config from theme
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            filled_style: theme.accent_style(),
            unfilled_style: theme.secondary_style(),
            label_style: theme.fg_style(),
            show_percentage: true,
            label_position: LabelPosition::None,
            animation_duration: Duration::from_millis(800),
            indeterminate_style: IndeterminateStyle::BackAndForth,
            full_block: '█',
            partial_blocks: ['▏', '▎', '▍', '▌', '▋', '▊', '▉'],
        }
    }

    /// Create config optimized for animations (uses full blocks only)
    pub fn for_animation(mut self) -> Self {
        // When animations are minimized, use full blocks only
        self.partial_blocks = ['█', '█', '█', '█', '█', '█', '█'];
        self
    }
}

impl Default for ProgressBarConfig {
    fn default() -> Self {
        Self {
            filled_style: Style::default().fg(Color::Blue),
            unfilled_style: Style::default().fg(Color::DarkGray),
            label_style: Style::default().fg(Color::White),
            show_percentage: true,
            label_position: LabelPosition::None,
            animation_duration: Duration::from_millis(800),
            indeterminate_style: IndeterminateStyle::BackAndForth,
            full_block: '█',
            partial_blocks: ['▏', '▎', '▍', '▌', '▋', '▊', '▉'],
        }
    }
}

/// Internal state for indeterminate animation
#[derive(Debug)]
struct IndeterminateState {
    /// Current position (0.0 to 1.0)
    position: f64,
    /// Direction: 1.0 for forward, -1.0 for backward (for BackAndForth)
    direction: f64,
    /// Accumulated time for pulse effect
    accumulated_time: Duration,
}

impl IndeterminateState {
    fn new() -> Self {
        Self {
            position: 0.0,
            direction: 1.0,
            accumulated_time: Duration::ZERO,
        }
    }

    /// Reset animation state
    fn reset(&mut self) {
        self.position = 0.0;
        self.direction = 1.0;
        self.accumulated_time = Duration::ZERO;
    }
}

/// Progress bar widget
///
/// Can operate in two modes:
/// - **Determinate**: Shows fixed progress from 0.0 to 100.0
/// - **Indeterminate**: Shows animated busy indicator
///
/// # Examples
///
/// ```ignore
/// use tabitha::widget::{ProgressBar, LabelPosition};
///
/// // Simple determinate progress
/// let mut progress = ProgressBar::new();
/// progress.set_value(75.0);
///
/// // Busy indicator with label
/// let mut busy = ProgressBar::indeterminate()
///     .with_label("Loading...")
///     .with_label_position(LabelPosition::Left);
/// ```
#[derive(Debug)]
pub struct ProgressBar {
    /// Current value (0.0 to 100.0), None for indeterminate
    value: Option<f64>,
    /// Optional label text
    label: Option<String>,
    /// Configuration
    config: ProgressBarConfig,
    /// Animation state for indeterminate mode
    anim_state: IndeterminateState,
}

impl ProgressBar {
    /// Create a new determinate progress bar starting at 0%
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut progress = ProgressBar::new();
    /// progress.set_value(50.0);
    /// ```
    pub fn new() -> Self {
        Self {
            value: Some(0.0),
            label: None,
            config: ProgressBarConfig::default(),
            anim_state: IndeterminateState::new(),
        }
    }

    /// Create an indeterminate (busy) progress bar
    ///
    /// Shows an animated indicator without a fixed progress value.
    /// Useful for "thinking", "loading", or "waiting" states.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut thinking = ProgressBar::indeterminate()
    ///     .with_label("AI is thinking...");
    /// ```
    pub fn indeterminate() -> Self {
        Self {
            value: None,
            label: None,
            config: ProgressBarConfig::default(),
            anim_state: IndeterminateState::new(),
        }
    }

    /// Set progress value (0.0 to 100.0)
    ///
    /// Values outside this range are clamped.
    /// Setting a value switches the bar to determinate mode.
    ///
    /// # Example
    ///
    /// ```ignore
    /// progress.set_value(75.5);
    /// ```
    pub fn set_value(&mut self, value: f64) {
        self.value = Some(value.clamp(0.0, 100.0));
        // Reset animation state when switching to determinate
        self.anim_state.reset();
    }

    /// Set label text
    ///
    /// # Example
    ///
    /// ```ignore
    /// progress.set_label("Uploading file...");
    /// ```
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    /// Get current value
    ///
    /// Returns `None` if in indeterminate mode.
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// Check if in determinate mode
    pub fn is_determinate(&self) -> bool {
        self.value.is_some()
    }

    /// Check if in indeterminate mode
    pub fn is_indeterminate(&self) -> bool {
        self.value.is_none()
    }

    /// Set configuration
    pub fn with_config(mut self, config: ProgressBarConfig) -> Self {
        self.config = config;
        self
    }

    /// Set filled style
    pub fn with_filled_style(mut self, style: Style) -> Self {
        self.config.filled_style = style;
        self
    }

    /// Set unfilled style
    pub fn with_unfilled_style(mut self, style: Style) -> Self {
        self.config.unfilled_style = style;
        self
    }

    /// Set label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set label position
    pub fn with_label_position(mut self, pos: LabelPosition) -> Self {
        self.config.label_position = pos;
        self
    }

    /// Set indeterminate animation style
    pub fn with_indeterminate_style(mut self, style: IndeterminateStyle) -> Self {
        self.config.indeterminate_style = style;
        self
    }

    /// Set animation duration
    pub fn with_animation_duration(mut self, duration: Duration) -> Self {
        self.config.animation_duration = duration;
        self
    }

    /// Set whether to show percentage
    pub fn with_show_percentage(mut self, show: bool) -> Self {
        self.config.show_percentage = show;
        self
    }

    /// Apply theme to configuration
    pub fn with_theme(mut self, theme: &Theme) -> Self {
        self.config = ProgressBarConfig::from_theme(theme);
        self
    }

    /// Render the progress bar as a string of characters
    fn render_bar(&self, width: usize, filled_ratio: f64) -> String {
        if width == 0 {
            return String::new();
        }

        let filled = filled_ratio * width as f64;
        let full_blocks = filled.floor() as usize;
        let remainder = filled - full_blocks as f64;

        let mut result = String::with_capacity(width);

        // Add full blocks
        result.push_str(
            &self
                .config
                .full_block
                .to_string()
                .repeat(full_blocks.min(width)),
        );

        // Add partial block if needed
        if full_blocks < width && remainder > 0.001 {
            let partial_idx = (remainder * 7.0).min(6.0) as usize;
            result.push(self.config.partial_blocks[partial_idx]);
        }

        // Fill remaining with unfilled character (space)
        let current_len = result.chars().count();
        if current_len < width {
            result.push_str(&" ".repeat(width - current_len));
        }

        result
    }

    /// Draw the determinate progress bar
    fn draw_determinate(&self, frame: &mut Frame, area: Rect) {
        let value = self.value.unwrap_or(0.0);
        let percentage = format!("{:.0}%", value);

        // Calculate available width for the bar itself
        let label_width = self.label.as_ref().map(|l| l.len() + 1).unwrap_or(0);
        let pct_width = if self.config.show_percentage {
            percentage.len() + 1
        } else {
            0
        };
        let bar_width = area
            .width
            .saturating_sub(label_width as u16 + pct_width as u16);

        if bar_width == 0 {
            return;
        }

        let filled_ratio = value / 100.0;
        let bar_str = self.render_bar(bar_width as usize, filled_ratio);

        // Build the line
        let mut spans = Vec::new();

        // Label (if positioned left)
        if let Some(ref label) = self.label {
            if self.config.label_position == LabelPosition::Left {
                spans.push(Span::styled(label.clone(), self.config.label_style));
                spans.push(Span::raw(" "));
            }
        }

        // Progress bar
        spans.push(Span::styled(bar_str.clone(), self.config.filled_style));

        // Percentage
        if self.config.show_percentage {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(percentage, self.config.label_style));
        }

        // Label (if positioned right)
        if let Some(ref label) = self.label {
            if self.config.label_position == LabelPosition::Right {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(label.clone(), self.config.label_style));
            }
        }

        let line = Line::from(spans);
        frame.render_widget(ratatui::widgets::Paragraph::new(line), area);
    }

    /// Draw the indeterminate progress bar
    fn draw_indeterminate(&self, frame: &mut Frame, area: Rect) {
        let position = self.anim_state.position; // 0.0 to 1.0

        // Calculate available width
        let label_width = self.label.as_ref().map(|l| l.len() + 1).unwrap_or(0);
        let bar_width = area.width.saturating_sub(label_width as u16);

        if bar_width == 0 {
            return;
        }

        let bar_width_usize = bar_width as usize;

        // Create the animated bar based on style
        let bar_str = match self.config.indeterminate_style {
            IndeterminateStyle::BackAndForth => {
                // A block that moves back and forth (20% of width)
                let block_size = (bar_width_usize as f64 * 0.25).max(3.0) as usize;
                let max_start = bar_width_usize.saturating_sub(block_size);
                let start = (position * max_start as f64) as usize;

                let mut bar = vec![' '; bar_width_usize];
                for item in bar.iter_mut().skip(start).take(block_size) {
                    *item = self.config.full_block;
                }
                bar.into_iter().collect()
            }
            IndeterminateStyle::Marquee => {
                // A block that scrolls continuously
                let block_size = (bar_width_usize as f64 * 0.25).max(3.0) as usize;
                let start = (position * bar_width_usize as f64) as usize % bar_width_usize;

                let mut bar = vec![' '; bar_width_usize];
                for (_, item) in bar.iter_mut().enumerate().skip(start).take(block_size) {
                    *item = self.config.full_block;
                }
                bar.into_iter().collect()
            }
            IndeterminateStyle::Pulse => {
                // Full bar with varying "intensity" simulated by different characters
                let intensity = 0.3 + position * 0.7; // 0.3 to 1.0
                let _filled = (bar_width_usize as f64 * intensity) as usize;
                self.render_bar(bar_width_usize, intensity.min(1.0))
            }
        };

        // Build the line
        let mut spans = Vec::new();

        // Label (if positioned left)
        if let Some(ref label) = self.label {
            if self.config.label_position == LabelPosition::Left {
                spans.push(Span::styled(label.clone(), self.config.label_style));
                spans.push(Span::raw(" "));
            }
        }

        // Progress bar
        spans.push(Span::styled(bar_str, self.config.filled_style));

        // Label (if positioned right)
        if let Some(ref label) = self.label {
            if self.config.label_position == LabelPosition::Right {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(label.clone(), self.config.label_style));
            }
        }

        let line = Line::from(spans);
        frame.render_widget(ratatui::widgets::Paragraph::new(line), area);
    }

    /// Update animation state
    ///
    /// Returns true if the visual state changed and a redraw is needed.
    fn tick_internal(&mut self, elapsed: Duration, speed_multiplier: f32) -> bool {
        if self.is_determinate() {
            return false;
        }

        // Apply speed multiplier
        let adjusted_elapsed =
            Duration::from_nanos((elapsed.as_nanos() as f64 * speed_multiplier as f64) as u64);

        // Update accumulated time first
        self.anim_state.accumulated_time += adjusted_elapsed;

        let duration_ms = self.config.animation_duration.as_millis() as f64;
        let total_elapsed_ms = self.anim_state.accumulated_time.as_millis() as f64;

        match self.config.indeterminate_style {
            IndeterminateStyle::BackAndForth => {
                // Calculate progress within one full cycle (forward + back)
                let cycle_duration = duration_ms * 2.0;
                let progress = (total_elapsed_ms % cycle_duration) / cycle_duration;

                // Convert to position (0.0 -> 1.0 -> 0.0)
                if progress < 0.5 {
                    self.anim_state.position = progress * 2.0;
                } else {
                    self.anim_state.position = 2.0 - (progress * 2.0);
                }
            }
            IndeterminateStyle::Marquee => {
                // Continuous scroll
                let progress = (total_elapsed_ms % duration_ms) / duration_ms;
                self.anim_state.position = progress;
            }
            IndeterminateStyle::Pulse => {
                // Sine wave between 0.3 and 1.0
                let cycle_duration = duration_ms;
                let progress = (total_elapsed_ms % cycle_duration) / cycle_duration;
                // Sine wave: 0.5 + 0.5 * sin(2 * PI * progress - PI/2) gives 0.0 to 1.0
                let sine = 0.5
                    + 0.5
                        * (2.0 * std::f64::consts::PI * progress - std::f64::consts::FRAC_PI_2)
                            .sin();
                // Scale to 0.3 to 1.0
                self.anim_state.position = 0.3 + sine * 0.7;
            }
        }

        true // Always needs redraw for animation
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Control for ProgressBar {
    type Event = ProgressBarEvent;

    fn draw(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        if self.is_determinate() {
            self.draw_determinate(frame, area);
        } else {
            self.draw_indeterminate(frame, area);
        }
    }

    fn handle_event(&mut self, _event: &Event) -> EventResult {
        // Progress bar doesn't handle input events
        EventResult::Unhandled
    }

    fn tick(&mut self, ctx: &mut ControlAnimationContext<'_>) -> bool {
        // Only animate if animations are enabled and we're in indeterminate mode
        if !ctx.is_enabled() || self.is_determinate() {
            return false;
        }

        // Check animation mode
        let mode = ctx.mode();

        if mode == AnimationMode::None {
            // Static - no animation
            return false;
        }

        // Use a fixed time step for consistent animation
        // This simulates elapsed time while respecting speed multiplier
        let base_elapsed = Duration::from_millis(16); // ~60fps
        let speed = ctx.speed_multiplier();
        let elapsed = Duration::from_nanos((base_elapsed.as_nanos() as f32 * speed) as u64);

        // In reduced mode, use simple blink (on/off) instead of movement
        if mode == AnimationMode::Reduced {
            // Simple blink: just toggle visibility based on accumulated time
            self.anim_state.accumulated_time += elapsed;
            let blink_duration = Duration::from_millis(500); // 500ms blink
            let cycle =
                self.anim_state.accumulated_time.as_millis() % (blink_duration.as_millis() * 2);
            // Show when in first half of cycle
            self.anim_state.position = if cycle < blink_duration.as_millis() {
                1.0
            } else {
                0.0
            };
            return true;
        }

        self.tick_internal(elapsed, 1.0) // Speed already applied to elapsed
    }

    fn take_events(&mut self) -> Vec<Self::Event> {
        // ProgressBar doesn't emit events
        Vec::new()
    }

    fn has_events(&self) -> bool {
        false
    }

    fn on_focus(&mut self) {
        // No special focus behavior
    }

    fn on_blur(&mut self) {
        // No special blur behavior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_new() {
        let pb = ProgressBar::new();
        assert!(pb.is_determinate());
        assert_eq!(pb.value(), Some(0.0));
    }

    #[test]
    fn test_progress_bar_indeterminate() {
        let pb = ProgressBar::indeterminate();
        assert!(pb.is_indeterminate());
        assert_eq!(pb.value(), None);
    }

    #[test]
    fn test_set_value() {
        let mut pb = ProgressBar::new();
        pb.set_value(75.0);
        assert_eq!(pb.value(), Some(75.0));

        // Test clamping
        pb.set_value(150.0);
        assert_eq!(pb.value(), Some(100.0));

        pb.set_value(-10.0);
        assert_eq!(pb.value(), Some(0.0));
    }

    #[test]
    fn test_render_bar() {
        let pb = ProgressBar::new();

        // Empty bar
        let result = pb.render_bar(10, 0.0);
        assert_eq!(result.chars().count(), 10);

        // Full bar
        let result = pb.render_bar(10, 1.0);
        assert_eq!(result, "██████████");

        // Half bar
        let result = pb.render_bar(10, 0.5);
        assert_eq!(result.chars().count(), 10);
    }
}
