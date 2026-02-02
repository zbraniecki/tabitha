//! TextBox widget example demonstrating text input in tabitha.
//!
//! This example shows:
//! - Two text boxes: username and password
//! - Tab to switch focus between fields
//! - Text input with cursor navigation
//! - Cursor blinking when focused
//! - Password masking
//! - Event handling for submit (Enter key)
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
    widget::{Control, TextBox, TextBoxConfig, TextBoxEvent},
    AppBuilder, AppContext, Component, DrawContext, Event, EventResult, KeyCode, MainUi,
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
        Self {
            username: TextBox::new("username")
                .with_title("Username")
                .with_placeholder("Enter your username..."),
            password: TextBox::new("password")
                .with_title("Password")
                .with_placeholder("Enter your password...")
                .with_config(TextBoxConfig::password()),
            status_message: "Press Tab to switch fields, Enter to submit".to_string(),
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
            // Notify current control of blur
            let current_focus = ctx.focus().focused_id().map(|s| s.to_string());
            if let Some(ref id) = current_focus {
                if let Some(control) = self.focused_control(Some(id)) {
                    control.on_blur();
                    // Process any blur events
                    let events = control.take_events();
                    self.process_events(id, events);
                }
            }

            // Move focus
            ctx.focus().focus_next();

            // Notify new control of focus
            let new_focus = ctx.focus().focused_id().map(|s| s.to_string());
            if let Some(ref id) = new_focus {
                if let Some(control) = self.focused_control(Some(id)) {
                    control.on_focus();
                    // Process any focus events
                    let events = control.take_events();
                    self.process_events(id, events);
                }
            }

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

    fn tick(&mut self, _ctx: &mut AppContext) {
        // Tick the text boxes for cursor blinking
        self.username.tick();
        self.password.tick();
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
        .enable_dev_console(args.dev)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
