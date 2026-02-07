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
        CometBar, Control, CursorAnimationMode, CursorFadeConfig, TextBox, TextBoxConfig,
        TextBoxEvent,
    },
    AppBuilder, AppContext, CanQuit, Component, DrawContext, Event, EventResult, KeyCode,
    KeyModifiers, MainUi, Task, TaskContext, TaskSender,
};

// =============================================================================
// Network Connection Manager Task
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum NetworkMessage {
    Connected,
    RequestSent,
    Thinking,
    Responding(String),
    Waiting,
    Disconnected,
}

struct NetworkTask {
    control: NetworkControl,
}

impl NetworkTask {
    fn new(control: NetworkControl) -> Self {
        Self { control }
    }
}

impl Task for NetworkTask {
    type Message = NetworkMessage;
    type Error = std::convert::Infallible;

    async fn run(
        self,
        sender: TaskSender<Self::Message>,
        mut ctx: TaskContext,
    ) -> Result<(), Self::Error> {
        // Simulate connection establishment
        tokio::time::sleep(Duration::from_millis(500)).await;
        if sender.send(NetworkMessage::Connected).await.is_err() {
            return Ok(());
        }

        // Main loop - wait for requests and process them
        loop {
            tokio::select! {
                _ = ctx.cancelled() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Check for pending requests using the control
                    if self.control.has_pending_request() {
                        self.control.clear_request();
                        
                        // Process the request
                        if sender.send(NetworkMessage::RequestSent).await.is_err() {
                            break;
                        }

                        // Thinking phase (1 second)
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        if sender.send(NetworkMessage::Thinking).await.is_err() {
                            break;
                        }

                        // Responding phase (2 seconds with content streaming)
                        let response_text = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.";

                        for word in response_text.split_whitespace() {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            if sender.send(NetworkMessage::Responding(word.to_string())).await.is_err() {
                                break;
                            }
                        }

                        // Waiting phase
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        if sender.send(NetworkMessage::Waiting).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// External control for network task
#[derive(Clone)]
struct NetworkControl {
    has_pending_request: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl NetworkControl {
    fn new() -> Self {
        Self {
            has_pending_request: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn send_request(&self) {
        self.has_pending_request.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn has_pending_request(&self) -> bool {
        self.has_pending_request.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn clear_request(&self) {
        self.has_pending_request.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

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

#[allow(dead_code)]
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
// Participant System (Extensible for multiple users/agents)
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum Participant {
    User { id: usize, name: String },
    Assistant { id: usize, name: String },
    Agent { id: usize, name: String },
}

#[allow(dead_code)]
impl Participant {
    fn user(id: usize) -> Self {
        Participant::User {
            id,
            name: format!("User {}", id),
        }
    }

    fn assistant(id: usize) -> Self {
        Participant::Assistant {
            id,
            name: format!("Assistant {}", id),
        }
    }

    fn agent(id: usize) -> Self {
        Participant::Agent {
            id,
            name: format!("Agent {}", id),
        }
    }

    fn name(&self) -> &str {
        match self {
            Participant::User { name, .. } => name,
            Participant::Assistant { name, .. } => name,
            Participant::Agent { name, .. } => name,
        }
    }

    fn color(&self, theme: &ThemeVariant) -> Color {
        match self {
            Participant::User { id: 1, .. } => theme.text_highlight,
            Participant::User { id: 2, .. } => Color::Magenta,
            Participant::Assistant { .. } => theme.accent_primary,
            Participant::Agent { id: 1, .. } => Color::Green,
            Participant::Agent { id: 2, .. } => Color::Blue,
            _ => theme.text_normal,
        }
    }
}

fn current_user() -> Participant {
    Participant::user(1)
}

fn current_assistant() -> Participant {
    Participant::assistant(1)
}

// =============================================================================
// Message Model
// =============================================================================

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
enum MessageState {
    Thinking,
    Responding,
    Waiting,
    Complete,
}

struct Message {
    participant: Participant,
    content: String,
    state: MessageState,
}

#[allow(dead_code)]
impl Message {
    fn new(participant: Participant, content: impl Into<String>) -> Self {
        Self {
            participant,
            content: content.into(),
            state: MessageState::Complete,
        }
    }

    fn thinking(participant: Participant) -> Self {
        Self {
            participant,
            content: String::new(),
            state: MessageState::Thinking,
        }
    }

    fn is_thinking(&self) -> bool {
        matches!(self.state, MessageState::Thinking)
    }

    fn is_responding(&self) -> bool {
        matches!(self.state, MessageState::Responding)
    }

    fn is_waiting(&self) -> bool {
        matches!(self.state, MessageState::Waiting)
    }

    fn set_thinking(&mut self) {
        self.state = MessageState::Thinking;
    }

    fn set_responding(&mut self) {
        self.state = MessageState::Responding;
    }

    fn set_waiting(&mut self) {
        self.state = MessageState::Waiting;
    }

    fn complete(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.state = MessageState::Complete;
    }
}

// =============================================================================
// Session Model
// =============================================================================

struct Session {
    #[allow(dead_code)]
    participants: Vec<Participant>,
    history: Vec<Message>,
    scroll_offset: usize,
}

#[allow(dead_code)]
impl Session {
    fn new() -> Self {
        Self {
            participants: vec![current_user(), current_assistant()],
            history: Vec::new(),
            scroll_offset: 0,
        }
    }

    fn add_message(&mut self, participant: Participant, content: impl Into<String>) {
        let msg = Message::new(participant, content);
        self.history.push(msg);
        self.scroll_to_bottom();
    }

    fn add_thinking_message(&mut self, participant: Participant) -> usize {
        let msg = Message::thinking(participant);
        self.history.push(msg);
        self.scroll_to_bottom();
        self.history.len() - 1
    }

    fn update_message_content(&mut self, idx: usize, content: impl Into<String>) {
        if let Some(msg) = self.history.get_mut(idx) {
            msg.content = content.into();
        }
    }

    fn set_message_state(&mut self, idx: usize, state: MessageState) {
        if let Some(msg) = self.history.get_mut(idx) {
            msg.state = state;
        }
    }

    fn scroll_to_bottom(&mut self) {
        // Scroll offset is now in lines from the top
        // When content overflows, we want to show from the bottom
        // So we set scroll_offset to a large value (will be clamped in draw)
        self.scroll_offset = usize::MAX;
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }
}

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
// Conversation Component
// =============================================================================

#[allow(dead_code)]
struct Conversation {
    messages: Vec<Message>,
    scroll_offset: usize,
    lorem_index: usize,
}

#[allow(dead_code)]
impl Conversation {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            lorem_index: 0,
        }
    }

    fn add_user_message(&mut self, content: impl Into<String>) {
        let msg = Message::new(current_user(), content);
        self.messages.push(msg);
        self.scroll_to_bottom();
    }

    fn add_assistant_thinking(&mut self) {
        let msg = Message::thinking(current_assistant());
        self.messages.push(msg);
        self.scroll_to_bottom();
    }

    fn complete_assistant_message(&mut self, content: impl Into<String>) {
        if let Some(last_msg) = self.messages.last_mut() {
            if last_msg.is_thinking() {
                last_msg.complete(content);
            }
        }
    }

    fn get_next_lorem_response(&mut self) -> String {
        let start_idx = self.lorem_index % LOREM_IPSUM.len();
        let end_idx = (start_idx + 3).min(LOREM_IPSUM.len());
        let lines: Vec<&str> = LOREM_IPSUM[start_idx..end_idx].to_vec();
        self.lorem_index = end_idx;
        lines.join("\n")
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.messages.len().saturating_sub(1);
    }

    fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.messages.len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    fn draw(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext, theme: &ThemeVariant) {
        // Fill background
        let bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(bg, area);

        if self.messages.is_empty() {
            return;
        }

        // Calculate layout for messages
        let mut current_y = area.y;
        let mut message_areas: Vec<(usize, Rect)> = Vec::new();

        for (idx, _msg) in self.messages.iter().enumerate().skip(self.scroll_offset) {
            if current_y >= area.y + area.height {
                break;
            }

            // Padding above
            current_y += 1;

            // Message box - calculate height based on content
            // Minimum height: 3 lines (padding + content + padding)
            let content_lines = if let Some(msg) = self.messages.get(idx) {
                if msg.state == MessageState::Complete {
                    msg.content.lines().count().max(1)
                } else {
                    1 // Thinking/Responding/Waiting states show single line
                }
            } else {
                1
            };
            let msg_height = (content_lines as u16 + 2).min(area.y + area.height - current_y); // +2 for padding
            
            let msg_area = Rect {
                x: area.x,
                y: current_y,
                width: area.width,
                height: msg_height,
            };
            message_areas.push((idx, msg_area));

            current_y += msg_area.height + 1; // +1 for padding below
        }

        // Draw each message
        for (idx, msg_area) in message_areas {
            if let Some(msg) = self.messages.get(idx) {
                self.draw_message(frame, msg_area, msg, theme);
            }
        }

        // Draw scrollbar
        if self.messages.len() > 1 {
            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(self.messages.len()).position(self.scroll_offset);
            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn draw_message(&self, frame: &mut Frame, area: Rect, msg: &Message, theme: &ThemeVariant) {
        let accent_color = msg.participant.color(theme);

        // Draw left border for all rows in the message box
        let border_style = Style::default().fg(accent_color);
        for y in area.y..area.y + area.height {
            let border_rect = Rect {
                x: area.x,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new("┃").style(border_style),
                border_rect,
            );
        }

        // Content area (to the right of border, with top padding)
        // Minimum box height is 3: padding + content + padding
        let content_area = Rect {
            x: area.x + 1,
            y: area.y + 1, // +1 for top padding
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(2), // -2 for top and bottom padding
        };

        // Fill the entire box area with background
        let box_bg = Block::default().style(Style::default().bg(theme.app_bg));
        let box_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(1),
            height: area.height,
        };
        frame.render_widget(box_bg, box_area);

        let state_text = if msg.is_thinking() {
            "Thinking"
        } else if msg.is_responding() {
            "Responding"
        } else if msg.is_waiting() {
            "Waiting"
        } else {
            ""
        };

        if !state_text.is_empty() {
            // Show state in light grey
            let line = Line::from(vec![
                Span::styled(
                    state_text,
                    Style::default().fg(theme.text_secondary).bg(theme.app_bg),
                ),
            ]);
            let paragraph = Paragraph::new(line);
            frame.render_widget(paragraph, content_area);
        } else {
            // Show message content
            let content_lines: Vec<Line> = msg
                .content
                .lines()
                .take(content_area.height as usize)
                .map(|line| {
                    Line::from(vec![
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(theme.text_normal).bg(theme.app_bg),
                        ),
                    ])
                })
                .collect();
            let paragraph = Paragraph::new(content_lines);
            frame.render_widget(paragraph, content_area);
        }
    }

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
    submitted_text: Option<String>,
    comet_bar: CometBar,
    is_thinking: bool,
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

        let mode_color = Color::Yellow; // Default "Plan" color

        Self {
            textbox: TextBox::builder("prompt")
                .placeholder("Type your message here...")
                .config(config)
                .build(),
            mode: "Plan",
            model: "Kimi K2.5",
            provider: "OpenRouter",
            submitted_text: None,
            comet_bar: CometBar::new().with_color(mode_color),
            is_thinking: false,
        }
    }

    fn take_submitted_text(&mut self) -> Option<String> {
        self.submitted_text.take()
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

        // Build the left side: comet bar only (always visible for testing)
        let left_side_width = 10; // 8 for comet + 2 margins
        let left_side_area = Rect {
            x: shortcuts_area.x,
            y: shortcuts_area.y,
            width: left_side_width,
            height: shortcuts_area.height,
        };

        // Draw comet bar (8 chars wide)
        let comet_area = Rect {
            x: left_side_area.x,
            y: left_side_area.y,
            width: 8,
            height: left_side_area.height,
        };
        self.comet_bar.draw(frame, comet_area, false);

        // Shortcuts line: "esc interrupt tab agents ctrl+p commands"
        let mut right_spans = vec![];

        // Always show esc + interrupt for testing
        right_spans.push(Span::styled(
            " esc ",
            Style::default().fg(theme.text_normal).bg(theme.app_bg),
        ));
        right_spans.push(Span::styled(
            "interrupt ",
            Style::default().fg(theme.text_secondary).bg(theme.app_bg),
        ));

        // Original shortcuts
        right_spans.push(Span::styled(
            "tab ",
            Style::default().fg(theme.text_normal).bg(theme.app_bg),
        ));
        right_spans.push(Span::styled(
            "agents ",
            Style::default().fg(theme.text_secondary).bg(theme.app_bg),
        ));
        right_spans.push(Span::styled(
            "ctrl+p ",
            Style::default().fg(theme.text_normal).bg(theme.app_bg),
        ));
        right_spans.push(Span::styled(
            "commands",
            Style::default().fg(theme.text_secondary).bg(theme.app_bg),
        ));

        let shortcuts_line = Line::from(right_spans);
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
            if let TextBoxEvent::Submit(text) = evt {
                // Store submitted text and clear the textbox
                self.submitted_text = Some(text);
                self.textbox.clear();
            }
        }

        result
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        let mut needs_redraw = false;

        if let Some(mut anim_ctx) = ctx.control_animations() {
            needs_redraw |= self.textbox.tick(&mut anim_ctx);

            // Always animate comet bar for testing
            needs_redraw |= self.comet_bar.tick(&mut anim_ctx);
        }

        needs_redraw
    }

    fn set_thinking(&mut self, thinking: bool, mode_color: Color) {
        self.is_thinking = thinking;
        if thinking {
            self.comet_bar = CometBar::new().with_color(mode_color);
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
    sessions: Vec<Session>,
    current_session: usize,
    prompt_area: PromptArea,
    focus: FocusArea,
    color_scheme: ColorScheme,
    theme: Theme,
    network_control: NetworkControl,
    connected: bool,
    current_message_idx: Option<usize>,
}

impl CodeViewerApp {
    fn new() -> Self {
        let mut prompt_area = PromptArea::new();
        prompt_area.textbox.on_focus();

        Self {
            sidebar: Sidebar::new(),
            sessions: vec![Session::new()],
            current_session: 0,
            prompt_area,
            focus: FocusArea::Prompt,
            color_scheme: ColorScheme::Dark,
            theme: Theme::adaptive(),
            network_control: NetworkControl::new(),
            connected: false,
            current_message_idx: None,
        }
    }

    fn network_control(&self) -> NetworkControl {
        self.network_control.clone()
    }

    fn submit_message(&mut self, text: String, _ctx: &mut AppContext) {
        if text.trim().is_empty() {
            return;
        }

        let session = &mut self.sessions[self.current_session];

        // Add user message to session history
        session.add_message(current_user(), &text);

        // Add assistant thinking message
        let assistant_idx = session.add_thinking_message(current_assistant());

        // Signal network task to send request
        self.network_control.send_request();
        self.current_message_idx = Some(assistant_idx);

        // Show comet bar
        let mode_color = if self.prompt_area.mode == "Plan" {
            self.active_theme().accent_primary
        } else {
            self.active_theme().accent_secondary
        };
        self.prompt_area.set_thinking(true, mode_color);
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

    fn draw_session(&self, frame: &mut Frame, area: Rect, _ctx: &DrawContext, theme: &ThemeVariant) {
        let session = &self.sessions[self.current_session];
        
        // Fill background
        let bg = Block::default().style(Style::default().bg(theme.app_bg));
        frame.render_widget(bg, area);

        if session.history.is_empty() {
            return;
        }

        // Calculate total height needed for all messages
        let mut total_height: u16 = 0;
        let message_heights: Vec<u16> = session.history.iter().map(|msg| {
            let content_lines = if msg.state == MessageState::Complete {
                msg.content.lines().count().max(1)
            } else {
                1
            };
            (content_lines as u16 + 2) + 1 // +2 for padding, +1 for spacing below
        }).collect();
        
        for height in &message_heights {
            total_height += *height;
        }
        // Remove the last spacing (no padding after last message)
        total_height = total_height.saturating_sub(1);

        let available_height = area.height;
        
        // Calculate how much we need to scroll
        // If content fits: no scroll needed, start from top
        // If content overflows: scroll to show bottom, unless user scrolled up
        let overflow = total_height.saturating_sub(available_height);
        
        // scroll_offset is how many lines user scrolled up from bottom
        // Clamp it to valid range
        let max_scroll_offset = overflow as usize;
        let scroll_offset = session.scroll_offset.min(max_scroll_offset);
        
        // Calculate starting Y position
        // Start from top normally, or from (top - overflow + scroll_offset) when overflowing
        let start_y = if overflow == 0 {
            area.y
        } else {
            // Start high enough so the bottom of content is at the bottom of area
            // Then subtract scroll_offset to allow scrolling up
            area.y.saturating_sub(overflow) + (scroll_offset as u16)
        };

        // Draw messages
        let mut current_y = start_y;
        let mut message_areas: Vec<(usize, Rect)> = Vec::new();

        for (idx, msg_height) in message_heights.iter().enumerate() {
            // Skip if completely above visible area
            if current_y + *msg_height <= area.y {
                current_y += *msg_height;
                continue;
            }
            
            // Stop if completely below visible area
            if current_y >= area.y + area.height {
                break;
            }

            // Calculate visible portion
            let visible_y = current_y.max(area.y);
            let visible_height = (*msg_height - (visible_y - current_y))
                .min(area.y + area.height - visible_y);
            
            let msg_area = Rect {
                x: area.x,
                y: visible_y,
                width: area.width,
                height: visible_height,
            };
            message_areas.push((idx, msg_area));

            current_y += *msg_height;
        }

        // Draw each message
        for (idx, msg_area) in message_areas {
            if let Some(msg) = session.history.get(idx) {
                self.draw_message(frame, msg_area, msg, theme);
            }
        }

        // Draw scrollbar if content overflows
        if overflow > 0 {
            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
            let mut scrollbar_state = ScrollbarState::new(max_scroll_offset + 1).position(scroll_offset);
            frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        }
    }

    fn draw_message(&self, frame: &mut Frame, area: Rect, msg: &Message, theme: &ThemeVariant) {
        let accent_color = msg.participant.color(theme);

        // Draw left border for all rows in the message box
        let border_style = Style::default().fg(accent_color);
        for y in area.y..area.y + area.height {
            let border_rect = Rect {
                x: area.x,
                y,
                width: 1,
                height: 1,
            };
            frame.render_widget(
                Paragraph::new("┃").style(border_style),
                border_rect,
            );
        }

        // Content area (to the right of border, with top padding)
        // Minimum box height is 3: padding + content + padding
        let content_area = Rect {
            x: area.x + 1,
            y: area.y + 1, // +1 for top padding
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(2), // -2 for top and bottom padding
        };

        // Fill the entire box area with background
        let box_bg = Block::default().style(Style::default().bg(theme.app_bg));
        let box_area = Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(1),
            height: area.height,
        };
        frame.render_widget(box_bg, box_area);

        let state_text = if msg.is_thinking() {
            "Thinking"
        } else if msg.is_responding() {
            "Responding"
        } else if msg.is_waiting() {
            "Waiting"
        } else {
            ""
        };

        if !state_text.is_empty() {
            // Show state in light grey
            let line = Line::from(vec![
                Span::styled(
                    state_text,
                    Style::default().fg(theme.text_secondary).bg(theme.app_bg),
                ),
            ]);
            let paragraph = Paragraph::new(line);
            frame.render_widget(paragraph, content_area);
        } else {
            // Show message content
            let content_lines: Vec<Line> = msg
                .content
                .lines()
                .take(content_area.height as usize)
                .map(|line| {
                    Line::from(vec![
                        Span::styled(
                            line.to_string(),
                            Style::default().fg(theme.text_normal).bg(theme.app_bg),
                        ),
                    ])
                })
                .collect();
            let paragraph = Paragraph::new(content_lines);
            frame.render_widget(paragraph, content_area);
        }
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

            // Session conversation
            self.draw_session(frame, content_chunks[0], ctx, theme);

            // Sidebar (on the right)
            let sidebar_focused = self.is_focused(FocusArea::Sidebar);
            self.sidebar
                .draw(frame, content_chunks[1], sidebar_focused, theme);
        } else {
            // No sidebar, use full area for conversation
            self.draw_session(frame, main_chunks[3], ctx, theme);
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
        let result = match self.focus {
            FocusArea::Sidebar => self.sidebar.handle_event(event, ctx),
            FocusArea::Prompt => self.prompt_area.handle_event(event, ctx),
        };

        // Check for submitted message from prompt
        if let Some(text) = self.prompt_area.take_submitted_text() {
            self.submit_message(text, ctx);
        }

        result
    }

    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        self.prompt_area.tick(ctx)
    }
}

impl MainUi for CodeViewerApp {
    fn handle_task_message(
        &mut self,
        task_name: &str,
        message: Box<dyn std::any::Any + Send>,
        _ctx: &mut AppContext,
    ) -> bool {
        if task_name == "network" {
            if let Some(msg) = message.downcast_ref::<NetworkMessage>() {
                let session = &mut self.sessions[self.current_session];
                
                match msg {
                    NetworkMessage::Connected => {
                        self.connected = true;
                        return true;
                    }
                    NetworkMessage::RequestSent => {
                        // Request has been sent, waiting for response
                        return true;
                    }
                    NetworkMessage::Thinking => {
                        // Assistant is thinking
                        if let Some(idx) = self.current_message_idx {
                            session.set_message_state(idx, MessageState::Thinking);
                        }
                        // Show comet bar
                        let mode_color = if self.prompt_area.mode == "Plan" {
                            self.active_theme().accent_primary
                        } else {
                            self.active_theme().accent_secondary
                        };
                        self.prompt_area.set_thinking(true, mode_color);
                        return true;
                    }
                    NetworkMessage::Responding(content) => {
                        // Assistant is responding with content - accumulate words
                        if let Some(idx) = self.current_message_idx {
                            session.set_message_state(idx, MessageState::Responding);
                            if let Some(msg) = session.history.get_mut(idx) {
                                if !msg.content.is_empty() {
                                    msg.content.push(' ');
                                }
                                msg.content.push_str(content);
                            }
                        }
                        return true;
                    }
                    NetworkMessage::Waiting => {
                        // Response complete, mark as complete to show content
                        if let Some(idx) = self.current_message_idx {
                            session.set_message_state(idx, MessageState::Complete);
                        }
                        self.current_message_idx = None;
                        self.network_control.clear_request();
                        // Hide comet bar
                        let mode_color = if self.prompt_area.mode == "Plan" {
                            self.active_theme().accent_primary
                        } else {
                            self.active_theme().accent_secondary
                        };
                        self.prompt_area.set_thinking(false, mode_color);
                        return true;
                    }
                    NetworkMessage::Disconnected => {
                        self.connected = false;
                        return true;
                    }
                }
            }
        }
        false
    }
}

// =============================================================================
// Main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let log_rx = args.init_tracing();

    // Create app state first to get network control
    let app_state = CodeViewerApp::new();
    let network_control = app_state.network_control();

    // Build the application
    let app = AppBuilder::new()
        .main_ui(app_state)
        .add_task("network", NetworkTask::new(network_control))
        .tick_rate(Duration::from_millis(100))
        .mouse_capture(false)
        .with_log_receiver(log_rx)
        .build()?;

    // Run the application
    app.run().await?;

    Ok(())
}
