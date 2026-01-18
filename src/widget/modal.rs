//! Modal dialog component for the TUI framework.
//!
//! The Modal provides a blocking, centered dialog with:
//! - Configurable buttons with custom IDs and labels
//! - Backdrop dimming effect
//! - Keyboard navigation (Tab, Enter, Escape)
//! - Event blocking (prevents background interaction when open)
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{Modal, ModalButton, ModalResult};
//!
//! let mut modal = Modal::new("confirm", "Delete this item?")
//!     .with_title("Confirm Delete")
//!     .with_button(ModalButton::new("delete", "Delete"))
//!     .with_button(ModalButton::new("cancel", "Cancel"));
//!
//! // Open the modal
//! modal.open();
//!
//! // In handle_event, after calling modal.handle_event():
//! match modal.take_result() {
//!     ModalResult::ButtonPressed(id) if id == "delete" => {
//!         // User confirmed deletion
//!     }
//!     ModalResult::ButtonPressed(_) | ModalResult::Dismissed => {
//!         // Cancelled or dismissed with Escape
//!     }
//!     _ => {}
//! }
//! ```

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::component::Component;
use crate::context::{AppContext, DrawContext};
use crate::event::{Event, KeyCode, KeyModifiers};
use crate::focus::EventResult;

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

/// Which element is currently focused within the modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModalFocus {
    /// Focus is on the input field.
    Input,
    /// Focus is on the buttons.
    Buttons,
}

/// A modal dialog component.
///
/// Modal dialogs are blocking, centered dialogs that prevent interaction
/// with the background UI until dismissed. They support configurable
/// buttons with custom IDs and labels, and optionally an input field.
///
/// # Usage
///
/// 1. Create a modal with `Modal::new(id, message)`
/// 2. Optionally add an input field with `.with_input(label)`
/// 3. Add buttons with `.with_button()`
/// 4. Call `.open()` to show the modal
/// 5. In your event handler, call `modal.handle_event()` FIRST when open
/// 6. Check `modal.take_result()` to see if/how the modal was closed
///    - For modals with input, the result includes the input value
///
/// # Drawing Order
///
/// **Important**: The modal must be drawn LAST in your draw method to appear
/// on top of other UI elements.
pub struct Modal {
    /// Unique identifier for this modal.
    id: String,
    /// Optional title displayed in the border.
    title: Option<String>,
    /// Message displayed in the modal body.
    message: String,
    /// Optional input field.
    input: Option<ModalInput>,
    /// Buttons displayed at the bottom.
    buttons: Vec<ModalButton>,
    /// Visual configuration.
    config: ModalConfig,
    /// Whether the modal is currently open.
    open: bool,
    /// Result of the modal interaction.
    result: ModalResult,
    /// Which element is focused (input or buttons).
    focus: ModalFocus,
    /// Index of the currently focused button.
    focused_button: usize,
}

