//! Example demonstrating the Modal dialog system with centralized ModalManager.
//!
//! This example shows:
//! - Opening modals via ctx.modal().open()
//! - Checking modal results via ctx.modal().take_result()
//! - Single-button alert modal
//! - Two-button confirmation modal
//! - Three-button save dialog modal
//! - Input prompt modal (with text field)
//! - Integration with TextBox control
//! - Automatic modal rendering by the framework
//!
//! Run with: `cargo run --example modal`
//!
//! Controls:
//! - 1: Show alert modal
//! - 2: Show confirmation modal
//! - 3: Show save dialog modal
//! - 4: Show rapid succession modals (demonstrates replacement)
//! - 5: Show prompt modal (with input field)
//! - Ctrl+C/Ctrl+Q: Quit

use std::time::Duration;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use tabitha::{
    AppBuilder, AppContext, Component, Control, DrawContext, Event, EventResult, KeyCode, MainUi,
    Modal, ModalButton, ModalResult, TextBox,
};

/// Main application state.
struct ModalExample {
    /// Text box for user input.
    textbox: TextBox,
    /// Status message displayed at the bottom.
    status: String,
}

impl ModalExample {
    fn new() -> Self {
        Self {
            textbox: TextBox::new("input")
                .with_title("Enter text")
                .with_placeholder("Type something here..."),
            status: "Press 1-5 for different modals".to_string(),
        }
    }

    /// Create an alert modal.
    fn create_alert_modal() -> Modal {
        Modal::new("alert", "This is an informational alert message.")
            .with_title("Alert")
            .with_button(ModalButton::new("ok", "OK"))
    }

    /// Create a confirmation modal.
    fn create_confirm_modal() -> Modal {
        Modal::new(
            "confirm",
            "Are you sure you want to delete this item?\n\nThis action cannot be undone.",
        )
        .with_title("Confirm Delete")
        .with_button(ModalButton::new("delete", "Delete"))
        .with_button(ModalButton::new("cancel", "Cancel"))
    }

    /// Create a save dialog modal.
    fn create_save_modal() -> Modal {
        Modal::new(
            "save",
            "You have unsaved changes.\n\nDo you want to save before closing?",
        )
        .with_title("Save Changes?")
        .with_button(ModalButton::new("save", "Save"))
        .with_button(ModalButton::new("discard", "Don't Save"))
        .with_button(ModalButton::new("cancel", "Cancel"))
    }

    /// Create a prompt modal with an inline input field.
    fn create_prompt_modal() -> Modal {
        Modal::new("prompt", "Please enter your name:")
            .with_title("Enter Name")
            .with_inline_input()
            .with_button(ModalButton::new("ok", "OK"))
            .with_button(ModalButton::new("cancel", "Cancel"))
            .with_size(50, 20) // Compact modal with inline input
    }

    /// Handle the result of a modal interaction.
    fn handle_modal_result(&mut self, modal_id: &str, result: ModalResult) {
        match result {
            ModalResult::ButtonPressed { button_id, input } => {
                // Show different messages depending on whether there was input
                if let Some(ref input_value) = input {
                    self.status = format!(
                        "Modal '{}': Button '{}', Input: '{}'",
                        modal_id, button_id, input_value
                    );
                } else {
                    self.status = format!("Modal '{}': Button '{}' pressed", modal_id, button_id);
                }

                // Handle specific actions
                match (modal_id, button_id.as_str()) {
                    ("confirm", "delete") => {
                        self.textbox.clear();
                        self.status = "Item deleted! (TextBox cleared)".to_string();
                    }
                    ("save", "save") => {
                        self.status = format!("Saved! Text: '{}'", self.textbox.text());
                    }
                    ("save", "discard") => {
                        self.textbox.clear();
                        self.status = "Changes discarded! (TextBox cleared)".to_string();
                    }
                    ("prompt", "ok") => {
                        if let Some(name) = input {
                            if name.is_empty() {
                                self.status = "You didn't enter a name!".to_string();
                            } else {
                                self.status = format!("Hello, {}!", name);
                                // Optionally set the textbox to the entered name
                                self.textbox.set_text(&name);
                            }
                        }
                    }
                    ("prompt", "cancel") => {
                        self.status = "Prompt cancelled.".to_string();
                    }
                    _ => {}
                }
            }
            ModalResult::Dismissed => {
                self.status = format!("Modal '{}': Dismissed with Escape", modal_id);
            }
            ModalResult::Closed => {
                self.status = format!("Modal '{}': Closed (replaced by another modal)", modal_id);
            }
            ModalResult::Pending => {
                // Modal is still open, no action needed
            }
        }
    }
}

