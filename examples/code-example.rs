//! OpenCode-inspired TUI example for tabitha.
//!
//! This example demonstrates OpenTUI-like patterns:
//! - Top bar with context metrics
//! - Main content area with sidebar (right side)
//! - TODO list with checkboxes and active item highlighting
//! - Bottom prompt area with mode, model, provider
//!
//! Controls:
//! - Tab: Switch focus (main → sidebar → prompt)
//! - Alt+/;: Toggle sidebar visibility
//! - j/k or ↑/↓: Navigate TODO items
//! - Space: Toggle checkbox
//! - ↑/↓: Scroll main content
//! - q/Ctrl+C: Quit

#[path = "_common/mod.rs"]
mod common;
use clap::Parser;
use common::Args;

use std::time::Duration;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use tabitha::{
    widget::{
        Control, CursorAnimationMode, CursorFadeConfig, TextBox, TextBoxConfig, TextBoxEvent,
    },
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, KeyCode,
    KeyModifiers, MainUi,
};

// =============================================================================
// ColorScheme and Theme System
// =============================================================================

/// User's color scheme preference
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum ColorScheme {
    Dark,
    Light,
}

/// A complete set of colors for one theme variant (dark or light)
#[allow(dead_code)]
struct ThemeVariant {
    // Background colors
    app_bg: Color,
    topbar_bg: Color,
    prompt_bg: Color,
    // Text colors
    text_normal: Color,
    text_secondary: Color,
    text_highlight: Color,
    // Border/Accent colors
    border_focused: Color,
    border_unfocused: Color,
    accent_primary: Color,
    accent_secondary: Color,
    // Special colors
    success: Color,
    mode_active: Color,
}

impl ThemeVariant {
    fn dark() -> Self {
        Self {
            app_bg: Color::Rgb(10, 10, 10),            // #0a0a0a
            topbar_bg: Color::Rgb(20, 20, 20),         // #141414
            prompt_bg: Color::Rgb(30, 30, 30),         // #1e1e1e
            text_normal: Color::Rgb(255, 255, 255),    // Pure white
            text_secondary: Color::Rgb(115, 115, 115), // #737373
            text_highlight: Color::Cyan,
            border_focused: Color::Cyan,
            border_unfocused: Color::Gray,
            accent_primary: Color::Yellow,
            accent_secondary: Color::Blue,
            success: Color::Green,
            mode_active: Color::Yellow,
        }
    }

    fn light() -> Self {
        Self {
            app_bg: Color::Rgb(245, 245, 245),    // Light gray
            topbar_bg: Color::Rgb(235, 235, 235), // Slightly darker than app_bg
            prompt_bg: Color::Rgb(220, 220, 220), // Slightly darker gray
            text_normal: Color::Rgb(30, 30, 30),  // Dark text
            text_secondary: Color::Rgb(100, 100, 100),
            text_highlight: Color::Rgb(0, 100, 200),
            border_focused: Color::Rgb(0, 100, 200),
            border_unfocused: Color::Rgb(150, 150, 150),
            accent_primary: Color::Rgb(200, 150, 0),
            accent_secondary: Color::Rgb(50, 100, 200),
            success: Color::Rgb(0, 150, 0),
            mode_active: Color::Rgb(200, 150, 0),
        }
    }
}

/// A theme can be adaptive (supports both dark and light) or forced (single variant)
#[allow(dead_code)]
enum Theme {
    /// Theme adapts to the color scheme preference
    Adaptive {
        dark: ThemeVariant,
        light: ThemeVariant,
    },
    /// Theme forces a specific variant regardless of preference
    Forced(ThemeVariant),
}

impl Theme {
    /// Create a new adaptive theme with both dark and light variants
    fn adaptive() -> Self {
        Theme::Adaptive {
            dark: ThemeVariant::dark(),
            light: ThemeVariant::light(),
        }
    }

    /// Check if this theme supports a specific color scheme
    #[allow(dead_code)]
    fn supports_scheme(&self, scheme: ColorScheme) -> bool {
        match self {
            Theme::Adaptive { .. } => true, // Adaptive themes support both
            Theme::Forced(variant) => {
                // Check if the forced variant matches the scheme
                // We determine this by comparing app_bg color
                let is_dark = matches!(variant.app_bg, Color::Rgb(0, 0, 0));
                match scheme {
                    ColorScheme::Dark => is_dark,
                    ColorScheme::Light => !is_dark,
                }
            }
        }
    }