impl Modal {
    /// Create a new modal with the given ID and message.
    ///
    /// The modal starts closed. Call `.open()` to show it.
    pub fn new(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            message: message.into(),
            input: None,
            buttons: Vec::new(),
            config: ModalConfig::default(),
            open: false,
            result: ModalResult::Pending,
            focus: ModalFocus::Buttons,
            focused_button: 0,
        }
    }

    /// Set the modal title (displayed in the border).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Add an input field to the modal.
    ///
    /// The input field appears between the message and the buttons.
    /// When the modal is closed via a button press, the input value
    /// is included in the `ModalResult::ButtonPressed` result.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let modal = Modal::new("prompt", "Enter your name:")
    ///     .with_input("Name")
    ///     .with_button(ModalButton::new("ok", "OK"))
    ///     .with_button(ModalButton::new("cancel", "Cancel"));
    ///
    /// // When the user presses OK, the result includes the input:
    /// if let ModalResult::ButtonPressed { button_id, input } = result {
    ///     if button_id == "ok" {
    ///         if let Some(name) = input {
    ///             println!("User entered: {}", name);
    ///         }
    ///     }
    /// }
    /// ```
    pub fn with_input(mut self, label: impl Into<String>) -> Self {
        self.input = Some(ModalInput {
            label: label.into(),
            placeholder: None,
            value: String::new(),
            cursor: 0,
            inline: false,
        });
        self.focus = ModalFocus::Input; // Focus input by default when present
        self
    }

    /// Add an inline input field to the modal.
    ///
    /// The input field appears on the same line as the message, making the
    /// modal more compact. The label is not displayed (use the message instead).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let modal = Modal::new("prompt", "Please enter your name:")
    ///     .with_inline_input()
    ///     .with_button(ModalButton::new("ok", "OK"));
    /// ```
    pub fn with_inline_input(mut self) -> Self {
        self.input = Some(ModalInput {
            label: String::new(),
            placeholder: None,
            value: String::new(),
            cursor: 0,
            inline: true,
        });
        self.focus = ModalFocus::Input;
        self
    }

    /// Add an input field with a placeholder.
    pub fn with_input_placeholder(
        mut self,
        label: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        self.input = Some(ModalInput {
            label: label.into(),
            placeholder: Some(placeholder.into()),
            value: String::new(),
            cursor: 0,
            inline: false,
        });
        self.focus = ModalFocus::Input;
        self
    }

    /// Add an input field with a default value.
    pub fn with_input_value(
        mut self,
        label: impl Into<String>,
        default_value: impl Into<String>,
    ) -> Self {
        let value: String = default_value.into();
        let cursor = value.chars().count();
        self.input = Some(ModalInput {
            label: label.into(),
            placeholder: None,
            value,
            cursor,
            inline: false,
        });
        self.focus = ModalFocus::Input;
        self
    }

    /// Add a button to the modal.
    ///
    /// Buttons are displayed in the order they are added.
    /// The first button is focused by default when the modal opens.
    pub fn with_button(mut self, button: ModalButton) -> Self {
        self.buttons.push(button);
        self
    }

    /// Set the visual configuration.
    pub fn with_config(mut self, config: ModalConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the modal size as percentages of the screen.
    pub fn with_size(mut self, width_percent: u16, height_percent: u16) -> Self {
        self.config.width_percent = width_percent.min(100);
        self.config.height_percent = height_percent.min(100);
        self
    }

    /// Get the modal ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Check if the modal is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the modal.
    ///
    /// Resets the result to `Pending` and sets focus appropriately:
    /// - If there's an input field, focus starts on the input
    /// - Otherwise, focus starts on the first button
    pub fn open(&mut self) {
        self.open = true;
        self.result = ModalResult::Pending;
        self.focused_button = 0;
        // Focus input if present, otherwise buttons
        self.focus = if self.input.is_some() {
            ModalFocus::Input
        } else {
            ModalFocus::Buttons
        };
        // Reset input cursor to end if there's a value
        if let Some(ref mut input) = self.input {
            input.cursor = input.value.chars().count();
        }
    }

    /// Close the modal programmatically.
    ///
    /// Sets the result to `ModalResult::Closed`.
    pub fn close(&mut self) {
        self.open = false;
        self.result = ModalResult::Closed;
    }

    /// Get the current result without consuming it.
    pub fn result(&self) -> &ModalResult {
        &self.result
    }

    /// Take the result and reset it to `Pending`.
    ///
    /// This is the typical way to check the result after handling events.
    pub fn take_result(&mut self) -> ModalResult {
        std::mem::take(&mut self.result)
    }

    /// Get the message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Set the message text.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Get the title.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Set the title.
    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &ModalConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut ModalConfig {
        &mut self.config
    }

    // --- Internal helpers ---

    /// Check if the modal has an input field.
    fn has_input(&self) -> bool {
        self.input.is_some()
    }

    /// Cycle focus to the next element (input -> buttons -> input).
    fn focus_next(&mut self) {
        match self.focus {
            ModalFocus::Input => {
                self.focus = ModalFocus::Buttons;
                self.focused_button = 0;
            }
            ModalFocus::Buttons => {
                if self.has_input() {
                    // Cycle back to input
                    self.focus = ModalFocus::Input;
                } else {
                    // No input, cycle through buttons
                    self.focus_next_button();
                }
            }
        }
    }

    /// Cycle focus to the previous element.
    fn focus_prev(&mut self) {
        match self.focus {
            ModalFocus::Input => {
                // Move to last button
                self.focus = ModalFocus::Buttons;
                self.focused_button = self.buttons.len().saturating_sub(1);
            }
            ModalFocus::Buttons => {
                if self.has_input() && self.focused_button == 0 {
                    // At first button, go back to input
                    self.focus = ModalFocus::Input;
                } else {
                    // Stay in buttons, go to previous
                    self.focus_prev_button();
                }
            }
        }
    }

    /// Focus the next button (within buttons only).
    fn focus_next_button(&mut self) {
        if !self.buttons.is_empty() {
            self.focused_button = (self.focused_button + 1) % self.buttons.len();
        }
    }

    /// Focus the previous button (within buttons only).
    fn focus_prev_button(&mut self) {
        if !self.buttons.is_empty() {
            self.focused_button = if self.focused_button == 0 {
                self.buttons.len() - 1
            } else {
                self.focused_button - 1
            };
        }
    }

    /// Activate the currently focused button.
    fn activate_focused_button(&mut self) {
        if let Some(button) = self.buttons.get(self.focused_button) {
            let input_value = self.input.as_ref().map(|i| i.value.clone());
            self.result = ModalResult::ButtonPressed {
                button_id: button.id.clone(),
                input: input_value,
            };
            self.open = false;
        }
    }

    /// Handle a character input for the input field.
    fn handle_input_char(&mut self, c: char) {
        if let Some(ref mut input) = self.input {
            // Insert character at cursor position
            let char_idx = input
                .value
                .char_indices()
                .nth(input.cursor)
                .map(|(i, _)| i)
                .unwrap_or(input.value.len());
            input.value.insert(char_idx, c);
            input.cursor += 1;
        }
    }

    /// Handle backspace for the input field.
    fn handle_input_backspace(&mut self) {
        if let Some(ref mut input) = self.input {
            if input.cursor > 0 {
                let char_idx = input
                    .value
                    .char_indices()
                    .nth(input.cursor - 1)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let next_idx = input
                    .value
                    .char_indices()
                    .nth(input.cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(input.value.len());
                input.value.replace_range(char_idx..next_idx, "");
                input.cursor -= 1;
            }
        }
    }

    /// Handle delete for the input field.
    fn handle_input_delete(&mut self) {
        if let Some(ref mut input) = self.input {
            let char_count = input.value.chars().count();
            if input.cursor < char_count {
                let char_idx = input
                    .value
                    .char_indices()
                    .nth(input.cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(input.value.len());
                let next_idx = input
                    .value
                    .char_indices()
                    .nth(input.cursor + 1)
                    .map(|(i, _)| i)
                    .unwrap_or(input.value.len());
                input.value.replace_range(char_idx..next_idx, "");
            }
        }
    }

    /// Move input cursor left.
    fn move_input_cursor_left(&mut self) {
        if let Some(ref mut input) = self.input {
            if input.cursor > 0 {
                input.cursor -= 1;
            }
        }
    }

    /// Move input cursor right.
    fn move_input_cursor_right(&mut self) {
        if let Some(ref mut input) = self.input {
            let char_count = input.value.chars().count();
            if input.cursor < char_count {
                input.cursor += 1;
            }
        }
    }

    /// Move input cursor to start.
    fn move_input_cursor_home(&mut self) {
        if let Some(ref mut input) = self.input {
            input.cursor = 0;
        }
    }

    /// Move input cursor to end.
    fn move_input_cursor_end(&mut self) {
        if let Some(ref mut input) = self.input {
            input.cursor = input.value.chars().count();
        }
    }

    /// Calculate the modal rect centered in the given area.
    fn calculate_modal_rect(&self, area: Rect) -> Rect {
        let modal_width = (area.width as u32 * self.config.width_percent as u32 / 100) as u16;
        let modal_height = (area.height as u32 * self.config.height_percent as u32 / 100) as u16;

        // Ensure minimum size
        let modal_width = modal_width.max(20).min(area.width);
        let modal_height = modal_height.max(5).min(area.height);

        let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.y + (area.height.saturating_sub(modal_height)) / 2;

        Rect::new(x, y, modal_width, modal_height)
    }

    /// Draw the input field.
    fn draw_input(&self, frame: &mut Frame, area: Rect) {
        let Some(ref input) = self.input else {
            return;
        };

        let is_focused = self.focus == ModalFocus::Input;

        // Draw label on first line
        let label_line = Line::from(Span::styled(
            format!("{}:", input.label),
            self.config.input_label_style,
        ));
        let label_area = Rect::new(area.x, area.y, area.width, 1);
        frame.render_widget(Paragraph::new(label_line), label_area);

        // Draw input box on second line
        if area.height >= 2 {
            let input_area = Rect::new(area.x, area.y + 1, area.width, 1);
            let style = if is_focused {
                self.config.input_focused_style
            } else {
                self.config.input_unfocused_style
            };

            // Show placeholder if empty and not focused
            let display_text = if input.value.is_empty() && !is_focused {
                input.placeholder.as_deref().unwrap_or("")
            } else {
                &input.value
            };

            // Build the display with cursor
            let text = if is_focused {
                // Show cursor
                let before: String = display_text.chars().take(input.cursor).collect();
                let cursor_char = display_text.chars().nth(input.cursor).unwrap_or(' ');
                let after: String = display_text.chars().skip(input.cursor + 1).collect();

                Line::from(vec![
                    Span::styled(before, style),
                    Span::styled(
                        cursor_char.to_string(),
                        Style::default().bg(Color::White).fg(Color::Black),
                    ),
                    Span::styled(after, style),
                ])
            } else {
                Line::from(Span::styled(display_text, style))
            };

            // Fill background
            let bg_fill = " ".repeat(input_area.width as usize);
            frame.render_widget(Paragraph::new(bg_fill).style(style), input_area);
            frame.render_widget(Paragraph::new(text), input_area);
        }
    }

    /// Draw the message and inline input field on the same line.
    fn draw_inline_input(&self, frame: &mut Frame, area: Rect) {
        let Some(ref input) = self.input else {
            return;
        };

        let is_focused = self.focus == ModalFocus::Input;
        let input_style = if is_focused {
            self.config.input_focused_style
        } else {
            self.config.input_unfocused_style
        };

        // Calculate the message width
        let message_width = self.message.chars().count() as u16;
        let space_after_message = 1u16;
        let input_start = message_width + space_after_message;
        let input_width = area.width.saturating_sub(input_start).max(10);

        // Build the line with message + input
        let mut spans = vec![Span::styled(&self.message, self.config.content_style)];
        spans.push(Span::raw(" "));

        // Show placeholder if empty and not focused
        let display_text = if input.value.is_empty() && !is_focused {
            input.placeholder.as_deref().unwrap_or("")
        } else {
            &input.value
        };

        // Build the input portion with cursor
        if is_focused {
            // Show cursor
            let before: String = display_text.chars().take(input.cursor).collect();
            let cursor_char = display_text.chars().nth(input.cursor).unwrap_or(' ');
            let after: String = display_text.chars().skip(input.cursor + 1).collect();

            // Pad to fill input width
            let total_len = before.chars().count() + 1 + after.chars().count();
            let padding = " ".repeat((input_width as usize).saturating_sub(total_len));

            spans.push(Span::styled(before, input_style));
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            spans.push(Span::styled(format!("{}{}", after, padding), input_style));
        } else {
            // Pad to fill input width
            let padding =
                " ".repeat((input_width as usize).saturating_sub(display_text.chars().count()));
            spans.push(Span::styled(
                format!("{}{}", display_text, padding),
                input_style,
            ));
        }

        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), area);
    }

    /// Draw the buttons at the bottom of the modal.
    fn draw_buttons(&self, frame: &mut Frame, area: Rect) {
        if self.buttons.is_empty() {
            return;
        }

        let buttons_focused = self.focus == ModalFocus::Buttons;

        // Calculate total button width
        let button_spacing = 2;
        let button_padding = 2; // Space around label: [ Label ]
        let total_width: usize = self
            .buttons
            .iter()
            .map(|b| b.label.chars().count() + button_padding + 2) // +2 for brackets
            .sum::<usize>()
            + (self.buttons.len().saturating_sub(1)) * button_spacing;

        // Center buttons horizontally
        let start_x = area.x + (area.width.saturating_sub(total_width as u16)) / 2;

        let mut current_x = start_x;
        for (i, button) in self.buttons.iter().enumerate() {
            let is_focused = buttons_focused && i == self.focused_button;
            let style = if is_focused {
                self.config.focused_button_style
            } else {
                self.config.unfocused_button_style
            };

            let label = format!(" {} ", button.label);
            let button_width = label.chars().count() as u16 + 2; // +2 for [ ]

            // Draw button
            let button_area = Rect::new(current_x, area.y, button_width, 1);

            let text = if is_focused {
                format!("[{}]", label)
            } else {
                format!(" {} ", label)
            };

            let paragraph = Paragraph::new(text).style(style);
            frame.render_widget(paragraph, button_area);

            current_x += button_width + button_spacing as u16;
        }
    }
}