impl Component for ModalExample {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Layout: title, textbox, instructions, status
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // TextBox
                Constraint::Min(5),    // Instructions
                Constraint::Length(3), // Status
            ])
            .split(area);

        // Title
        let title = Paragraph::new("Modal Dialog Example (Centralized ModalManager)")
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(title, chunks[0]);

        // TextBox
        self.textbox
            .draw(frame, chunks[1], ctx.focus().is_focused("input"));

        // Instructions
        let instructions = Paragraph::new(
            "Keyboard shortcuts:\n\n\
             1 - Alert modal (single OK button)\n\
             2 - Confirmation modal (Delete/Cancel)\n\
             3 - Save dialog (Save/Don't Save/Cancel)\n\
             4 - Rapid modals (demonstrates replacement)\n\
             5 - Prompt modal (with input field!)\n\n\
             In modal: Tab to switch focus, Enter to submit, Escape to dismiss\n\n\
             Note: Modal is drawn by the framework - no manual drawing needed!",
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Instructions "),
        );
        frame.render_widget(instructions, chunks[2]);

        // Status
        let status = Paragraph::new(self.status.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title(" Status "));
        frame.render_widget(status, chunks[3]);

        // NOTE: Modal is drawn automatically by the framework!
        // No need to call modal.draw() here.
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Handle quit first
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Check for modal results first
        // This is called even when modal consumes the event, allowing us to react to results
        if let Some((modal_id, result)) = ctx.modal().take_result() {
            self.handle_modal_result(&modal_id, result);
        }

        // If modal is open, it handles its own events - we don't need to forward
        // Just return handled to prevent other actions while modal is open
        if ctx.modal().is_open() {
            return EventResult::Handled;
        }

        // Handle keyboard shortcuts to open modals
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('1') => {
                    ctx.modal().open(Self::create_alert_modal());
                    self.status = "Alert modal opened".to_string();
                    return EventResult::Handled;
                }
                KeyCode::Char('2') => {
                    ctx.modal().open(Self::create_confirm_modal());
                    self.status = "Confirmation modal opened".to_string();
                    return EventResult::Handled;
                }
                KeyCode::Char('3') => {
                    ctx.modal().open(Self::create_save_modal());
                    self.status = "Save dialog opened".to_string();
                    return EventResult::Handled;
                }
                KeyCode::Char('4') => {
                    // Demonstrate rapid succession - opening a new modal while one is open
                    ctx.modal().open(Self::create_alert_modal());
                    self.status = "First: alert modal opened...".to_string();
                    // Immediately open another - the first will be closed with ModalResult::Closed
                    ctx.modal().open(Self::create_confirm_modal());
                    self.status =
                        "Rapid: alert was replaced by confirm (check next result)".to_string();
                    return EventResult::Handled;
                }
                KeyCode::Char('5') => {
                    ctx.modal().open(Self::create_prompt_modal());
                    self.status = "Prompt modal opened - enter your name!".to_string();
                    return EventResult::Handled;
                }
                _ => {}
            }
        }

        // Forward events to textbox when focused
        if ctx.focus().is_focused("input") {
            return self.textbox.handle_event(event);
        }

        EventResult::Unhandled
    }

    fn focus_id(&self) -> Option<&str> {
        Some("app")
    }

    fn focus_children(&self) -> Vec<&str> {
        vec!["input"]
    }
}

impl MainUi for ModalExample {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppBuilder::new()
        .main_ui(ModalExample::new())
        .tick_rate(Duration::from_millis(100)) // For cursor blinking
        .register_focus("input")
        .initial_focus("input")
        .build()?;

    app.run().await?;
    Ok(())
}