    /// Get the color variant for a specific scheme
    fn colors(&self, scheme: ColorScheme) -> &ThemeVariant {
        match self {
            Theme::Adaptive { dark, light } => match scheme {
                ColorScheme::Dark => dark,
                ColorScheme::Light => light,
            },
            Theme::Forced(variant) => variant, // Ignores scheme preference
        }
    }
}

// =============================================================================
// Mock Data
// =============================================================================

const LOREM_IPSUM: &[&str] = &[
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
    "Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
    "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.",
    "Duis aute irure dolor in reprehenderit in voluptate velit esse.",
    "Excepteur sint occaecat cupidatat non proident, sunt in culpa.",
    "",
    "Sed ut perspiciatis unde omnis iste natus error sit voluptatem.",
    "Accusantium doloremque laudantium, totam rem aperiam, eaque ipsa.",
    "Quae ab illo inventore veritatis et quasi architecto beatae.",
    "Vitae dicta sunt explicabo. Nemo enim ipsam voluptatem.",
    "",
    "Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet.",
    "Consectetur, adipisci velit, sed quia non numquam eius modi tempora.",
    "Incidunt ut labore et dolore magnam aliquam quaerat voluptatem.",
    "Ut enim ad minima veniam, quis nostrum exercitationem ullam.",
    "Corporis suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur.",
    "",
    "Quis autem vel eum iure reprehenderit qui in ea voluptate.",
    "Velit esse quam nihil molestiae consequatur, vel illum qui dolorem.",
    "Eum fugiat quo voluptas nulla pariatur? At vero eos et accusamus.",
];

// =============================================================================
// TODO Item
// =============================================================================

struct TodoItem {
    text: &'static str,
    checked: bool,
}

impl TodoItem {
    fn new(text: &'static str, checked: bool) -> Self {
        Self { text, checked }
    }

    fn toggle(&mut self) {
        self.checked = !self.checked;
    }

    fn draw(&self, is_active: bool, is_focused: bool, theme: &ThemeVariant) -> Line<'static> {
        let checkbox = if self.checked { "[✓]" } else { "[ ]" };
        let text_style = if self.checked {
            Style::default()
                .fg(theme.text_secondary)
                .add_modifier(Modifier::CROSSED_OUT)
                .bg(theme.app_bg)
        } else if is_active && is_focused {
            Style::default()
                .fg(Color::Black)
                .bg(theme.text_highlight)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(theme.text_highlight)
                .add_modifier(Modifier::BOLD)
                .bg(theme.app_bg)
        } else {
            Style::default().fg(theme.text_normal).bg(theme.app_bg)
        };

        Line::from(vec![
            Span::styled(
                format!("{} ", checkbox),
                if is_active && is_focused {
                    Style::default().fg(Color::Black).bg(theme.text_highlight)
                } else if self.checked {
                    Style::default().fg(theme.success).bg(theme.app_bg)
                } else {
                    Style::default().fg(theme.text_normal).bg(theme.app_bg)
                },
            ),
            Span::styled(self.text.to_string(), text_style),
        ])
    }
}

// =============================================================================
// Sidebar Component
// =============================================================================

struct Sidebar {
    items: Vec<TodoItem>,
    active_index: usize,
    visible: bool,
}

impl Sidebar {
    fn new() -> Self {
        Self {
            items: vec![
                TodoItem::new("Review codebase structure", true),
                TodoItem::new("Analyze OpenTUI patterns", true),
                TodoItem::new("Write code example", false),
                TodoItem::new("Add syntax highlighting", false),
                TodoItem::new("Test keyboard navigation", false),
                TodoItem::new("Update documentation", false),
            ],
            active_index: 2, // Start on the active task
            visible: false,
        }
    }

    fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn next_item(&mut self) {
        if self.active_index < self.items.len().saturating_sub(1) {
            self.active_index += 1;
        }
    }

    fn prev_item(&mut self) {
        if self.active_index > 0 {
            self.active_index -= 1;
        }
    }

