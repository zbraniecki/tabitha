//! Modal dialog types - data structures for buttons, results, input, and configuration.
//!
//! This module contains the core types used by the modal system.

use ratatui::style::{Color, Style};

/// A single button in the modal dialog.
#[derive(Debug, Clone)]
pub struct ModalButton {
    /// Unique identifier for this button (returned in ModalResult).
    pub id: String,
    /// Display label for the button.
    pub label: String,
}

impl ModalButton {
    /// Create a new modal button with the given ID and label.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Result indicating how the modal was closed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModalResult {
    /// A button was pressed - contains the button's ID and optional input value.
    ButtonPressed {
        /// The ID of the button that was pressed.
        button_id: String,
        /// The input value if the modal had an input field.
        input: Option<String>,
    },
    /// Modal was dismissed via Escape key.
    Dismissed,
    /// Modal was closed programmatically via close().
    Closed,
    /// Modal is still open (no result yet).
    #[default]
    Pending,
}

impl ModalResult {
    /// Create a ButtonPressed result with just a button ID (no input).
    pub fn button_pressed(button_id: impl Into<String>) -> Self {
        ModalResult::ButtonPressed {
            button_id: button_id.into(),
            input: None,
        }
    }

    /// Create a ButtonPressed result with a button ID and input value.
    pub fn button_pressed_with_input(
        button_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        ModalResult::ButtonPressed {
            button_id: button_id.into(),
            input: Some(input.into()),
        }
    }

    /// Get the button ID if this is a ButtonPressed result.
    pub fn button_id(&self) -> Option<&str> {
        match self {
            ModalResult::ButtonPressed { button_id, .. } => Some(button_id),
            _ => None,
        }
    }

    /// Get the input value if this is a ButtonPressed result with input.
    pub fn input(&self) -> Option<&str> {
        match self {
            ModalResult::ButtonPressed { input, .. } => input.as_deref(),
            _ => None,
        }
    }
}

/// Configuration for an optional input field in the modal.
#[derive(Debug, Clone)]
pub struct ModalInput {
    /// Label displayed above the input field (or not shown if inline).
    pub label: String,
    /// Placeholder text shown when input is empty.
    pub placeholder: Option<String>,
    /// Current input value.
    pub value: String,
    /// Cursor position within the input.
    pub cursor: usize,
    /// If true, input is rendered inline with the message (on the same line).
    pub inline: bool,
}

/// Configuration for Modal appearance.
#[derive(Debug, Clone)]
pub struct ModalConfig {
    /// Width as percentage of screen (0-100).
    pub width_percent: u16,
    /// Height as percentage of screen (0-100).
    pub height_percent: u16,
    /// Style for the modal border.
    pub border_style: Style,
    /// Style for the backdrop (dimmed background).
    pub backdrop_style: Style,
    /// Style for the focused button.
    pub focused_button_style: Style,
    /// Style for unfocused buttons.
    pub unfocused_button_style: Style,
    /// Style for the message content.
    pub content_style: Style,
    /// Style for the title.
    pub title_style: Style,
    /// Style for the input field when focused.
    pub input_focused_style: Style,
    /// Style for the input field when not focused.
    pub input_unfocused_style: Style,
    /// Style for the input label.
    pub input_label_style: Style,
}

impl Default for ModalConfig {
    fn default() -> Self {
        Self {
            width_percent: 50,
            height_percent: 30,
            border_style: Style::default().fg(Color::White),
            backdrop_style: Style::default().bg(Color::Black),
            focused_button_style: Style::default().bg(Color::White).fg(Color::Black),
            unfocused_button_style: Style::default().fg(Color::White),
            content_style: Style::default().fg(Color::White),
            title_style: Style::default().fg(Color::Yellow),
            input_focused_style: Style::default().bg(Color::Blue).fg(Color::White),
            input_unfocused_style: Style::default().bg(Color::DarkGray).fg(Color::White),
            input_label_style: Style::default().fg(Color::Cyan),
        }
    }
}

impl ModalConfig {
    /// Set the modal size as percentages of the screen.
    pub fn with_size(mut self, width_percent: u16, height_percent: u16) -> Self {
        self.width_percent = width_percent.min(100);
        self.height_percent = height_percent.min(100);
        self
    }
}