impl Component for Modal {
    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        if !self.open {
            return;
        }

        // 1. Draw backdrop (full screen dark overlay)
        let backdrop = Block::default().style(self.config.backdrop_style);
        frame.render_widget(backdrop, area);

        // 2. Calculate centered modal rect
        let modal_rect = self.calculate_modal_rect(area);

        // 3. Clear the modal area and draw the modal box
        frame.render_widget(Clear, modal_rect);

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.config.border_style);

        if let Some(ref title) = self.title {
            block = block.title(Span::styled(
                format!(" {} ", title),
                self.config.title_style,
            ));
        }

        let inner = block.inner(modal_rect);
        frame.render_widget(block, modal_rect);

        // 4. Layout: message (top) + optional input + buttons (bottom)
        // Calculate space needed:
        // - Input field: 3 lines (label + input + spacing) if present
        // - Buttons: 2 lines (spacing + buttons)
        let input_height: u16 = if self.input.is_some() { 3 } else { 0 };
        let button_lines: u16 = 2;
        let message_height = inner.height.saturating_sub(input_height + button_lines);

        let mut current_y = inner.y;

        // 5. Render message
        if message_height > 0 {
            let message_area = Rect::new(inner.x, current_y, inner.width, message_height);
            let message_lines: Vec<Line> = self
                .message
                .lines()
                .map(|line| Line::from(Span::styled(line, self.config.content_style)))
                .collect();
            let paragraph = Paragraph::new(message_lines);
            frame.render_widget(paragraph, message_area);
            current_y += message_height;
        }

        // 6. Render input field (if present)
        if self.input.is_some() && input_height > 0 {
            let input_area = Rect::new(inner.x, current_y, inner.width, input_height);
            self.draw_input(frame, input_area);
            current_y += input_height;
        }

        // 7. Render buttons
        if current_y < inner.y + inner.height {
            let button_area = Rect::new(inner.x, current_y, inner.width, 1);
            self.draw_buttons(frame, button_area);
        }
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        if !self.open {
            return EventResult::Unhandled;
        }

        // Modal intercepts ALL events when open
        if let Event::Key(key) = event {
            // Handle input-specific keys when input is focused
            if self.focus == ModalFocus::Input && self.input.is_some() {
                match key.code {
                    // Text input
                    KeyCode::Char(c) => {
                        self.handle_input_char(c);
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Backspace => {
                        self.handle_input_backspace();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Delete => {
                        self.handle_input_delete();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Left => {
                        self.move_input_cursor_left();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Right => {
                        self.move_input_cursor_right();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Home => {
                        self.move_input_cursor_home();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::End => {
                        self.move_input_cursor_end();
                        return EventResult::StopPropagation;
                    }
                    // Tab moves to buttons
                    KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.focus_next();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        self.focus_prev();
                        return EventResult::StopPropagation;
                    }
                    KeyCode::BackTab => {
                        self.focus_prev();
                        return EventResult::StopPropagation;
                    }
                    // Enter in input submits the first button
                    KeyCode::Enter => {
                        self.focus = ModalFocus::Buttons;
                        self.focused_button = 0;
                        self.activate_focused_button();
                        return EventResult::StopPropagation;
                    }
                    // Escape dismisses
                    KeyCode::Esc => {
                        self.result = ModalResult::Dismissed;
                        self.open = false;
                        return EventResult::StopPropagation;
                    }
                    _ => {}
                }
            }

            // Handle button navigation (when buttons are focused or no input)
            match key.code {
                // Navigation: Tab -> next element
                KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.focus_next();
                }
                // Navigation within buttons: Right
                KeyCode::Right if self.focus == ModalFocus::Buttons => {
                    self.focus_next_button();
                }

                // Navigation: Shift+Tab -> previous element
                KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.focus_prev();
                }
                KeyCode::BackTab => {
                    self.focus_prev();
                }
                // Navigation within buttons: Left
                KeyCode::Left if self.focus == ModalFocus::Buttons => {
                    self.focus_prev_button();
                }

                // Activate: Enter or Space (when buttons focused)
                KeyCode::Enter | KeyCode::Char(' ') if self.focus == ModalFocus::Buttons => {
                    self.activate_focused_button();
                }

                // Dismiss: Escape
                KeyCode::Esc => {
                    self.result = ModalResult::Dismissed;
                    self.open = false;
                }

                _ => {}
            }
        }

        // Always stop propagation when modal is open
        EventResult::StopPropagation
    }

    fn focus_id(&self) -> Option<&str> {
        if self.open {
            Some(&self.id)
        } else {
            None
        }
    }

    fn is_focusable(&self) -> bool {
        self.open
    }

    fn focus_children(&self) -> Vec<&str> {
        // Modal manages its own button focus internally
        // We don't expose button IDs to the focus manager
        vec![]
    }
}

// =============================================================================
// ModalManager - Centralized modal management
// =============================================================================

/// Centralized manager for modal dialogs.
///
/// The `ModalManager` ensures only one modal can be open at a time and handles
/// the modal lifecycle automatically. When you open a new modal while another
/// is open, the existing modal is closed (with `ModalResult::Closed`) and
/// replaced by the new one.
///
/// # Usage
///
/// The `ModalManager` is integrated into the `App` and accessible via `AppContext`:
///
/// ```ignore
/// // In your component's handle_event:
/// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///     // Check for modal results first
///     if let Some((modal_id, result)) = ctx.modal().take_result() {
///         match (modal_id.as_str(), result) {
///             ("confirm", ModalResult::ButtonPressed(id)) if id == "delete" => {
///                 self.delete_item();
///             }
///             _ => {}
///         }
///     }
///
///     // Open a modal when needed
///     if event.is_key(KeyCode::Char('d')) {
///         ctx.modal().open(
///             Modal::new("confirm", "Delete this item?")
///                 .with_button(ModalButton::new("delete", "Delete"))
///                 .with_button(ModalButton::new("cancel", "Cancel"))
///         );
///         return EventResult::Handled;
///     }
///
///     EventResult::Unhandled
/// }
/// ```
///
/// # Event and Drawing
///
/// The `App` automatically:
/// - Handles modal events before dispatching to your UI (when a modal is open)
/// - Draws the modal on top of your UI
///
/// You don't need to manually handle events or draw the modal.
pub struct ModalManager {
    /// The currently open modal, if any.
    current: Option<Modal>,
    /// ID of the last closed modal (for result matching).
    last_id: Option<String>,
    /// Result from the last closed modal.
    last_result: Option<ModalResult>,
}

impl ModalManager {
    /// Create a new empty modal manager.
    pub fn new() -> Self {
        Self {
            current: None,
            last_id: None,
            last_result: None,
        }
    }

    /// Open a modal dialog.
    ///
    /// If another modal is currently open, it will be closed first with
    /// `ModalResult::Closed`. The new modal is opened immediately.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.modal().open(
    ///     Modal::new("confirm", "Are you sure?")
    ///         .with_title("Confirm")
    ///         .with_button(ModalButton::new("yes", "Yes"))
    ///         .with_button(ModalButton::new("no", "No"))
    /// );
    /// ```
    pub fn open(&mut self, mut modal: Modal) {
        // Close any existing modal first
        if let Some(existing) = self.current.take() {
            self.last_id = Some(existing.id.clone());
            self.last_result = Some(ModalResult::Closed);
        }

        // Open the new modal
        modal.open();
        self.current = Some(modal);
    }

    /// Close the current modal programmatically.
    ///
    /// Sets the result to `ModalResult::Closed`. Does nothing if no modal is open.
    pub fn close(&mut self) {
        if let Some(mut modal) = self.current.take() {
            modal.close();
            self.last_id = Some(modal.id.clone());
            self.last_result = Some(modal.result);
        }
    }

    /// Check if a modal is currently open.
    pub fn is_open(&self) -> bool {
        self.current.is_some()
    }

    /// Get the ID of the currently open modal.
    pub fn current_id(&self) -> Option<&str> {
        self.current.as_ref().map(|m| m.id.as_str())
    }

    /// Take the result from the last closed modal.
    ///
    /// Returns `Some((modal_id, result))` if a modal was recently closed,
    /// `None` otherwise. The result is cleared after being taken.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some((modal_id, result)) = ctx.modal().take_result() {
    ///     match (modal_id.as_str(), result) {
    ///         ("confirm", ModalResult::ButtonPressed(id)) if id == "yes" => {
    ///             // User confirmed
    ///         }
    ///         ("confirm", ModalResult::Dismissed) => {
    ///             // User pressed Escape
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub fn take_result(&mut self) -> Option<(String, ModalResult)> {
        match (self.last_id.take(), self.last_result.take()) {
            (Some(id), Some(result)) => Some((id, result)),
            _ => None,
        }
    }

    /// Peek at the result without consuming it.
    pub fn result(&self) -> Option<(&str, &ModalResult)> {
        match (&self.last_id, &self.last_result) {
            (Some(id), Some(result)) => Some((id.as_str(), result)),
            _ => None,
        }
    }

    /// Get the ID of the last closed modal.
    pub fn last_id(&self) -> Option<&str> {
        self.last_id.as_deref()
    }

    // --- Internal methods used by App ---

    /// Handle an event when a modal is open.
    ///
    /// This is called by the App's event loop. Returns `true` if the modal
    /// consumed the event (i.e., a modal is open).
    pub(crate) fn handle_event(&mut self, event: &Event) -> bool {
        let Some(modal) = self.current.as_mut() else {
            return false;
        };

        // Handle the event
        if let Event::Key(key) = event {
            // Handle input-specific keys when input is focused
            if modal.focus == ModalFocus::Input && modal.input.is_some() {
                match key.code {
                    // Text input
                    KeyCode::Char(c) => {
                        modal.handle_input_char(c);
                    }
                    KeyCode::Backspace => {
                        modal.handle_input_backspace();
                    }
                    KeyCode::Delete => {
                        modal.handle_input_delete();
                    }
                    KeyCode::Left => {
                        modal.move_input_cursor_left();
                    }
                    KeyCode::Right => {
                        modal.move_input_cursor_right();
                    }
                    KeyCode::Home => {
                        modal.move_input_cursor_home();
                    }
                    KeyCode::End => {
                        modal.move_input_cursor_end();
                    }
                    // Tab moves to buttons
                    KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                        modal.focus_next();
                    }
                    KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        modal.focus_prev();
                    }
                    KeyCode::BackTab => {
                        modal.focus_prev();
                    }
                    // Enter in input submits the first button
                    KeyCode::Enter => {
                        modal.focus = ModalFocus::Buttons;
                        modal.focused_button = 0;
                        modal.activate_focused_button();
                    }
                    // Escape dismisses
                    KeyCode::Esc => {
                        modal.result = ModalResult::Dismissed;
                        modal.open = false;
                    }
                    _ => {}
                }
            } else {
                // Handle button navigation
                match key.code {
                    // Navigation: Tab -> next element
                    KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                        modal.focus_next();
                    }
                    // Navigation within buttons: Right
                    KeyCode::Right if modal.focus == ModalFocus::Buttons => {
                        modal.focus_next_button();
                    }

                    // Navigation: Shift+Tab -> previous element
                    KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        modal.focus_prev();
                    }
                    KeyCode::BackTab => {
                        modal.focus_prev();
                    }
                    // Navigation within buttons: Left
                    KeyCode::Left if modal.focus == ModalFocus::Buttons => {
                        modal.focus_prev_button();
                    }

                    // Activate: Enter or Space (when buttons focused)
                    KeyCode::Enter | KeyCode::Char(' ') if modal.focus == ModalFocus::Buttons => {
                        modal.activate_focused_button();
                    }

                    // Dismiss: Escape
                    KeyCode::Esc => {
                        modal.result = ModalResult::Dismissed;
                        modal.open = false;
                    }

                    _ => {}
                }
            }
        }

        // Check if modal closed itself
        if !modal.open {
            let closed_modal = self.current.take().unwrap();
            self.last_id = Some(closed_modal.id);
            self.last_result = Some(closed_modal.result);
        }

        true // Modal consumed the event
    }

    /// Draw the current modal (if any) on top of the given area.
    ///
    /// This is called by the App's draw method after drawing the main UI.
    pub(crate) fn draw(&self, frame: &mut Frame, area: Rect) {
        if let Some(modal) = &self.current {
            // 1. Draw backdrop (full screen dark overlay)
            let backdrop = Block::default().style(modal.config.backdrop_style);
            frame.render_widget(backdrop, area);

            // 2. Calculate centered modal rect
            let modal_rect = modal.calculate_modal_rect(area);

            // 3. Clear the modal area and draw the modal box
            frame.render_widget(Clear, modal_rect);

            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(modal.config.border_style);

            if let Some(ref title) = modal.title {
                block = block.title(Span::styled(
                    format!(" {} ", title),
                    modal.config.title_style,
                ));
            }

            let inner = block.inner(modal_rect);
            frame.render_widget(block, modal_rect);

            // 4. Layout: message (top) + optional input + buttons (bottom)
            let is_inline = modal.input.as_ref().map(|i| i.inline).unwrap_or(false);
            let input_height: u16 = if modal.input.is_some() && !is_inline {
                3
            } else {
                0
            };
            let button_lines: u16 = 2;
            let message_height = inner.height.saturating_sub(input_height + button_lines);

            let mut current_y = inner.y;

            // 5. Render message (and inline input if applicable)
            if message_height > 0 {
                let message_area = Rect::new(inner.x, current_y, inner.width, message_height);

                if is_inline {
                    // Render message and input on the same line
                    modal.draw_inline_input(frame, message_area);
                } else {
                    let message_lines: Vec<Line> = modal
                        .message
                        .lines()
                        .map(|line| Line::from(Span::styled(line, modal.config.content_style)))
                        .collect();
                    let paragraph = Paragraph::new(message_lines);
                    frame.render_widget(paragraph, message_area);
                }
                current_y += message_height;
            }

            // 6. Render input field (if present and not inline)
            if modal.input.is_some() && input_height > 0 && !is_inline {
                let input_area = Rect::new(inner.x, current_y, inner.width, input_height);
                modal.draw_input(frame, input_area);
                current_y += input_height;
            }

            // 7. Render buttons
            if current_y < inner.y + inner.height {
                let button_area = Rect::new(inner.x, current_y, inner.width, 1);
                modal.draw_buttons(frame, button_area);
            }
        }
    }
}

