//! Modal dialog component implementation.
//!
//! This module contains the core Modal struct and its implementation.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::component::Component;
use crate::context::{AppContext, DrawContext};
use crate::event::Event;
use crate::focus::EventResult;
use crate::theme::Theme;

use super::focus::{event_handlers::*, ModalFocus};

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
    pub(crate) id: String,
    /// Optional title displayed in the border.
    pub(crate) title: Option<String>,
    /// Message displayed in the modal body.
    pub(crate) message: String,
    /// Optional input field.
    pub(crate) input: Option<ModalInput>,
    /// Buttons displayed at the bottom.
    pub(crate) buttons: Vec<ModalButton>,
    /// Visual configuration.
    pub(crate) config: ModalConfig,
    /// Whether the modal is currently open.
    pub(crate) open: bool,
    /// Result of the modal interaction.
    pub(crate) result: ModalResult,
    /// Which element is focused (input or buttons).
    pub(crate) focus: ModalFocus,
    /// Index of the currently focused button.
    pub(crate) focused_button: usize,
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

    /// Alias for `with_inline_input()` for API consistency.
    pub fn with_input_inline(self) -> Self {
        self.with_inline_input()
    }

    /// Add a button to the modal.
    ///
    /// Buttons are displayed in the order they are added.
    /// The first button is focused by default when the modal opens.
    pub fn with_button(mut self, button: ModalButton) -> Self {
        self.buttons.push(button);
        self
    }

    /// Add multiple buttons to the modal.
    pub fn with_buttons(mut self, buttons: impl IntoIterator<Item = ModalButton>) -> Self {
        self.buttons.extend(buttons);
        self
    }

    /// Clear all buttons from the modal.
    pub fn clear_buttons(&mut self) {
        self.buttons.clear();
        self.focused_button = 0;
    }

    /// Get the modal ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Set the message text.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Get the buttons.
    pub fn buttons(&self) -> &[ModalButton] {
        &self.buttons
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

    /// Take the result and reset it to `Pending`.
    ///
    /// This is the typical way to check the result after handling events.
    pub fn take_result(&mut self) -> ModalResult {
        std::mem::take(&mut self.result)
    }

    /// Get the current result without consuming it.
    pub fn result(&self) -> &ModalResult {
        &self.result
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

    /// Take the current input value, clearing it from the modal.
    pub fn take_input_value(&mut self) -> Option<String> {
        self.input.as_mut().map(|i| {
            let value = std::mem::take(&mut i.value);
            i.cursor = 0;
            value
        })
    }

    /// Get the current input value without consuming it.
    pub fn input_value(&self) -> Option<&str> {
        self.input.as_ref().map(|i| i.value.as_str())
    }

    /// Set the input value.
    pub fn set_input_value(&mut self, value: impl Into<String>) {
        if let Some(ref mut input) = self.input {
            let value: String = value.into();
            input.cursor = value.chars().count();
            input.value = value;
        }
    }

    /// Check if the modal has an input field.
    fn has_input(&self) -> bool {
        self.input.is_some()
    }

    /// Calculate the modal rect centered in the given area.
    pub fn calculate_modal_rect(&self, area: Rect) -> Rect {
        let modal_width = (area.width as u32 * self.config.width_percent as u32 / 100) as u16;
        let modal_height = (area.height as u32 * self.config.height_percent as u32 / 100) as u16;

        // Ensure minimum size
        let modal_width = modal_width.max(20).min(area.width);
        let modal_height = modal_height.max(5).min(area.height);

        let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let y = area.y + (area.height.saturating_sub(modal_height)) / 2;

        Rect::new(x, y, modal_width, modal_height)
    }

    /// Cycle focus to the next element (input -> buttons -> input).
    pub fn focus_next(&mut self) {
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
    pub fn focus_prev(&mut self) {
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
    pub fn focus_next_button(&mut self) {
        if !self.buttons.is_empty() {
            self.focused_button = (self.focused_button + 1) % self.buttons.len();
        }
    }

    /// Focus the previous button (within buttons only).
    pub fn focus_prev_button(&mut self) {
        if !self.buttons.is_empty() {
            self.focused_button = if self.focused_button == 0 {
                self.buttons.len() - 1
            } else {
                self.focused_button - 1
            };
        }
    }

    /// Activate the currently focused button.
    pub fn activate_focused_button(&mut self) {
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
    pub fn handle_input_char(&mut self, c: char) {
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
    pub fn handle_input_backspace(&mut self) {
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
    pub fn handle_input_delete(&mut self) {
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
    pub fn move_input_cursor_left(&mut self) {
        if let Some(ref mut input) = self.input {
            if input.cursor > 0 {
                input.cursor -= 1;
            }
        }
    }

    /// Move input cursor right.
    pub fn move_input_cursor_right(&mut self) {
        if let Some(ref mut input) = self.input {
            let char_count = input.value.chars().count();
            if input.cursor < char_count {
                input.cursor += 1;
            }
        }
    }

    /// Move input cursor to start.
    pub fn move_input_cursor_home(&mut self) {
        if let Some(ref mut input) = self.input {
            input.cursor = 0;
        }
    }

    /// Move input cursor to end.
    pub fn move_input_cursor_end(&mut self) {
        if let Some(ref mut input) = self.input {
            input.cursor = input.value.chars().count();
        }
    }

    /// Draw the buttons at the bottom of the modal.
    pub fn draw_buttons(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
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
                theme.highlight_style()
            } else {
                theme.fg_style()
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

    /// Draw the input field.
    pub fn draw_input(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(ref input) = self.input else {
            return;
        };

        let is_focused = self.focus == ModalFocus::Input;

        // Draw label on first line
        let label_style = theme.secondary_style();
        let label_line = Line::from(Span::styled(format!("{}:", input.label), label_style));
        let label_area = Rect::new(area.x, area.y, area.width, 1);
        frame.render_widget(Paragraph::new(label_line), label_area);

        // Draw input box on second line
        if area.height >= 2 {
            let input_area = Rect::new(area.x, area.y + 1, area.width, 1);
            // For input fields, use foreground colors with a subtle background for distinction
            let style = if is_focused {
                // Focused: accent foreground with dark background
                Style::default().fg(theme.accent).bg(theme.background)
            } else {
                // Unfocused: muted foreground with dark background
                Style::default()
                    .fg(theme.muted_foreground)
                    .bg(theme.background)
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
    fn draw_inline_input(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let Some(ref input) = self.input else {
            return;
        };

        let is_focused = self.focus == ModalFocus::Input;
        // For inline input, use a subtle background to make it stand out
        let input_style = if is_focused {
            // Focused: accent foreground on secondary background
            Style::default().fg(theme.accent).bg(theme.secondary)
        } else {
            // Unfocused: muted foreground on muted background
            Style::default().fg(theme.muted_foreground).bg(theme.muted)
        };

        // Calculate the message width
        let message_width = self.message.chars().count() as u16;
        let space_after_message = 1u16;
        let input_start = message_width + space_after_message;
        let input_width = area.width.saturating_sub(input_start).max(10);

        // Build the line with message + input
        let content_style = theme.fg_style();
        let mut spans = vec![Span::styled(&self.message, content_style)];
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
}

impl Component for Modal {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if !self.open {
            return;
        }

        let theme = ctx.theme();

        // 1. Draw backdrop (full screen dark overlay)
        let backdrop = Block::default().style(self.config.backdrop_style);
        frame.render_widget(backdrop, area);

        // 2. Calculate centered modal rect
        let modal_rect = self.calculate_modal_rect(area);

        // 3. Clear the modal area and draw the modal box
        frame.render_widget(Clear, modal_rect);

        // Use theme accent color for border
        let border_style = theme.border_focused_style();
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        if let Some(ref title) = self.title {
            // Use theme accent foreground color for title (no background)
            let title_style = Style::default().fg(theme.accent);
            block = block.title(Span::styled(format!(" {} ", title), title_style));
        }

        let inner = block.inner(modal_rect);
        frame.render_widget(block, modal_rect);

        // 4. Layout: message (top) + optional input + buttons (bottom)
        // Calculate space needed:
        // - Input field: 3 lines (label + input + spacing) if present
        // - Buttons: 2 lines (spacing + buttons)
        let is_inline = self.input.as_ref().map(|i| i.inline).unwrap_or(false);
        let input_height: u16 = if self.input.is_some() && !is_inline {
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
                self.draw_inline_input(frame, message_area, theme);
            } else {
                let content_style = theme.fg_style();
                let message_lines: Vec<Line> = self
                    .message
                    .lines()
                    .map(|line| Line::from(Span::styled(line, content_style)))
                    .collect();
                let paragraph = Paragraph::new(message_lines);
                frame.render_widget(paragraph, message_area);
            }
            current_y += message_height;
        }

        // 6. Render input field (if present and not inline)
        if self.input.is_some() && input_height > 0 && !is_inline {
            let input_area = Rect::new(inner.x, current_y, inner.width, input_height);
            self.draw_input(frame, input_area, theme);
            current_y += input_height;
        }

        // 7. Render buttons
        if current_y < inner.y + inner.height {
            let button_area = Rect::new(inner.x, current_y, inner.width, 1);
            self.draw_buttons(frame, button_area, theme);
        }
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        if !self.open {
            return EventResult::Unhandled;
        }

        // Use shared event handlers for deduplicated logic
        use super::focus::event_handlers::*;

        // Handle the event
        if let Event::Key(_) = event {
            let has_input = self.has_input();

            // Handle input-specific keys when input is focused
            if self.focus == ModalFocus::Input
                && has_input
                && handle_input_keys(event, self).is_some()
            {
                return EventResult::StopPropagation;
            }

            // Handle navigation keys (works in both input and button contexts)
            if handle_navigation_keys(event, self, has_input) {
                return EventResult::StopPropagation;
            }

            // Handle button activation (only when buttons are focused)
            if self.focus == ModalFocus::Buttons {
                if let Some(_action) = handle_button_keys(event, self) {
                    return EventResult::StopPropagation;
                }
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
// Trait implementations for shared event handlers
// =============================================================================

impl ModalNavigation for Modal {
    fn focus_next(&mut self) {
        self.focus_next();
    }

    fn focus_prev(&mut self) {
        self.focus_prev();
    }

    fn focus_next_button(&mut self) {
        self.focus_next_button();
    }

    fn focus_prev_button(&mut self) {
        self.focus_prev_button();
    }

    fn is_buttons_focused(&self) -> bool {
        self.focus == ModalFocus::Buttons
    }
}

impl ModalInputHandlers for Modal {
    fn handle_input_char(&mut self, c: char) {
        self.handle_input_char(c);
    }

    fn handle_input_backspace(&mut self) {
        self.handle_input_backspace();
    }

    fn handle_input_delete(&mut self) {
        self.handle_input_delete();
    }

    fn move_input_cursor_left(&mut self) {
        self.move_input_cursor_left();
    }

    fn move_input_cursor_right(&mut self) {
        self.move_input_cursor_right();
    }

    fn move_input_cursor_home(&mut self) {
        self.move_input_cursor_home();
    }

    fn move_input_cursor_end(&mut self) {
        self.move_input_cursor_end();
    }

    fn move_focus_to_buttons(&mut self) {
        self.focus = ModalFocus::Buttons;
        self.focused_button = 0;
    }

    fn activate_first_button(&mut self) {
        self.activate_focused_button();
    }

    fn dismiss(&mut self) {
        self.result = ModalResult::Dismissed;
        self.open = false;
    }
}

impl ModalButtonHandlers for Modal {
    fn activate_focused_button(&mut self) {
        self.activate_focused_button();
    }

    fn dismiss(&mut self) {
        self.result = ModalResult::Dismissed;
        self.open = false;
    }
}
