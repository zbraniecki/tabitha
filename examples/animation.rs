//! Animation example showcasing different animation modes in tabitha.
//!
//! This example demonstrates:
//! - ProgressBar with indeterminate/busy animations
//! - TextBox with cursor blink animations
//! - Three animation modes: Full, Reduced, None
//! - Sidebar with toggle/resize/move controls
//! - Cycling through animation modes with 'a' key
//!
//! Controls:
//! - b: Toggle sidebar visibility
//! - [ ]: Decrease/increase sidebar width
//! - l/r: Move sidebar to left/right side
//! - a: Cycle animation mode (Full -> Reduced -> None -> Full)
//! - Type in text boxes to see cursor animation
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
use tabitha::widget::{Control, IndeterminateStyle, LabelPosition, ProgressBar, TextBox};
use tabitha::{
    AnimationMode, AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult,
    KeyCode, MainUi, Side, Sidebar, SidebarState,
};

// =============================================================================
// Main Application
// =============================================================================

/// The main application showcasing animations with sidebar
struct AnimationApp {
    sidebar_state: SidebarState,
    progress_bar_backforth: ProgressBar,
    progress_bar_marquee: ProgressBar,
    progress_bar_pulse: ProgressBar,
    textbox1: TextBox,
    textbox2: TextBox,
    width_level: u8,
    animation_mode: AnimationMode,
    last_tick: Instant,
}

impl AnimationApp {
    fn new() -> Self {
        // Create sidebar state
        let sidebar_state = SidebarState::new(Side::Left)
            .with_width(Constraint::Percentage(25))
            .with_animations(true)
            .with_animation_duration(Duration::from_millis(200));

        // Create progress bars with different animation styles
        let progress_bar_backforth = ProgressBar::indeterminate()
            .with_label("BackAndForth")
            .with_label_position(LabelPosition::Left)
            .with_indeterminate_style(IndeterminateStyle::BackAndForth)
            .with_animation_duration(Duration::from_millis(800));

        let progress_bar_marquee = ProgressBar::indeterminate()
            .with_label("Marquee")
            .with_label_position(LabelPosition::Left)
            .with_indeterminate_style(IndeterminateStyle::Marquee)
            .with_animation_duration(Duration::from_millis(600));

        let progress_bar_pulse = ProgressBar::indeterminate()
            .with_label("Pulse")
            .with_label_position(LabelPosition::Left)
            .with_indeterminate_style(IndeterminateStyle::Pulse)
            .with_animation_duration(Duration::from_millis(1000));

        // Create text boxes for cursor animation demo
        let textbox1 = TextBox::new("input1").with_placeholder("Type here...");
        let textbox2 = TextBox::new("input2").with_placeholder("And here...");

        Self {
            sidebar_state,
            progress_bar_backforth,
            progress_bar_marquee,
            progress_bar_pulse,
            textbox1,
            textbox2,
            width_level: 1, // 0=15%, 1=25%, 2=35%
            animation_mode: AnimationMode::Full,
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
}

impl Component for AnimationApp {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        // Layout: Main area with sidebar/content split, footer at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Main area (sidebar + content)
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Split main area between sidebar and content
        let [content_area, sidebar_area] = Sidebar::new(&self.sidebar_state, chunks[0]);

        // Draw content area (progress bars + text boxes)
        self.draw_content(frame, content_area);

        // Draw sidebar
        self.draw_sidebar(frame, sidebar_area);

        // Draw footer at bottom
        self.draw_footer(frame, chunks[1]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Quit on Ctrl+C or Ctrl+Q
        if event.is_quit() {
            ctx.quit();
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
                    self.last_tick = Instant::now();
                    EventResult::Handled
                }
                // Decrease width
                KeyCode::Char('[') => {
                    if self.width_level > 0 {
                        self.width_level -= 1;
                        self.update_sidebar_width();
                        self.last_tick = Instant::now();
                    }
                    EventResult::Handled
                }
                // Increase width
                KeyCode::Char(']') => {
                    if self.width_level < 2 {
                        self.width_level += 1;
                        self.update_sidebar_width();
                        self.last_tick = Instant::now();
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
                // Cycle animation mode (Full -> Reduced -> None -> Full)
                KeyCode::Char('a') => {
                    if let Some(mut anim_ctx) = ctx.control_animations() {
                        self.animation_mode = anim_ctx.cycle_mode();
                        // Only Full mode animates sidebar transitions;
                        // Reduced and None jump instantly.
                        self.sidebar_state
                            .set_animations_enabled(self.animation_mode == AnimationMode::Full);
                    }
                    EventResult::Handled
                }
                _ => {
                    // Forward to textboxes
                    let result1 = self.textbox1.handle_event(event);
                    if result1.is_handled() {
                        return result1;
                    }
                    self.textbox2.handle_event(event)
                }
            }
        } else {
            EventResult::Unhandled
        }
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        // Update sidebar animations with elapsed time since last tick
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick);
        self.last_tick = now;
        self.sidebar_state.tick(elapsed);
        let sidebar_animating = self.sidebar_state.is_animating();

        // Update progress bar and textbox animations
        let controls_changed = if let Some(mut anim_ctx) = ctx.control_animations() {
            let a = self.progress_bar_backforth.tick(&mut anim_ctx);
            let b = self.progress_bar_marquee.tick(&mut anim_ctx);
            let c = self.progress_bar_pulse.tick(&mut anim_ctx);
            let d = self.textbox1.tick(&mut anim_ctx);
            let e = self.textbox2.tick(&mut anim_ctx);
            a || b || c || d || e
        } else {
            false
        };

        sidebar_animating || controls_changed
    }
}

impl AnimationApp {
    fn draw_content(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Animation Demo ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Layout: progress bars on top, text boxes below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(12), // Progress bars section
                Constraint::Min(0),     // Text boxes section
            ])
            .split(inner);