    fn toggle_current(&mut self) {
        if let Some(item) = self.items.get_mut(self.active_index) {
            item.toggle();
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, is_focused: bool, theme: &ThemeVariant) {
        if !self.visible {
            return;
        }

        let block = Block::default()
            .title(" TODO ")
            .borders(Borders::ALL)
            .border_style(if is_focused {
                Style::default().fg(theme.border_focused)
            } else {
                Style::default().fg(theme.border_unfocused)
            })
            .style(Style::default().bg(theme.app_bg));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Draw items
        let visible_count = inner.height as usize;
        let mut lines = Vec::new();

        for (idx, item) in self.items.iter().enumerate().take(visible_count) {
            let is_active = idx == self.active_index;
            lines.push(item.draw(is_active, is_focused, theme));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);

        // Draw scrollbar if needed
        if self.items.len() > visible_count {
            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state =
                ScrollbarState::new(self.items.len()).position(self.active_index);
            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.prev_item();
                    EventResult::Handled
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.next_item();
                    EventResult::Handled
                }
                KeyCode::Char(' ') => {
                    self.toggle_current();
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }
}

// =============================================================================
// Main Content Component
// =============================================================================

struct MainContent {
    scroll_offset: u16,
}

impl MainContent {
    fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    #[allow(dead_code)]
    fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    #[allow(dead_code)]
    fn scroll_down(&mut self, amount: u16) {
        let max_scroll = LOREM_IPSUM.len().saturating_sub(1) as u16;
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext, theme: &ThemeVariant) {
        let block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default().bg(theme.app_bg));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Get visible lines
        let visible_lines = inner.height as usize;
        let start_idx = self.scroll_offset as usize;
        let end_idx = (start_idx + visible_lines).min(LOREM_IPSUM.len());

        let lines: Vec<Line> = LOREM_IPSUM[start_idx..end_idx]
            .iter()
            .map(|text| {
                Line::from(Span::styled(
                    text.to_string(),
                    Style::default().fg(theme.text_normal).bg(theme.app_bg),
                ))
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);

        // Draw scrollbar
        let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state =
            ScrollbarState::new(LOREM_IPSUM.len()).position(self.scroll_offset as usize);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    #[allow(dead_code)]
    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.scroll_up(1);
                    EventResult::Handled
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.scroll_down(1);
                    EventResult::Handled
                }
                KeyCode::PageUp => {
                    self.scroll_up(5);
                    EventResult::Handled
                }
                KeyCode::PageDown => {
                    self.scroll_down(5);
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            }
        } else {
            EventResult::Unhandled
        }
    }
}

// =============================================================================
// Top Bar Component
// =============================================================================

struct TopBar;

impl TopBar {
    fn draw(frame: &mut Frame, area: Rect, theme: &ThemeVariant) {
        let border_color = theme.text_secondary;
        let bg_color = theme.topbar_bg;

        // Left border characters
        let border_vertical = Paragraph::new("┃").style(Style::default().fg(border_color));

        // Draw left border on all 3 rows
        for y in area.y..area.y + area.height.min(3) {
            let border_rect = Rect {
                x: area.x,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(&border_vertical, border_rect);
        }

        // Content area (to the right of border, with 1-char right margin)
        let content_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2), // -1 for left border, -1 for right margin
            height: area.height,
        };

        // Fill entire content area with topbar background
        let bg = Block::default().style(Style::default().bg(bg_color));
        frame.render_widget(bg, content_area);

        // Middle row contains the title and metrics
        if area.height >= 2 {
            let middle_row_y = area.y + 1;
            let content_width = content_area.width as usize;

            // Left: Title
            let title = "  # Title of the conversation";
            // Right: Metrics
            let metrics = "21,638 8% ($0.02) v1.1.53";

            // Calculate spacing to right-align metrics
            let spacing = content_width.saturating_sub(title.len() + metrics.len());
            let spacer = " ".repeat(spacing);

            let line = Line::from(vec![
                Span::styled(
                    title,
                    Style::default()
                        .fg(theme.text_normal)
                        .add_modifier(Modifier::BOLD)
                        .bg(bg_color),
                ),
                Span::styled(spacer, Style::default().bg(bg_color)),
                Span::styled(
                    metrics,
                    Style::default().fg(theme.text_secondary).bg(bg_color),
                ),
            ]);

            let middle_row_area = Rect {
                x: content_area.x,
                y: middle_row_y,
                width: content_area.width,
                height: 1,
            };
            let paragraph = Paragraph::new(line);
            frame.render_widget(paragraph, middle_row_area);
        }
    }
}

