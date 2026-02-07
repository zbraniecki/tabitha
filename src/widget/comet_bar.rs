//! Comet-style progress bar widget for showing network/request progress.
//!
//! Displays an 8-character wide bar with a "comet tail" animation effect:
//! - 5 dim levels: 0 (full) 1 2 3 4 (barely visible/dot)
//! - Specific back-and-forth animation sequence
//! - Half-height display (vertically centered)
//!
//! Animation sequence (8 chars, positions 0-7):
//! 1.  ********
//! 2.  0*******
//! 3.  10******
//! 4.  210*****
//! 5.  3210****
//! 6.  43210***
//! 7.  *43210**
//! 8.  **43210*
//! 9.  ***43210
//! 10. ****4321
//! 11. *****432
//! 12. ******43
//! 13. *******4
//! 14. ********
//! 15. *******0
//! 16. ******01
//! 17. *****012
//! ... (continues back and forth)
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::CometBar;
//! use ratatui::style::Color;
//!
//! let mut comet = CometBar::new();
//! comet.set_color(Color::Blue);
//!
//! // In draw method:
//! comet.draw(frame, area, false);
//!
//! // In tick method:
//! comet.tick(ctx.control_animations().as_mut().unwrap());
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

/// Events emitted by CometBar (empty for now)
#[derive(Debug, Clone)]
pub enum CometBarEvent {}

impl ControlEvent for CometBarEvent {}

/// Configuration for CometBar appearance
#[derive(Debug, Clone)]
pub struct CometBarConfig {
    /// Width of the bar in characters (8)
    pub width: usize,
    /// Color for the comet
    pub color: Color,
    /// Time per frame in milliseconds
    pub frame_duration_ms: u64,
    /// Characters for 5 dim levels (0=brightest to 4=dimmest)
    pub dim_chars: [char; 5],
    /// Background character
    pub bg_char: char,
}

impl Default for CometBarConfig {
    fn default() -> Self {
        Self {
            width: 8,
            color: Color::Blue,
            frame_duration_ms: 100, // 100ms per frame = 10 fps
            dim_chars: ['█', '▓', '▒', '░', '·'],
            bg_char: '·',
        }
    }
}

/// Animation state following the exact sequence
#[derive(Debug)]
struct CometState {
    /// Current frame in the animation sequence (0-16 for one full cycle)
    frame: usize,
    /// Accumulated time
    accumulated_time: Duration,
}

impl CometState {
    fn new() -> Self {
        Self {
            frame: 0,
            accumulated_time: Duration::ZERO,
        }
    }

    fn reset(&mut self) {
        self.frame = 0;
        self.accumulated_time = Duration::ZERO;
    }
}

/// Comet-style progress bar widget with exact back-and-forth animation
#[derive(Debug)]
pub struct CometBar {
    config: CometBarConfig,
    state: CometState,
    /// Current animation mode
    animation_mode: AnimationMode,
}

impl CometBar {
    /// Create a new comet bar with default configuration
    pub fn new() -> Self {
        Self {
            config: CometBarConfig::default(),
            state: CometState::new(),
            animation_mode: AnimationMode::Full,
        }
    }

    /// Create a comet bar with custom configuration
    pub fn with_config(config: CometBarConfig) -> Self {
        Self {
            config,
            state: CometState::new(),
            animation_mode: AnimationMode::Full,
        }
    }

    /// Set the bar color (matches mode color)
    pub fn set_color(&mut self, color: Color) {
        self.config.color = color;
    }

    /// Set the bar color (builder pattern)
    pub fn with_color(mut self, color: Color) -> Self {
        self.config.color = color;
        self
    }

    /// Reset animation to initial state
    pub fn reset(&mut self) {
        self.state.reset();
    }

