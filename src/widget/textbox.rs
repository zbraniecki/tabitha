//! TextBox widget for single-line text input.
//!
//! The TextBox provides a text input field with:
//! - Cursor navigation (Home, End, Left, Right)
//! - Text editing (insert, delete, backspace)
//! - Cursor blinking (configurable)
//! - Event emission for changes, key events, and submission
//! - Optional placeholder text
//! - Optional password masking
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{TextBox, TextBoxConfig, TextBoxEvent};
//!
//! let mut textbox = TextBox::new("username")
//!     .with_placeholder("Enter username...");
//!
//! // In handle_event:
//! textbox.handle_event(&event);
//! for evt in textbox.take_events() {
//!     match evt {
//!         TextBoxEvent::Changed(text) => println!("Text changed: {}", text),
//!         TextBoxEvent::Submit(text) => println!("Submitted: {}", text),
//!         _ => {}
//!     }
//! }
//! ```

use std::cell::Cell;
use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::animation::{AnimationMode, ControlAnimationContext, FadeAnimation};
use crate::event::{Event, KeyCode, KeyModifiers};
use crate::focus::EventResult;
use crate::widget::{
    Control, ControlEvent, CursorAnimationMode, CursorBlinkConfig, CursorFadeConfig,
};
use std::time::Duration;

/// Events emitted by TextBox.
#[derive(Debug, Clone)]
pub enum TextBoxEvent {
    /// Text content changed. Contains the new text.
    Changed(String),
    /// Enter was pressed. Contains the current text.
    Submit(String),
    /// A key was pressed. Contains the key code.
    KeyDown(KeyCode),
    /// A key was released. Contains the key code.
    KeyUp(KeyCode),
    /// Focus was gained.
    FocusGained,
    /// Focus was lost.
    FocusLost,
}

impl ControlEvent for TextBoxEvent {}

/// Configuration for TextBox appearance and behavior.
#[derive(Debug, Clone)]
pub struct TextBoxConfig {
    /// Cursor blink configuration (used when cursor_mode is Blink).
    pub cursor_blink: CursorBlinkConfig,
    /// Cursor animation mode (Blink or Fade).
    pub cursor_mode: CursorAnimationMode,
    /// Cursor fade configuration (used when cursor_mode is Fade).
    pub cursor_fade: Option<CursorFadeConfig>,
    /// Style for the text when focused.
    pub focused_style: Style,
    /// Style for the text when unfocused.
    pub unfocused_style: Style,
    /// Style for the border when focused.
    pub focused_border_style: Style,
    /// Style for the border when unfocused.
    pub unfocused_border_style: Style,
    /// Style for placeholder text.
    pub placeholder_style: Style,
    /// Style for the cursor.
    pub cursor_style: Style,
    /// Whether to mask input (password field).
    pub password_mask: Option<char>,
    /// Maximum length of input (None for unlimited).
    pub max_length: Option<usize>,
}

impl Default for TextBoxConfig {
    fn default() -> Self {
        Self {
            cursor_blink: CursorBlinkConfig::default(),
            cursor_mode: CursorAnimationMode::default(),
            cursor_fade: Some(CursorFadeConfig::default()),
            focused_style: Style::default().fg(Color::White),
            unfocused_style: Style::default().fg(Color::Gray),
            focused_border_style: Style::default().fg(Color::Yellow),
            unfocused_border_style: Style::default().fg(Color::DarkGray),
            placeholder_style: Style::default().fg(Color::DarkGray),
            cursor_style: Style::default().bg(Color::White).fg(Color::Black),
            password_mask: None,
            max_length: None,
        }
    }
}

impl TextBoxConfig {
    /// Create a password field configuration.
    pub fn password() -> Self {
        Self {
            password_mask: Some('•'),
            ..Default::default()
        }
    }