// =============================================================================
// Prompt Area Component
// =============================================================================

struct PromptArea {
    textbox: TextBox,
    mode: &'static str,
    model: &'static str,
    provider: &'static str,
}

impl PromptArea {
    fn new() -> Self {
        // Create a config without borders for single-line input
        let config = TextBoxConfig {
            cursor_mode: CursorAnimationMode::Fade,
            cursor_fade: Some(CursorFadeConfig::default()),
            show_border: false,
            ..TextBoxConfig::default()
        };

        Self {
            textbox: TextBox::builder("prompt")
                .placeholder("Type your message here...")
                .config(config)
                .build(),
            mode: "Plan",
            model: "Kimi K2.5",
            provider: "OpenRouter",
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, is_focused: bool, theme: &ThemeVariant) {
        let mode_color = if self.mode == "Plan" {
            theme.accent_primary
        } else {
            theme.accent_secondary
        };

        let half_block_bg = theme.prompt_bg; // Use theme's prompt background

        // Layout: inner box area (includes half-row bottom), shortcuts line below
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Inner box with half-row bottom integrated
                Constraint::Length(1), // Shortcuts line (outside the box)
            ])
            .split(area);

        let inner_box_area = vertical_chunks[0];
        let shortcuts_area = vertical_chunks[1];

        // The last row combines both the half border (╹) and half line (▀)
        let box_height = inner_box_area.height;
        let last_row_y = inner_box_area.y + box_height - 1;

        // Draw left border: ┃ for all rows except last, ╹ for last row
        for y in inner_box_area.y..last_row_y {
            let border_char = Paragraph::new("┃").style(Style::default().fg(mode_color));
            let border_rect = Rect {
                x: inner_box_area.x,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(border_char, border_rect);
        }

        // Last row: ╹ for the left border
        let last_border_char = Paragraph::new("╹").style(Style::default().fg(mode_color));
        frame.render_widget(
            last_border_char,
            Rect {
                x: inner_box_area.x,
                y: last_row_y,
                width: 1,
                height: 1,
            },
        );

        // Draw contrasting background for the prompt box area (excluding the last row)
        let shaded_area = Rect {
            x: inner_box_area.x + 1,
            y: inner_box_area.y,
            width: inner_box_area.width.saturating_sub(1),
            height: box_height.saturating_sub(1), // Exclude the last row
        };
        let bg_block = Block::default().style(Style::default().bg(half_block_bg));
        frame.render_widget(bg_block, shaded_area);

        // Last row: ▀ (upper half-block) for the background to the right of ╹
        // First fill the entire row with app background (black)
        let last_row_bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(
            last_row_bg,
            Rect {
                x: inner_box_area.x + 1,
                y: last_row_y,
                width: inner_box_area.width.saturating_sub(1),
                height: 1,
            },
        );

        let half_row_width = inner_box_area.width.saturating_sub(1);
        let half_row_line = "▀".repeat(half_row_width as usize);
        let half_row_widget =
            Paragraph::new(half_row_line).style(Style::default().fg(half_block_bg));
        frame.render_widget(
            half_row_widget,
            Rect {
                x: inner_box_area.x + 1,
                y: last_row_y,
                width: half_row_width,
                height: 1,
            },
        );

        // Content area with 1 char padding from left (after border) and right
        let content_area = Rect {
            x: inner_box_area.x + 2, // 1 for border + 1 padding
            y: inner_box_area.y,
            width: inner_box_area.width.saturating_sub(3), // -1 border -2 padding
            height: box_height.saturating_sub(1),          // Exclude the last row
        };

        // Split content: margin, textbox, margin, status
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Top margin
                Constraint::Length(1), // Prompt (single line, no border)
                Constraint::Length(1), // Bottom margin
                Constraint::Length(1), // Status
            ])
            .split(content_area);

        // Draw textbox (indented to match "Plan" text)
        let textbox_area = Rect {
            x: content_chunks[1].x + 1, // +1 to align with "Plan" text (after the leading space)
            y: content_chunks[1].y,
            width: content_chunks[1].width.saturating_sub(1),
            height: content_chunks[1].height,
        };
        self.textbox.draw(frame, textbox_area, is_focused);

        // Draw status line: mode in accent color, text_normal, text_secondary
        let mode_style = Style::default().fg(mode_color).add_modifier(Modifier::BOLD);
        let model_style = Style::default().fg(theme.text_normal);
        let provider_style = Style::default().fg(theme.text_secondary);

        let status_line = Line::from(vec![
            Span::styled(format!(" {} ", self.mode), mode_style),
            Span::raw(" "),
            Span::styled(self.model.to_string(), model_style),
            Span::raw(" • "),
            Span::styled(self.provider.to_string(), provider_style),
        ]);
        frame.render_widget(Paragraph::new(status_line), content_chunks[3]);

        // Fill shortcuts area background first
        let shortcuts_bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(shortcuts_bg, shortcuts_area);

        // Shortcuts line: "tab agents ctrl+p commands" with text_normal keys and text_secondary labels
        let shortcuts_line = Line::from(vec![
            Span::styled(
                "tab ",
                Style::default().fg(theme.text_normal).bg(theme.app_bg),
            ),
            Span::styled(
                "agents ",
                Style::default().fg(theme.text_secondary).bg(theme.app_bg),
            ),
            Span::styled(
                "ctrl+p ",
                Style::default().fg(theme.text_normal).bg(theme.app_bg),
            ),
            Span::styled(
                "commands",
                Style::default().fg(theme.text_secondary).bg(theme.app_bg),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(shortcuts_line).alignment(ratatui::layout::Alignment::Right),
            shortcuts_area,
        );
    }

    fn handle_event(&mut self, event: &Event, _ctx: &mut AppContext) -> EventResult {
        let result = self.textbox.handle_event(event);

        // Process textbox events
        let events = self.textbox.take_events();
        for evt in events {
            if let TextBoxEvent::Submit(_text) = evt {
                // Handle submission - text is in _text
            }
        }

        result
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        if let Some(mut anim_ctx) = ctx.control_animations() {
            self.textbox.tick(&mut anim_ctx)
        } else {
            false
        }
    }
}