    /// Get dimmed color based on level (0-4, 0=brightest/full)
    fn dimmed_color(&self, level: usize) -> Color {
        let ratio = 1.0 - (level as f32 / 4.0);

        if let Color::Rgb(r, g, b) = self.config.color {
            let new_r = (r as f32 * ratio) as u8;
            let new_g = (g as f32 * ratio) as u8;
            let new_b = (b as f32 * ratio) as u8;
            Color::Rgb(new_r, new_g, new_b)
        } else {
            self.config.color
        }
    }

    /// Render the current frame based on animation state
    fn render(&self) -> Vec<(char, Color)> {
        let width = self.config.width;
        let mut result: Vec<(char, Color)> =
            vec![(self.config.bg_char, self.dimmed_color(4)); width];

        // Map frame number to head position and tail length
        // Frames 0-13: forward (head 0->7)
        // Frames 14-17+: backward (head 7->4 and continuing)
        let head_pos: Option<usize>;
        let tail_len: usize;
        match self.state.frame {
            0 => {
                head_pos = None;
                tail_len = 0;
            } // ********
            1 => {
                head_pos = Some(0);
                tail_len = 1;
            } // 0*******
            2 => {
                head_pos = Some(1);
                tail_len = 2;
            } // 10******
            3 => {
                head_pos = Some(2);
                tail_len = 3;
            } // 210*****
            4 => {
                head_pos = Some(3);
                tail_len = 4;
            } // 3210****
            5 => {
                head_pos = Some(4);
                tail_len = 5;
            } // 43210***
            6 => {
                head_pos = Some(5);
                tail_len = 5;
            } // *43210**
            7 => {
                head_pos = Some(6);
                tail_len = 5;
            } // **43210*
            8 => {
                head_pos = Some(7);
                tail_len = 5;
            } // ***43210
            9 => {
                head_pos = Some(7);
                tail_len = 4;
            } // ****4321
            10 => {
                head_pos = Some(7);
                tail_len = 3;
            } // *****432
            11 => {
                head_pos = Some(7);
                tail_len = 2;
            } // ******43
            12 => {
                head_pos = Some(7);
                tail_len = 1;
            } // *******4
            13 => {
                head_pos = None;
                tail_len = 0;
            } // ********
            // Backward
            14 => {
                head_pos = Some(6);
                tail_len = 1;
            } // *******0
            15 => {
                head_pos = Some(5);
                tail_len = 2;
            } // ******01
            16 => {
                head_pos = Some(4);
                tail_len = 3;
            } // *****012
            17 => {
                head_pos = Some(3);
                tail_len = 4;
            } // ****0123
            18 => {
                head_pos = Some(2);
                tail_len = 5;
            } // ***01234
            19 => {
                head_pos = Some(1);
                tail_len = 5;
            } // **01234*
            20 => {
                head_pos = Some(0);
                tail_len = 5;
            } // *01234**
            21 => {
                head_pos = Some(0);
                tail_len = 4;
            } // 0123***
            22 => {
                head_pos = Some(0);
                tail_len = 3;
            } // 012****
            23 => {
                head_pos = Some(0);
                tail_len = 2;
            } // 01*****
            24 => {
                head_pos = Some(0);
                tail_len = 1;
            } // 0******
            25 => {
                head_pos = None;
                tail_len = 0;
            } // ********
            _ => {
                head_pos = None;
                tail_len = 0;
            }
        }

        if let Some(head) = head_pos {
            // Draw tail (dimmer chars behind head)
            for i in 0..tail_len {
                let pos: usize = if self.state.frame <= 13 {
                    // Forward: tail extends to the left
                    head.saturating_sub(i + 1)
                } else {
                    // Backward: tail extends to the right
                    (head + i + 1).min(width - 1)
                };

                if pos < width {
                    let dim_level = i + 1; // 1, 2, 3, 4, 5
                    let char_idx = dim_level.min(4); // Clamp to 4 max
                    result[pos] = (self.config.dim_chars[char_idx], self.dimmed_color(char_idx));
                }
            }

            // Draw head (brightest)
            result[head] = (self.config.dim_chars[0], self.dimmed_color(0));
        }

        result
    }