    /// Create a theme-aware configuration from a theme.
    ///
    /// Uses theme colors for borders and text, making the TextBox respond
    /// to theme changes (e.g., dimming when modals are open).
    /// Note: This does not apply background colors - only foreground and border colors.
    pub fn from_theme(theme: &crate::theme::Theme) -> Self {
        Self {
            focused_style: theme.fg_style(), // Foreground only, no background
            unfocused_style: Style::default().fg(theme.muted_foreground), // Foreground only
            focused_border_style: theme.border_focused_style(),
            unfocused_border_style: theme.border_style(),
            placeholder_style: Style::default().fg(theme.muted_foreground), // Foreground only
            cursor_style: Style::default()
                .bg(theme.accent)
                .fg(theme.accent_foreground),
            ..Default::default()
        }
    }

    /// Set the password mask character.
    pub fn with_mask(mut self, mask: char) -> Self {
        self.password_mask = Some(mask);
        self
    }

    /// Set the maximum input length.
    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    /// Set the cursor blink configuration.
    pub fn with_cursor_blink(mut self, config: CursorBlinkConfig) -> Self {
        self.cursor_blink = config;
        self
    }
}

/// Builder for constructing a TextBox with complex configuration.
///
/// This provides a cleaner API for building TextBox widgets with multiple
/// configuration options. Use [TextBox::builder] to create a builder.
///
/// # Example
///
/// ```ignore
/// let textbox = TextBox::builder("username")
///     .placeholder("Enter username...")
///     .title("Username")
///     .max_length(20)
///     .config(TextBoxConfig::from_theme(&theme))
///     .build();
/// ```
pub struct TextBoxBuilder {
    id: String,
    text: Option<String>,
    placeholder: Option<String>,
    title: Option<String>,
    config: TextBoxConfig,
}

impl TextBoxBuilder {
    /// Create a new TextBoxBuilder with the given focus ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: None,
            placeholder: None,
            title: None,
            config: TextBoxConfig::default(),
        }
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the border title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the configuration.
    pub fn config(mut self, config: TextBoxConfig) -> Self {
        self.config = config;
        self
    }

    /// Set initial text content.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Set the password mask character.
    pub fn password_mask(mut self, mask: char) -> Self {
        self.config.password_mask = Some(mask);
        self
    }

    /// Set the maximum input length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.config.max_length = Some(max);
        self
    }

    /// Set the cursor blink configuration.
    pub fn cursor_blink(mut self, config: CursorBlinkConfig) -> Self {
        self.config.cursor_blink = config;
        self
    }

    /// Set the cursor animation mode.
    pub fn cursor_mode(mut self, mode: CursorAnimationMode) -> Self {
        self.config.cursor_mode = mode;
        self
    }

    /// Set the cursor fade configuration.
    pub fn cursor_fade(mut self, config: CursorFadeConfig) -> Self {
        self.config.cursor_fade = Some(config);
        self
    }

    /// Build the TextBox.
    pub fn build(self) -> TextBox {
        let text = self.text.unwrap_or_default();
        let cursor_pos = text.chars().count();
        let animation_id = format!("cursor_{}", self.id);

        TextBox {
            id: self.id,
            text,
            cursor_pos,
            placeholder: self.placeholder,
            title: self.title,
            config: self.config,
            animation_id,
            last_text_change: None,
            text_modified_this_tick: false,
            animation_registered: false,
            cursor_color: Color::White,
            events: Vec::new(),
            scroll_offset: Cell::new(0),
            cursor_visible: true,
            blink_accumulated: Duration::ZERO,
        }
    }
}