// =============================================================================
// Main Application
// =============================================================================

enum FocusArea {
    Sidebar,
    Prompt,
}

struct CodeViewerApp {
    sidebar: Sidebar,
    main_content: MainContent,
    prompt_area: PromptArea,
    focus: FocusArea,
    color_scheme: ColorScheme,
    theme: Theme,
}

impl CodeViewerApp {
    fn new() -> Self {
        let mut prompt_area = PromptArea::new();
        prompt_area.textbox.on_focus();

        Self {
            sidebar: Sidebar::new(),
            main_content: MainContent::new(),
            prompt_area,
            focus: FocusArea::Prompt,
            color_scheme: ColorScheme::Dark,
            theme: Theme::adaptive(),
        }
    }

    /// Toggle between dark and light color schemes
    #[allow(dead_code)]
    fn toggle_color_scheme(&mut self) {
        self.color_scheme = match self.color_scheme {
            ColorScheme::Dark => ColorScheme::Light,
            ColorScheme::Light => ColorScheme::Dark,
        };
    }

    /// Set the color scheme
    #[allow(dead_code)]
    fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
    }

    /// Get current color scheme
    #[allow(dead_code)]
    fn color_scheme(&self) -> ColorScheme {
        self.color_scheme
    }

    /// Check if current theme supports a specific color scheme
    #[allow(dead_code)]
    fn theme_supports(&self, scheme: ColorScheme) -> bool {
        self.theme.supports_scheme(scheme)
    }

    /// Get the active theme variant based on current color scheme
    fn active_theme(&self) -> &ThemeVariant {
        self.theme.colors(self.color_scheme)
    }

    fn switch_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::Prompt => {
                if self.sidebar.is_visible() {
                    FocusArea::Sidebar
                } else {
                    FocusArea::Prompt
                }
            }
            FocusArea::Sidebar => FocusArea::Prompt,
        };

        // Update focus in textbox
        match self.focus {
            FocusArea::Prompt => {
                self.prompt_area.textbox.on_focus();
            }
            _ => {
                self.prompt_area.textbox.on_blur();
            }
        }
    }

    fn is_focused(&self, area: FocusArea) -> bool {
        matches!(
            (&self.focus, area),
            (FocusArea::Sidebar, FocusArea::Sidebar) | (FocusArea::Prompt, FocusArea::Prompt)
        )
    }
}