impl Default for ModalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_creation() {
        let modal = Modal::new("test", "Test message")
            .with_title("Test Title")
            .with_button(ModalButton::new("ok", "OK"));

        assert_eq!(modal.id(), "test");
        assert_eq!(modal.message(), "Test message");
        assert_eq!(modal.title(), Some("Test Title"));
        assert!(!modal.is_open());
    }

    #[test]
    fn test_modal_open_close() {
        let mut modal = Modal::new("test", "Message").with_button(ModalButton::new("ok", "OK"));

        assert!(!modal.is_open());

        modal.open();
        assert!(modal.is_open());
        assert_eq!(modal.result(), &ModalResult::Pending);

        modal.close();
        assert!(!modal.is_open());
        assert_eq!(modal.result(), &ModalResult::Closed);
    }

    #[test]
    fn test_modal_take_result() {
        let mut modal = Modal::new("test", "Message").with_button(ModalButton::new("ok", "OK"));

        modal.open();
        modal.close();

        let result = modal.take_result();
        assert_eq!(result, ModalResult::Closed);

        // After take, should be Pending
        assert_eq!(modal.result(), &ModalResult::Pending);
    }

    #[test]
    fn test_modal_button_navigation() {
        let mut modal = Modal::new("test", "Message")
            .with_button(ModalButton::new("a", "A"))
            .with_button(ModalButton::new("b", "B"))
            .with_button(ModalButton::new("c", "C"));

        modal.open();
        assert_eq!(modal.focused_button, 0);

        modal.focus_next_button();
        assert_eq!(modal.focused_button, 1);

        modal.focus_next_button();
        assert_eq!(modal.focused_button, 2);

        // Wrap around
        modal.focus_next_button();
        assert_eq!(modal.focused_button, 0);

        // Go back
        modal.focus_prev_button();
        assert_eq!(modal.focused_button, 2);
    }

    #[test]
    fn test_modal_activate_button() {
        let mut modal = Modal::new("test", "Message")
            .with_button(ModalButton::new("delete", "Delete"))
            .with_button(ModalButton::new("cancel", "Cancel"));

        modal.open();
        modal.focus = ModalFocus::Buttons;
        modal.focus_next_button(); // Focus "Cancel"
        modal.activate_focused_button();

        assert!(!modal.is_open());
        assert_eq!(
            modal.result(),
            &ModalResult::ButtonPressed {
                button_id: "cancel".to_string(),
                input: None,
            }
        );
    }

    #[test]
    fn test_modal_result_default() {
        assert_eq!(ModalResult::default(), ModalResult::Pending);
    }

    #[test]
    fn test_modal_config() {
        let config = ModalConfig::default().with_size(60, 40);
        assert_eq!(config.width_percent, 60);
        assert_eq!(config.height_percent, 40);

        // Test clamping
        let config = ModalConfig::default().with_size(150, 200);
        assert_eq!(config.width_percent, 100);
        assert_eq!(config.height_percent, 100);
    }

    #[test]
    fn test_calculate_modal_rect() {
        let modal = Modal::new("test", "Message").with_size(50, 50);

        let area = Rect::new(0, 0, 100, 40);
        let rect = modal.calculate_modal_rect(area);

        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 20);
        assert_eq!(rect.x, 25); // Centered
        assert_eq!(rect.y, 10); // Centered
    }

    // --- ModalManager tests ---

    #[test]
    fn test_modal_manager_new() {
        let mut manager = ModalManager::new();
        assert!(!manager.is_open());
        assert!(manager.current_id().is_none());
        assert!(manager.take_result().is_none());
    }

    #[test]
    fn test_modal_manager_open() {
        let mut manager = ModalManager::new();

        let modal = Modal::new("test", "Test message").with_button(ModalButton::new("ok", "OK"));

        manager.open(modal);

        assert!(manager.is_open());
        assert_eq!(manager.current_id(), Some("test"));
    }

    #[test]
    fn test_modal_manager_close() {
        let mut manager = ModalManager::new();

        let modal = Modal::new("test", "Test message").with_button(ModalButton::new("ok", "OK"));

        manager.open(modal);
        assert!(manager.is_open());

        manager.close();
        assert!(!manager.is_open());

        // Should have a result now
        let result = manager.take_result();
        assert!(result.is_some());
        let (id, res) = result.unwrap();
        assert_eq!(id, "test");
        assert_eq!(res, ModalResult::Closed);
    }

    #[test]
    fn test_modal_manager_replace_modal() {
        let mut manager = ModalManager::new();

        // Open first modal
        let modal1 = Modal::new("modal1", "First modal").with_button(ModalButton::new("ok", "OK"));
        manager.open(modal1);
        assert_eq!(manager.current_id(), Some("modal1"));

        // Open second modal - should close first
        let modal2 = Modal::new("modal2", "Second modal").with_button(ModalButton::new("ok", "OK"));
        manager.open(modal2);

        // Current should be modal2
        assert_eq!(manager.current_id(), Some("modal2"));

        // First modal should have been closed
        let result = manager.take_result();
        assert!(result.is_some());
        let (id, res) = result.unwrap();
        assert_eq!(id, "modal1");
        assert_eq!(res, ModalResult::Closed);
    }

    #[test]
    fn test_modal_manager_result_after_take() {
        let mut manager = ModalManager::new();

        let modal = Modal::new("test", "Message").with_button(ModalButton::new("ok", "OK"));
        manager.open(modal);
        manager.close();

        // First take should return result
        assert!(manager.take_result().is_some());

        // Second take should return None
        assert!(manager.take_result().is_none());
    }

    #[test]
    fn test_modal_manager_default() {
        let manager = ModalManager::default();
        assert!(!manager.is_open());
    }
}