/// A single-line text input widget.
#[derive(Clone)]
pub struct TextBox {
    /// Unique identifier for focus tracking.
    id: String,
    /// The text content.
    text: String,
    /// Cursor position (character index).
    cursor_pos: usize,
    /// Optional placeholder text shown when empty.
    placeholder: Option<String>,
    /// Optional title/label for the border.
    title: Option<String>,
    /// Configuration.
    config: TextBoxConfig,
    /// Animation ID for cursor (registered with AnimationController).
    animation_id: String,
    /// Last time text was modified (for pausing animation during typing).
    last_text_change: Option<Instant>,
    /// Whether text was modified this tick (to detect continuous typing).
    text_modified_this_tick: bool,
    /// Whether the cursor animation has been registered with the controller.
    animation_registered: bool,
    /// Current cursor color (updated during tick, used during draw).
    cursor_color: Color,
    /// Pending events to be consumed by parent.
    events: Vec<TextBoxEvent>,
    /// Scroll offset for long text (Cell for interior mutability in draw).
    scroll_offset: Cell<usize>,
    /// Whether cursor is visible (for reduced animation mode).
    cursor_visible: bool,
    /// Accumulated time for reduced mode blink.
    blink_accumulated: Duration,
}

impl TextBox {
    /// Create a new TextBox with the given focus ID.
    ///
    /// For more complex configuration, use [TextBox::builder] instead.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        let animation_id = format!("cursor_{}", id);
        Self {
            id,
            text: String::new(),
            cursor_pos: 0,
            placeholder: None,
            title: None,
            config: TextBoxConfig::default(),
            animation_id,
            last_text_change: None,
            text_modified_this_tick: false,
            animation_registered: false,
            cursor_color: Color::White,
            events: Vec::new(),
            scroll_offset: Cell::new(0),
            cursor_visible: true,
            blink_accumulated: Duration::ZERO,
        }
    }

    /// Create a new TextBoxBuilder for complex configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let textbox = TextBox::builder("username")
    ///     .placeholder("Enter username...")
    ///     .title("Username")
    ///     .max_length(20)
    ///     .build();
    /// ```
    pub fn builder(id: impl Into<String>) -> TextBoxBuilder {
        TextBoxBuilder::new(id)
    }

    /// Set the placeholder text.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set the border title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the configuration.
    pub fn with_config(mut self, config: TextBoxConfig) -> Self {
        self.config = config;
        self
    }

    /// Set initial text content.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.cursor_pos = self.text.chars().count();
        self
    }

    /// Get the focus ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set the text content programmatically.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        let char_count = self.text.chars().count();
        if self.cursor_pos > char_count {
            self.cursor_pos = char_count;
        }
    }

    /// Get the cursor position (character index).
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Set the cursor position.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let max_pos = self.text.chars().count();
        self.cursor_pos = pos.min(max_pos);
    }

    /// Clear the text content.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_pos = 0;
        self.scroll_offset.set(0);
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &TextBoxConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut TextBoxConfig {
        &mut self.config
    }

    // --- Internal helpers ---

    /// Insert a character at the cursor position.
    fn insert_char(&mut self, c: char) {
        // Check max length
        if let Some(max) = self.config.max_length {
            if self.text.chars().count() >= max {
                return;
            }
        }

        // Convert cursor position to byte index
        let byte_idx = self
            .text
            .char_indices()
            .nth(self.cursor_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());

        self.text.insert(byte_idx, c);
        self.cursor_pos += 1;
        self.mark_text_changed();
        self.emit(TextBoxEvent::Changed(self.text.clone()));
    }

    /// Delete the character at the cursor position.
    fn delete_char(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor_pos < char_count {
            let byte_idx = self
                .text
                .char_indices()
                .nth(self.cursor_pos)
                .map(|(i, _)| i)
                .unwrap();
            self.text.remove(byte_idx);
            self.mark_text_changed();
            self.emit(TextBoxEvent::Changed(self.text.clone()));
        }
    }

    /// Delete the character before the cursor (backspace).
    fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.delete_char();
        }
    }

    /// Move cursor left.
    fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    /// Move cursor right.
    fn move_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }
    }

    /// Move cursor to start.
    fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to end.
    fn move_end(&mut self) {
        self.cursor_pos = self.text.chars().count();
    }

    /// Move cursor one word left.
    fn move_word_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let chars: Vec<char> = self.text.chars().collect();
        let mut pos = self.cursor_pos - 1;

        // Skip whitespace
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }

        // Skip word characters
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }

        self.cursor_pos = pos;
    }

    /// Move cursor one word right.
    fn move_word_right(&mut self) {
        let chars: Vec<char> = self.text.chars().collect();
        let len = chars.len();

        if self.cursor_pos >= len {
            return;
        }

        let mut pos = self.cursor_pos;

        // Skip current word
        while pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }

        // Skip whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }

        self.cursor_pos = pos;
    }

    /// Delete word before cursor.
    fn delete_word_back(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let old_pos = self.cursor_pos;
        self.move_word_left();
        let chars_to_delete = old_pos - self.cursor_pos;

        for _ in 0..chars_to_delete {
            self.delete_char();
        }
    }

    /// Emit an event.
    fn emit(&mut self, event: TextBoxEvent) {
        self.events.push(event);
    }

    /// Mark that text has been modified (will pause animation).
    fn mark_text_changed(&mut self) {
        self.text_modified_this_tick = true;
        self.last_text_change = Some(Instant::now());
    }

    /// Create the cursor fade animation for this textbox.
    fn create_cursor_animation(&self) -> FadeAnimation {
        let fade_config = self.config.cursor_fade.as_ref();
        let (dim_color, full_color) = match fade_config {
            Some(config) => (config.dim_color, config.full_color),
            None => ((0, 0, 0), (255, 255, 255)),
        };

        let fade_in_ms = fade_config.map(|c| c.fade_in_duration_ms).unwrap_or(350);
        let fade_out_ms = fade_config.map(|c| c.fade_out_duration_ms).unwrap_or(900);
        let hold_full_ms = fade_config.map(|c| c.hold_full_duration_ms).unwrap_or(400);
        let hold_dim_ms = fade_config.map(|c| c.hold_dim_duration_ms).unwrap_or(0);

        let mut animation = FadeAnimation::with_hold_times(
            dim_color,
            full_color,
            Duration::from_millis(fade_in_ms),
            Duration::from_millis(hold_full_ms),
            Duration::from_millis(hold_dim_ms),
        );

        // Set the fade out duration as well
        animation.set_fade_out_duration(Duration::from_millis(fade_out_ms));
        animation
    }

    /// Calculate the display text (potentially masked).
    fn display_text(&self) -> String {
        if let Some(mask) = self.config.password_mask {
            mask.to_string().repeat(self.text.chars().count())
        } else {
            self.text.clone()
        }
    }
}

