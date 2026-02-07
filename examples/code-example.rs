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
//! - b: Toggle sidebar visibility
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
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, KeyCode, MainUi,
};

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

    fn draw(&self, is_active: bool, is_focused: bool) -> Line<'static> {
        let checkbox = if self.checked { "[✓]" } else { "[ ]" };
        let text_style = if self.checked {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::CROSSED_OUT)
        } else if is_active && is_focused {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        Line::from(vec![
            Span::styled(
                format!("{} ", checkbox),
                if is_active && is_focused {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else if self.checked {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::White)
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
            visible: true,
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

    fn draw(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        if !self.visible {
            return;
        }

        let block = Block::default()
            .title(" TODO ")
            .borders(Borders::ALL)
            .border_style(if is_focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::Gray)
            });

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Draw items
        let visible_count = inner.height as usize;
        let mut lines = Vec::new();

        for (idx, item) in self.items.iter().enumerate().take(visible_count) {
            let is_active = idx == self.active_index;
            lines.push(item.draw(is_active, is_focused));
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

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext) {
        let block = Block::default().borders(Borders::NONE);

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
                    Style::default().fg(Color::White),
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
    fn draw(frame: &mut Frame, area: Rect) {
        // Left: Title
        let title = Span::styled(
            " OpenCode Editor ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

        // Right: Context metrics
        let metrics = Span::styled(
            " 12.4K  45%  $0.023  v1.0.0 ",
            Style::default().fg(Color::DarkGray),
        );

        let line = Line::from(vec![title, Span::raw(""), metrics]);
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
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

    fn draw(&self, frame: &mut Frame, area: Rect, is_focused: bool) {
        let mode_color = if self.mode == "Plan" {
            Color::Yellow
        } else {
            Color::Blue
        };

        // Layout: inner box area, shortcuts line below
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Inner box (margin + textbox + margin + status + spacing)
                Constraint::Length(1), // Shortcuts line (outside the box)
            ])
            .split(area);

        let inner_box_area = vertical_chunks[0];
        let shortcuts_area = vertical_chunks[1];

        // Draw the inner box with left border and default background
        // Left border in mode color spanning the whole box
        for y in inner_box_area.y..(inner_box_area.y + inner_box_area.height) {
            let border_char = Paragraph::new("│").style(Style::default().fg(mode_color));
            let border_rect = Rect {
                x: inner_box_area.x,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(border_char, border_rect);
        }

        // Draw contrasting background for the prompt box area
        let shaded_area = Rect {
            x: inner_box_area.x + 1, // After the left border
            y: inner_box_area.y,
            width: inner_box_area.width.saturating_sub(1),
            height: inner_box_area.height,
        };
        let bg_block = Block::default().style(Style::default().bg(Color::Rgb(45, 45, 45))); // More contrasting background
        frame.render_widget(bg_block, shaded_area);

        // Content area with 1 char padding from left (after border) and right
        let content_area = Rect {
            x: inner_box_area.x + 2, // 1 for border + 1 padding
            y: inner_box_area.y,
            width: inner_box_area.width.saturating_sub(3), // -1 border -2 padding
            height: inner_box_area.height,
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

        // Draw textbox (no prefix, full width)
        let textbox_area = Rect {
            x: content_chunks[1].x,
            y: content_chunks[1].y,
            width: content_chunks[1].width,
            height: content_chunks[1].height,
        };
        self.textbox.draw(frame, textbox_area, is_focused);

        // Draw status line: yellow/blue "Plan"/"Build", white model, gray provider
        let mode_style = Style::default().fg(mode_color).add_modifier(Modifier::BOLD);
        let model_style = Style::default().fg(Color::White);
        let provider_style = Style::default().fg(Color::DarkGray);

        let status_line = Line::from(vec![
            Span::styled(format!(" {} ", self.mode), mode_style),
            Span::raw(" "),
            Span::styled(self.model.to_string(), model_style),
            Span::raw(" • "),
            Span::styled(self.provider.to_string(), provider_style),
        ]);
        frame.render_widget(Paragraph::new(status_line), content_chunks[3]);

        // Shortcuts line: "tab agents ctrl+p commands" with white keys and gray labels
        let shortcuts_line = Line::from(vec![
            Span::styled("tab ", Style::default().fg(Color::White)),
            Span::styled("agents ", Style::default().fg(Color::DarkGray)),
            Span::styled("ctrl+p ", Style::default().fg(Color::White)),
            Span::styled("commands", Style::default().fg(Color::DarkGray)),
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
        }
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
        // Layout: top bar, main area (content + sidebar), bottom prompt
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Top bar
                Constraint::Min(10),   // Main area (content + sidebar)
                Constraint::Length(6), // Bottom prompt area (5 for box + 1 for shortcuts)
            ])
            .split(area);

        // Top bar
        TopBar::draw(frame, main_chunks[0]);

        // Main area: content + sidebar (right side)
        if self.sidebar.is_visible() {
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(75), // Main content
                    Constraint::Percentage(25), // Sidebar (right)
                ])
                .split(main_chunks[1]);

            // Main content
            self.main_content.draw(frame, content_chunks[0], ctx);

            // Sidebar (on the right)
            let sidebar_focused = self.is_focused(FocusArea::Sidebar);
            self.sidebar.draw(frame, content_chunks[1], sidebar_focused);
        } else {
            // No sidebar, use full area for content
            self.main_content.draw(frame, main_chunks[1], ctx);
        }

        // Bottom prompt area
        let prompt_focused = self.is_focused(FocusArea::Prompt);
        self.prompt_area.draw(frame, main_chunks[2], prompt_focused);
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

            // Toggle sidebar with 'b'
            if key.code == KeyCode::Char('b') {
                self.sidebar.toggle();
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