impl Component for CodeViewerApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let theme = self.active_theme();

        // Create horizontal margin: 2 columns padding on left and right
        let horizontal_margin = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2), // Left padding
                Constraint::Min(1),    // Main content area
                Constraint::Length(2), // Right padding
            ])
            .split(area);

        let main_area = horizontal_margin[1];

        // Layout: top margin, top bar, margin, main area (content + sidebar), bottom prompt, bottom padding
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Top margin
                Constraint::Length(3), // Top bar (padding + content + padding)
                Constraint::Length(1), // Margin between topbar and content
                Constraint::Min(10),   // Main area (content + sidebar)
                Constraint::Length(6), // Bottom prompt area (5 for box with half-row bottom + 1 for shortcuts)
                Constraint::Length(1), // Bottom padding row (empty, background filled)
            ])
            .split(main_area);

        // Fill left and right padding with background color
        let padding_bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(padding_bg.clone(), horizontal_margin[0]);
        frame.render_widget(padding_bg, horizontal_margin[2]);

        // Fill top margin with app background
        let top_margin = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(top_margin, main_chunks[0]);

        // Top bar
        TopBar::draw(frame, main_chunks[1], theme);

        // Fill margin between topbar and content with app background
        let mid_margin = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(mid_margin, main_chunks[2]);

        // Fill main area background with theme's app background before drawing content
        let main_bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(main_bg.clone(), main_chunks[3]);

        // Main area: content + sidebar (right side)
        if self.sidebar.is_visible() {
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(75), // Main content
                    Constraint::Percentage(25), // Sidebar (right)
                ])
                .split(main_chunks[3]);

            // Main content
            self.main_content.draw(frame, content_chunks[0], ctx, theme);

            // Sidebar (on the right)
            let sidebar_focused = self.is_focused(FocusArea::Sidebar);
            self.sidebar
                .draw(frame, content_chunks[1], sidebar_focused, theme);
        } else {
            // No sidebar, use full area for content
            self.main_content.draw(frame, main_chunks[3], ctx, theme);
        }

        // Bottom prompt area
        let prompt_focused = self.is_focused(FocusArea::Prompt);
        self.prompt_area
            .draw(frame, main_chunks[4], prompt_focused, theme);

        // Bottom padding row - fill with theme's app background
        let bottom_padding_area = main_chunks[5];
        let bottom_padding = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(bottom_padding, bottom_padding_area);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Handle quit
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }

        // Handle Tab to switch focus
        if let Event::Key(key) = event {
            if key.code == KeyCode::Tab {
                self.switch_focus();
                return EventResult::Handled;
            }

            // Toggle sidebar with Ctrl+;
            if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('/') {
                self.sidebar.toggle();
                return EventResult::Handled;
            }

            // Toggle log console with backtick (`)
            if key.modifiers.contains(KeyModifiers::ALT) && key.code == KeyCode::Char('`') {
                if let Some(mut overlays) = ctx.dev_overlays() {
                    overlays.toggle_log_viewer();
                }
                return EventResult::Handled;
            }

            // Toggle debug panel with F12
            if key.code == KeyCode::F(12) {
                if let Some(mut overlays) = ctx.dev_overlays() {
                    overlays.toggle_debug_panel();
                }
                return EventResult::Handled;
            }

            // Handle 'q' only when not in prompt
            if key.code == KeyCode::Char('q') && !matches!(self.focus, FocusArea::Prompt) {
                ctx.quit();
                return EventResult::Handled;
            }
        }

        // Route events to focused component
        match self.focus {
            FocusArea::Sidebar => self.sidebar.handle_event(event, ctx),
            FocusArea::Prompt => self.prompt_area.handle_event(event, ctx),
        }
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        self.prompt_area.tick(ctx)
    }
}

impl MainUi for CodeViewerApp {}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(CodeViewerApp::new())
        .tick_rate(Duration::from_millis(100))
        .mouse_capture(false)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
