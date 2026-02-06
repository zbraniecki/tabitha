//! TextBox widget example demonstrating text input in tabitha.
//!
//! This example shows:
//! - Two text boxes: username and password
//! - Tab to switch focus between fields
//! - Text input with cursor navigation
//! - Cursor animation (fade and blink modes)
//! - Password masking
//! - Event handling for submit (Enter key)
//!
//! Cursor Animation Modes:
//! - Username: Fade cursor (smooth transition between dim and bright)
//! - Password: Blink cursor (traditional on/off blinking)
//!
//! Controls:
//! - Tab: Switch focus between text boxes
//! - Enter: Submit the form
//! - Arrow keys: Move cursor within text
//! - Home/End: Jump to start/end of text
//! - Ctrl+A/E: Jump to start/end (Emacs style)
//! - Ctrl+W: Delete word backward
//! - Ctrl+U: Delete to start of line
//! - Ctrl+K: Delete to end of line
//! - q (when not in text box) or Ctrl+C: Quit

#[path = "_common/mod.rs"]
mod common;
use clap::Parser;
use common::Args;

use std::time::Duration;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tabitha::{
    widget::{
        Control, CursorAnimationMode, CursorFadeConfig, TextBox, TextBoxConfig, TextBoxEvent,
    },
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, HasFocus, KeyCode,
    MainUi,
};

// =============================================================================
// Login Form Application
// =============================================================================

struct LoginForm {
    username: TextBox,
    password: TextBox,
    status_message: String,
    submitted_data: Option<(String, String)>,
}

impl LoginForm {
    fn new() -> Self {
        // Create username text box with fade animation
        let username_config = TextBoxConfig {
            cursor_mode: CursorAnimationMode::Fade,
            cursor_fade: Some(CursorFadeConfig::default()),
            ..TextBoxConfig::default()
        };

        Self {
            username: TextBox::builder("username")
                .title("Username (fade cursor)")
                .placeholder("Enter your username...")
                .config(username_config)
                .build(),
            password: TextBox::builder("password")
                .title("Password (blink cursor)")
                .placeholder("Enter your password...")
                .config(TextBoxConfig::password())
                .build(),
            status_message: "Tab to switch, Enter to submit | Top: fade cursor, Bottom: blink"
                .to_string(),
            submitted_data: None,
        }
    }

    fn focused_control(
        &mut self,
        focus_id: Option<&str>,
    ) -> Option<&mut dyn Control<Event = TextBoxEvent>> {
        match focus_id {
            Some("username") => Some(&mut self.username),
            Some("password") => Some(&mut self.password),
            _ => None,
        }
    }

    fn process_events(&mut self, textbox_id: &str, events: Vec<TextBoxEvent>) {
        for event in events {
            match event {
                TextBoxEvent::Submit(_) => {
                    // Form submitted
                    let username = self.username.text().to_string();
                    let password = self.password.text().to_string();

                    if username.is_empty() || password.is_empty() {
                        self.status_message = "Please fill in both fields!".to_string();
                    } else {
                        self.status_message =
                            format!("Submitted! Username: '{}', Password: [hidden]", username);
                        self.submitted_data = Some((username, password));
                    }
                }
                TextBoxEvent::Changed(text) => {
                    self.status_message = format!("{} changed: {} chars", textbox_id, text.len());
                }
                TextBoxEvent::FocusGained => {
                    self.status_message = format!("Editing {}", textbox_id);
                }
                _ => {}
            }
        }
    }
}

impl Component for LoginForm {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Create layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Username field
                Constraint::Length(1), // Spacing
                Constraint::Length(3), // Password field
                Constraint::Length(2), // Spacing
                Constraint::Length(3), // Status
                Constraint::Min(0),    // Remaining space
                Constraint::Length(3), // Footer
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Login Form Example")
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::NONE));
        frame.render_widget(title, chunks[0]);

        // Username field
        let username_focused = ctx.focus().is_focused("username");
        self.username.draw(frame, chunks[1], username_focused);

        // Password field
        let password_focused = ctx.focus().is_focused("password");
        self.password.draw(frame, chunks[3], password_focused);

        // Status message
        let status_style = if self.submitted_data.is_some() {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Yellow)
        };
        let status = Paragraph::new(self.status_message.as_str())
            .style(status_style)
            .block(Block::default().borders(Borders::ALL).title(" Status "));
        frame.render_widget(status, chunks[5]);

        // Footer with controls
        let footer = Paragraph::new(
            "Tab: Switch fields | Enter: Submit | Ctrl+C: Quit | Arrow keys: Move cursor",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(footer, chunks[7]);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Handle quit
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Handle tab navigation
        if event.is_key(KeyCode::Tab) {
            // Get current focus for debugging
            let old_focus = ctx.focus().focused_id().map(|s| s.to_string());

            // Notify current control of blur
            if let Some(ref id) = old_focus {
                if let Some(control) = self.focused_control(Some(id)) {
                    control.on_blur();
                    let events = control.take_events();
                    self.process_events(id, events);
                }
            }

            // Move focus
            let moved = ctx.focus().focus_next();

            // Get new focus for debugging
            let new_focus = ctx.focus().focused_id().map(|s| s.to_string());

            // Notify new control of focus
            if let Some(ref id) = new_focus {
                if let Some(control) = self.focused_control(Some(id)) {
                    control.on_focus();
                    let events = control.take_events();
                    self.process_events(id, events);
                }
            }

            // Update status with focus change info
            self.status_message = format!(
                "Focus: {:?} -> {:?} (moved: {})",
                old_focus, new_focus, moved
            );

            return EventResult::Handled;
        }

        // Forward event to focused control
        let focus_id = ctx.focus().focused_id().map(|s| s.to_string());
        if let Some(ref id) = focus_id {
            if let Some(control) = self.focused_control(Some(id)) {
                let result = control.handle_event(event);
                // Process any events from the control
                let events = control.take_events();
                self.process_events(id, events);
                return result;
            }
        }

        EventResult::Unhandled
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        // Tick the text boxes for cursor blinking
        if let Some(mut anim_ctx) = ctx.control_animations() {
            let a = self.username.tick(&mut anim_ctx);
            let b = self.password.tick(&mut anim_ctx);
            a || b
        } else {
            false
        }
    }
}

impl MainUi for LoginForm {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(LoginForm::new())
        .register_focus("username")
        .register_focus("password")
        .initial_focus("username")
        .tick_rate(Duration::from_millis(100)) // For cursor blinking
        .mouse_capture(false)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
