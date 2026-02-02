//! Modal rendering helper functions.
//!
//! This module contains pure rendering functions for modal dialogs.
//! All functions are `pub(super)` and only accessible within the modal module.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme::Theme;

use super::types::{ModalButton, ModalConfig, ModalInput};

/// Calculate the modal rect centered in the given area.
pub(super) fn calculate_modal_rect(area: Rect, config: &ModalConfig) -> Rect {
    let modal_width = (area.width as u32 * config.width_percent as u32 / 100) as u16;
    let modal_height = (area.height as u32 * config.height_percent as u32 / 100) as u16;

    // Ensure minimum size
    let modal_width = modal_width.max(20).min(area.width);
    let modal_height = modal_height.max(5).min(area.height);

    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;

    Rect::new(x, y, modal_width, modal_height)
}

/// Draw the backdrop (full screen dark overlay).
pub(super) fn draw_backdrop(frame: &mut Frame, area: Rect, config: &ModalConfig) {
    let backdrop = ratatui::widgets::Block::default().style(config.backdrop_style);
    frame.render_widget(backdrop, area);
}

/// Draw the modal box with border and optional title.
pub(super) fn draw_modal_box(
    frame: &mut Frame,
    rect: Rect,
    title: Option<&str>,
    theme: &Theme,
) -> Rect {
    // Clear the modal area
    frame.render_widget(Clear, rect);

    // Use theme accent color for border
    let border_style = theme.border_focused_style();
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    if let Some(title_text) = title {
        // Use theme accent foreground color for title (no background)
        let title_style = Style::default().fg(theme.accent);
        block = block.title(Span::styled(format!(" {} ", title_text), title_style));
    }

    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    inner
}

/// Draw the buttons at the bottom of the modal.
pub(super) fn draw_buttons(
    frame: &mut Frame,
    area: Rect,
    buttons: &[ModalButton],
    focused_idx: usize,
    buttons_focused: bool,
    theme: &Theme,
) {
    if buttons.is_empty() {
        return;
    }

    // Calculate total button width
    let button_spacing = 2;
    let button_padding = 2; // Space around label: [ Label ]
    let total_width: usize = buttons
        .iter()
        .map(|b| b.label.chars().count() + button_padding + 2) // +2 for brackets
        .sum::<usize>()
        + (buttons.len().saturating_sub(1)) * button_spacing;

    // Center buttons horizontally
    let start_x = area.x + (area.width.saturating_sub(total_width as u16)) / 2;

    let mut current_x = start_x;
    for (i, button) in buttons.iter().enumerate() {
        let is_focused = buttons_focused && i == focused_idx;
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
pub(super) fn draw_input(
    frame: &mut Frame,
    area: Rect,
    input: &ModalInput,
    is_focused: bool,
    theme: &Theme,
) {
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
pub(super) fn draw_inline_input(
    frame: &mut Frame,
    area: Rect,
    message: &str,
    input: &ModalInput,
    is_focused: bool,
    theme: &Theme,
) {
    // For inline input, use a subtle background to make it stand out
    let input_style = if is_focused {
        // Focused: accent foreground on secondary background
        Style::default().fg(theme.accent).bg(theme.secondary)
    } else {
        // Unfocused: muted foreground on muted background
        Style::default().fg(theme.muted_foreground).bg(theme.muted)
    };

    // Calculate the message width
    let message_width = message.chars().count() as u16;
    let space_after_message = 1u16;
    let input_start = message_width + space_after_message;
    let input_width = area.width.saturating_sub(input_start).max(10);

    // Build the line with message + input
    let content_style = theme.fg_style();
    let mut spans = vec![Span::styled(message, content_style)];
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