    /// Update animation state
    fn tick_internal(&mut self, elapsed: Duration) -> bool {
        self.state.accumulated_time += elapsed;

        let frame_duration = Duration::from_millis(self.config.frame_duration_ms);
        let total_frames: usize = 26; // One complete back-and-forth cycle
        let total_duration = frame_duration * total_frames as u32;

        // Calculate current frame
        let cycle_time = self.state.accumulated_time.as_millis() % total_duration.as_millis();
        let new_frame =
            (cycle_time as f64 / frame_duration.as_millis() as f64) as usize % total_frames;

        let changed = new_frame != self.state.frame;
        self.state.frame = new_frame;

        changed
    }
}

impl Default for CometBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Control for CometBar {
    type Event = CometBarEvent;

    fn draw(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Only use 1 row, centered vertically if multiple rows available
        let draw_y = area.y + area.height / 2;
        if draw_y >= area.y + area.height {
            return;
        }

        let draw_area = Rect {
            x: area.x,
            y: draw_y,
            width: area.width.min(self.config.width as u16),
            height: 1,
        };

        // Get the rendered characters
        let chars = self.render();

        // Build spans
        let spans: Vec<Span> = chars
            .into_iter()
            .map(|(ch, color)| Span::styled(ch.to_string(), Style::default().fg(color)))
            .collect();

        let line = ratatui::text::Line::from(spans);
        frame.render_widget(ratatui::widgets::Paragraph::new(line), draw_area);
    }

    fn handle_event(&mut self, _event: &Event) -> EventResult {
        // CometBar doesn't handle input events
        EventResult::Unhandled
    }

    fn tick(&mut self, ctx: &mut ControlAnimationContext<'_>) -> bool {
        if !ctx.is_enabled() {
            return false;
        }

        let mode = ctx.mode();
        let mode_changed = self.animation_mode != mode;
        self.animation_mode = mode;

        if mode == AnimationMode::None {
            // Static - no animation
            return mode_changed; // Redraw once if mode changed
        }

        // Use a fixed time step for consistent animation
        let base_elapsed = Duration::from_millis(16); // ~60fps
        let speed = ctx.speed_multiplier();
        let elapsed = Duration::from_nanos((base_elapsed.as_nanos() as f32 * speed) as u64);

        if mode == AnimationMode::Reduced {
            // Reduced mode: slower updates (200ms per frame = 5 fps)
            self.state.accumulated_time += elapsed;
            let update_interval = Duration::from_millis(200);

            if self.state.accumulated_time < update_interval {
                return mode_changed;
            }

            // Use just one "tick" worth
            let tick_elapsed = update_interval;
            self.state.accumulated_time = Duration::ZERO;
            let tick_changed = self.tick_internal(tick_elapsed);
            mode_changed || tick_changed
        } else {
            // Full animation mode
            let tick_changed = self.tick_internal(elapsed);
            mode_changed || tick_changed
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
    fn test_comet_bar_new() {
        let bar = CometBar::new();
        assert_eq!(bar.config.width, 8);
        assert_eq!(bar.state.frame, 0);
    }

    #[test]
    fn test_comet_bar_with_color() {
        let bar = CometBar::new().with_color(Color::Yellow);
        assert_eq!(bar.config.color, Color::Yellow);
    }

    #[test]
    fn test_comet_bar_reset() {
        let mut bar = CometBar::new();
        bar.state.frame = 10;
        bar.reset();
        assert_eq!(bar.state.frame, 0);
    }

    #[test]
    fn test_render_frame_0() {
        let bar = CometBar::new();
        let rendered = bar.render();
        assert_eq!(rendered.len(), 8);
        // All should be background
        assert_eq!(rendered[0].0, '·');
    }

    #[test]
    fn test_render_frame_1() {
        let mut bar = CometBar::new();
        bar.state.frame = 1;
        let rendered = bar.render();
        assert_eq!(rendered[0].0, '█'); // Head at position 0
    }
}