impl Control for TextBox {
    type Event = TextBoxEvent;

    fn draw(&self, frame: &mut Frame, area: Rect, focused: bool) {
        // Determine styles based on focus
        let (text_style, border_style) = if focused {
            (self.config.focused_style, self.config.focused_border_style)
        } else {
            (
                self.config.unfocused_style,
                self.config.unfocused_border_style,
            )
        };

        // Create block with optional title
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        if let Some(ref title) = self.title {
            block = block.title(format!(" {} ", title));
        }

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Calculate visible width (accounting for borders)
        let visible_width = inner.width as usize;

        // Get display text
        let display = self.display_text();
        let is_empty = display.is_empty();

        // Update scroll to keep cursor visible (using Cell for interior mutability)
        if visible_width > 0 {
            let mut scroll_offset = self.scroll_offset.get();
            if self.cursor_pos < scroll_offset {
                scroll_offset = self.cursor_pos;
            } else if self.cursor_pos >= scroll_offset + visible_width {
                scroll_offset = self.cursor_pos.saturating_sub(visible_width - 1);
            }
            self.scroll_offset.set(scroll_offset);
        }
        let scroll_offset = self.scroll_offset.get();

        // Determine what to show - cursor always shows, but may be invisible (dim)
        let cursor_style = self.config.cursor_style.bg(self.cursor_color);
        let should_show_cursor = true;

        if is_empty {
            // Empty text - show placeholder (with cursor on first char when focused)
            if let Some(ref placeholder) = self.placeholder {
                if focused {
                    // Cursor on the first character of the placeholder
                    let mut chars = placeholder.chars();
                    if let Some(first_char) = chars.next() {
                        let rest: String = chars.collect();
                        let first_style = if should_show_cursor {
                            cursor_style
                        } else {
                            self.config.placeholder_style
                        };
                        let spans = vec![
                            Span::styled(first_char.to_string(), first_style),
                            Span::styled(rest, self.config.placeholder_style),
                        ];
                        let line = Line::from(spans);
                        let para = Paragraph::new(line);
                        frame.render_widget(para, inner);
                    }
                } else {
                    // Not focused - show placeholder normally
                    let para =
                        Paragraph::new(placeholder.as_str()).style(self.config.placeholder_style);
                    frame.render_widget(para, inner);
                }
            } else if focused && should_show_cursor {
                // No placeholder, just show cursor block
                let cursor_span = Span::styled(" ", cursor_style);
                let line = Line::from(vec![cursor_span]);
                let para = Paragraph::new(line);
                frame.render_widget(para, inner);
            }
        } else {
            // Has text - show text with cursor when focused
            let chars: Vec<char> = display.chars().collect();
            let scroll = scroll_offset.min(chars.len());

            // Build visible portion
            let visible_chars: String = chars.iter().skip(scroll).take(visible_width).collect();

            if focused {
                // Cursor position relative to scroll
                let cursor_rel = self.cursor_pos.saturating_sub(scroll);

                if cursor_rel <= visible_width {
                    // Build spans: before cursor, cursor char, after cursor
                    let before: String = visible_chars.chars().take(cursor_rel).collect();
                    let cursor_char = visible_chars
                        .chars()
                        .nth(cursor_rel)
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| " ".to_string());
                    let after: String = visible_chars.chars().skip(cursor_rel + 1).collect();

                    // Cursor char uses cursor style based on animation mode
                    let cursor_char_style = if should_show_cursor {
                        cursor_style
                    } else {
                        text_style
                    };

                    let spans = vec![
                        Span::styled(before, text_style),
                        Span::styled(cursor_char, cursor_char_style),
                        Span::styled(after, text_style),
                    ];

                    let line = Line::from(spans);
                    let para = Paragraph::new(line);
                    frame.render_widget(para, inner);
                } else {
                    // Cursor not in visible area
                    let para = Paragraph::new(visible_chars).style(text_style);
                    frame.render_widget(para, inner);
                }
            } else {
                // Not focused - no cursor
                let para = Paragraph::new(visible_chars).style(text_style);
                frame.render_widget(para, inner);
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::Key(key) => {
                // Emit KeyDown event
                self.emit(TextBoxEvent::KeyDown(key.code));

                // Note: reset_blink() is called only on text-modifying operations,
                // not on navigation keys. This allows cursor to keep blinking
                // when holding arrow keys.

                let handled = match key.code {
                    // Character input
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            match c {
                                'a' => {
                                    self.move_home();
                                    true
                                }
                                'e' => {
                                    self.move_end();
                                    true
                                }
                                'w' => {
                                    self.delete_word_back();
                                    true
                                }
                                'u' => {
                                    // Delete from cursor to start
                                    let new_text: String =
                                        self.text.chars().skip(self.cursor_pos).collect();
                                    self.text = new_text;
                                    self.cursor_pos = 0;
                                    self.mark_text_changed();
                                    self.emit(TextBoxEvent::Changed(self.text.clone()));
                                    true
                                }
                                'k' => {
                                    // Delete from cursor to end
                                    self.text = self.text.chars().take(self.cursor_pos).collect();
                                    self.mark_text_changed();
                                    self.emit(TextBoxEvent::Changed(self.text.clone()));
                                    true
                                }
                                _ => false,
                            }
                        } else if key.modifiers.contains(KeyModifiers::ALT) {
                            match c {
                                'b' => {
                                    self.move_word_left();
                                    true
                                }
                                'f' => {
                                    self.move_word_right();
                                    true
                                }
                                _ => false,
                            }
                        } else {
                            self.insert_char(c);
                            true
                        }
                    }

                    // Navigation
                    KeyCode::Left => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.move_word_left();
                        } else {
                            self.move_left();
                        }
                        true
                    }
                    KeyCode::Right => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.move_word_right();
                        } else {
                            self.move_right();
                        }
                        true
                    }
                    KeyCode::Home => {
                        self.move_home();
                        true
                    }
                    KeyCode::End => {
                        self.move_end();
                        true
                    }

                    // Editing
                    KeyCode::Backspace => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.delete_word_back();
                        } else {
                            self.backspace();
                        }
                        true
                    }
                    KeyCode::Delete => {
                        self.delete_char();
                        true
                    }

                    // Submission
                    KeyCode::Enter => {
                        self.emit(TextBoxEvent::Submit(self.text.clone()));
                        true
                    }

                    _ => false,
                };

                if handled {
                    EventResult::Handled
                } else {
                    EventResult::Unhandled
                }
            }

            Event::Paste(text) => {
                // Handle paste event
                for c in text.chars() {
                    if c == '\n' || c == '\r' {
                        continue; // Skip newlines in single-line input
                    }
                    self.insert_char(c);
                }
                EventResult::Handled
            }

            _ => EventResult::Unhandled,
        }
    }

    fn tick(&mut self, ctx: &mut ControlAnimationContext<'_>) -> bool {
        const TYPING_IDLE_THRESHOLD_MS: u64 = 500;

        // Register animation on first tick if not already done
        if !self.animation_registered {
            if !ctx.contains(&self.animation_id) {
                let animation = self.create_cursor_animation();
                ctx.add(&self.animation_id, animation);
            }
            self.animation_registered = true;
        }

        // Reset text modified flag
        let was_modified = self.text_modified_this_tick;
        self.text_modified_this_tick = false;

        // Check if we should pause due to typing
        let should_pause = if was_modified {
            true
        } else if let Some(last_change) = self.last_text_change {
            let idle_time_ms = last_change.elapsed().as_millis() as u64;
            idle_time_ms < TYPING_IDLE_THRESHOLD_MS
        } else {
            false
        };

        // Handle reduced animation mode - simple blink
        if ctx.mode() == AnimationMode::Reduced {
            if should_pause {
                // While typing, show cursor solid
                self.cursor_visible = true;
                self.cursor_color = Color::Rgb(255, 255, 255);
                return true;
            }

            // Use fixed time step for consistent blinking
            let base_elapsed = Duration::from_millis(16);
            let speed = ctx.speed_multiplier();
            let elapsed = Duration::from_nanos((base_elapsed.as_nanos() as f32 * speed) as u64);

            self.blink_accumulated += elapsed;
            let blink_interval = Duration::from_millis(800); // 800ms on, 800ms off

            // Toggle visibility based on accumulated time
            let cycle = self.blink_accumulated.as_millis() % (blink_interval.as_millis() * 2);
            let new_visible = cycle < blink_interval.as_millis();

            if new_visible != self.cursor_visible {
                self.cursor_visible = new_visible;
                self.cursor_color = if self.cursor_visible {
                    Color::Rgb(255, 255, 255)
                } else {
                    Color::DarkGray
                };
                return true;
            }
            return false;
        }

        // Full animation mode - use fade animation
        if should_pause {
            // Pause animation and force full brightness
            ctx.pause(&self.animation_id);
            ctx.set_progress(&self.animation_id, 1.0);
            let old_color = self.cursor_color;
            self.cursor_color = Color::Rgb(255, 255, 255);
            old_color != Color::Rgb(255, 255, 255)
        } else {
            // Resume animation
            ctx.resume(&self.animation_id);
            // Update cursor color from animation
            if let Some(color) = ctx.current_color(&self.animation_id) {
                let old_color = self.cursor_color;
                self.cursor_color = color;
                old_color != color
            } else {
                false
            }
        }
    }

    fn take_events(&mut self) -> Vec<TextBoxEvent> {
        std::mem::take(&mut self.events)
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    fn on_focus(&mut self) {
        // Reset to visible state
        self.cursor_color = Color::Rgb(255, 255, 255);
        self.emit(TextBoxEvent::FocusGained);
    }

    fn on_blur(&mut self) {
        // Hide cursor when not focused
        self.cursor_color = Color::Rgb(0, 0, 0);
        self.emit(TextBoxEvent::FocusLost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_textbox_insert() {
        let mut tb = TextBox::new("test");
        tb.insert_char('H');
        tb.insert_char('i');
        assert_eq!(tb.text(), "Hi");
        assert_eq!(tb.cursor_pos(), 2);
    }

    #[test]
    fn test_textbox_backspace() {
        let mut tb = TextBox::new("test").with_text("Hello");
        tb.backspace();
        assert_eq!(tb.text(), "Hell");
        assert_eq!(tb.cursor_pos(), 4);
    }

    #[test]
    fn test_textbox_cursor_movement() {
        let mut tb = TextBox::new("test").with_text("Hello");
        assert_eq!(tb.cursor_pos(), 5);

        tb.move_home();
        assert_eq!(tb.cursor_pos(), 0);

        tb.move_end();
        assert_eq!(tb.cursor_pos(), 5);

        tb.move_left();
        assert_eq!(tb.cursor_pos(), 4);

        tb.move_right();
        assert_eq!(tb.cursor_pos(), 5);
    }

    #[test]
    fn test_textbox_word_movement() {
        let mut tb = TextBox::new("test").with_text("hello world foo");
        assert_eq!(tb.cursor_pos(), 15);

        tb.move_word_left();
        assert_eq!(tb.cursor_pos(), 12); // start of "foo"

        tb.move_word_left();
        assert_eq!(tb.cursor_pos(), 6); // start of "world"

        tb.move_word_right();
        assert_eq!(tb.cursor_pos(), 12); // start of "foo"
    }

    #[test]
    fn test_textbox_max_length() {
        let mut tb = TextBox::new("test").with_config(TextBoxConfig::default().with_max_length(5));

        for c in "Hello World".chars() {
            tb.insert_char(c);
        }

        assert_eq!(tb.text(), "Hello");
        assert_eq!(tb.text().len(), 5);
    }

    #[test]
    fn test_textbox_password_mask() {
        let tb = TextBox::new("test")
            .with_config(TextBoxConfig::password())
            .with_text("secret");

        assert_eq!(tb.display_text(), "••••••");
    }

    #[test]
    fn test_textbox_events() {
        let mut tb = TextBox::new("test");
        tb.insert_char('a');

        let events = tb.take_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TextBoxEvent::Changed(ref s) if s == "a"));

        // Events should be drained
        assert!(!tb.has_events());
        assert!(tb.take_events().is_empty());
    }

    #[test]
    fn test_textbox_unicode() {
        let mut tb = TextBox::new("test");
        tb.insert_char('こ');
        tb.insert_char('ん');
        tb.insert_char('に');
        tb.insert_char('ち');
        tb.insert_char('は');

        assert_eq!(tb.text(), "こんにちは");
        assert_eq!(tb.cursor_pos(), 5);

        tb.move_left();
        tb.move_left();
        assert_eq!(tb.cursor_pos(), 3);

        tb.backspace();
        assert_eq!(tb.text(), "こんちは");
    }
}