        // Draw progress bars section
        self.draw_progress_bars(frame, chunks[0]);

        // Draw text boxes section
        self.draw_textboxes(frame, chunks[1]);
    }

    fn draw_progress_bars(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Progress Bars ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // BackAndForth
                Constraint::Length(3), // Marquee
                Constraint::Length(3), // Pulse
            ])
            .split(inner);

        self.progress_bar_backforth.draw(frame, chunks[0], false);
        self.progress_bar_marquee.draw(frame, chunks[1], false);
        self.progress_bar_pulse.draw(frame, chunks[2], false);
    }

    fn draw_textboxes(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Text Boxes (Cursor Animation) ")
            .borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(5), // TextBox 1
                Constraint::Length(5), // TextBox 2
                Constraint::Min(0),    // Info
            ])
            .split(inner);

        self.textbox1.draw(frame, chunks[0], true);
        self.textbox2.draw(frame, chunks[1], false);

        // Draw mode info
        let mode_text = format!(
            "Mode: {:?} | Full: Normal | Reduced: Fast/Simple | None: Static",
            self.animation_mode
        );
        let info = Paragraph::new(mode_text).style(Style::default().fg(Color::White));
        frame.render_widget(info, chunks[2]);
    }

    fn draw_sidebar(&self, frame: &mut Frame, area: Rect) {
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
            Animation: {:?}",
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
            self.animation_mode,
        );

        let status_para = Paragraph::new(status_text).style(Style::default().fg(Color::White));
        frame.render_widget(status_para, sidebar_inner);
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let mode_str = match self.animation_mode {
            AnimationMode::Full => "Full",
            AnimationMode::Reduced => "Reduced",
            AnimationMode::None => "None",
        };

        let controls = format!(
            "b: Toggle Sidebar | [ ]: Width | l/r: Side | a: Mode ({}) | q: Quit",
            mode_str
        );

        let footer = Paragraph::new(controls)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, area);
    }
}

impl MainUi for AnimationApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    println!("Animation Example - Controls:");
    println!("  b: Toggle sidebar visibility");
    println!("  [ ]: Decrease/increase sidebar width");
    println!("  l/r: Move sidebar to left/right side");
    println!("  a: Cycle animation mode (Full -> Reduced -> None)");
    println!("  Type in text boxes to see cursor animation");
    println!("  q: Quit");
    println!();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(AnimationApp::new())
        .mouse_capture(false)
        .tick_rate(Duration::from_millis(16)) // ~60fps for smooth animations
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
