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

use crossterm::cursor::{Hide, SetCursorStyle, Show};
use ratatui::{
    layout::{Position, Rect},
    style::{Color, Style},
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
    /// Text was copied to clipboard (clipboard feature required).
    #[cfg(feature = "clipboard")]
    Copied(String),
    /// Text was cut to clipboard (clipboard feature required).
    #[cfg(feature = "clipboard")]
    Cut(String),
}

impl ControlEvent for TextBoxEvent {}

/// Cursor shape options for the terminal cursor.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CursorShape {
    /// Block cursor (█)
    #[default]
    Block,
    /// Bar cursor (|)
    Bar,
}

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
    /// Cursor shape (Block or Bar).
    pub cursor_shape: CursorShape,
    /// Whether to mask input (password field).
    pub password_mask: Option<char>,
    /// Maximum length of input (None for unlimited).
    pub max_length: Option<usize>,
    /// Style for selected text (when selection feature is enabled).
    #[cfg(feature = "selection")]
    pub selection_style: Style,
    /// Whether to show the border around the textbox.
    pub show_border: bool,
}

impl Default for TextBoxConfig {
    fn default() -> Self {
        Self {
            cursor_blink: CursorBlinkConfig::default(),
            cursor_mode: CursorAnimationMode::default(),
            cursor_fade: Some(CursorFadeConfig::default()),
            focused_style: Style::default().fg(Color::White),
            unfocused_style: Style::default().fg(Color::White),
            focused_border_style: Style::default().fg(Color::Yellow),
            unfocused_border_style: Style::default().fg(Color::White),
            placeholder_style: Style::default().fg(Color::Rgb(100, 100, 100)),
            cursor_style: Style::default().bg(Color::White).fg(Color::Black),
            cursor_shape: CursorShape::default(),
            password_mask: None,
            max_length: None,
            #[cfg(feature = "selection")]
            selection_style: Style::default().bg(Color::Blue).fg(Color::White),
            show_border: true,
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
            is_focused: false,
            terminal_focused: true,
            cursor_screen_pos: Cell::new(None),
            #[cfg(feature = "selection")]
            selection_anchor: None,
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
    /// Whether the textbox currently has focus (widget-level).
    is_focused: bool,
    /// Whether the terminal/app has focus (window-level).
    terminal_focused: bool,
    /// Cursor screen position (x, y) for terminal cursor placement (Cell for interior mutability).
    cursor_screen_pos: Cell<Option<(u16, u16)>>,
    /// Anchor position for text selection (when selection feature is enabled).
    #[cfg(feature = "selection")]
    selection_anchor: Option<usize>,
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
            is_focused: false,
            terminal_focused: true,
            cursor_screen_pos: Cell::new(None),
            #[cfg(feature = "selection")]
            selection_anchor: None,
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

    /// Check if there is an active selection.
    #[cfg(feature = "selection")]
    pub fn has_selection(&self) -> bool {
        self.selection_anchor
            .map_or(false, |anchor| anchor != self.cursor_pos)
    }

    /// Get the current selection range as (start, end) character indices.
    /// Returns None if there is no active selection.
    #[cfg(feature = "selection")]
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if anchor < self.cursor_pos {
                (anchor, self.cursor_pos)
            } else {
                (self.cursor_pos, anchor)
            }
        })
    }

    /// Get the currently selected text.
    /// Returns None if there is no selection.
    #[cfg(feature = "selection")]
    pub fn selected_text(&self) -> Option<String> {
        self.selection_range()
            .map(|(start, end)| self.text.chars().skip(start).take(end - start).collect())
    }

    /// Clear the current selection (but keep cursor position).
    #[cfg(feature = "selection")]
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    /// Set the selection anchor at the current cursor position if not already set.
    #[cfg(feature = "selection")]
    fn ensure_anchor(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_pos);
        }
    }

    /// Delete the selected text and return it.
    /// Clears the selection after deletion.
    /// Returns None if there is no selection.
    #[cfg(feature = "selection")]
    fn delete_selection(&mut self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let selected: String = self.text.chars().skip(start).take(end - start).collect();

        // Get byte indices for the range
        let start_byte = self
            .text
            .char_indices()
            .nth(start)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end_byte = self
            .text
            .char_indices()
            .nth(end)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());

        // Delete the selected portion
        self.text.drain(start_byte..end_byte);

        // Move cursor to start of selection and clear selection
        self.cursor_pos = start;
        self.clear_selection();
        self.mark_text_changed();

        Some(selected)
    }

    /// Select all text in the input.
    #[cfg(feature = "selection")]
    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_pos = self.text.chars().count();
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
        // Start at full brightness so cursor is visible immediately
        animation.set_progress(1.0);
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
        let (text_style, _border_style) = if focused {
            (self.config.focused_style, self.config.focused_border_style)
        } else {
            (
                self.config.unfocused_style,
                self.config.unfocused_border_style,
            )
        };

        let (inner, visible_width) = if self.config.show_border {
            // Create block with optional title
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_style(if focused {
                    self.config.focused_border_style
                } else {
                    self.config.unfocused_border_style
                });

            if let Some(ref title) = self.title {
                block = block.title(format!(" {} ", title));
            }

            let inner = block.inner(area);
            frame.render_widget(block, area);
            (inner, inner.width as usize)
        } else {
            // No border - use full area
            (area, area.width as usize)
        };

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

        // Render text content without cursor characters
        if is_empty {
            // Empty text - show placeholder if available
            if let Some(ref placeholder) = self.placeholder {
                let para =
                    Paragraph::new(placeholder.as_str()).style(self.config.placeholder_style);
                frame.render_widget(para, inner);
            }
        } else {
            // Has text - show visible portion
            let chars: Vec<char> = display.chars().collect();
            let scroll = scroll_offset.min(chars.len());
            let visible_chars: String = chars.iter().skip(scroll).take(visible_width).collect();

            let para = Paragraph::new(visible_chars).style(text_style);
            frame.render_widget(para, inner);
        }

        // Calculate cursor screen position (always needed for cursor display)
        let cursor_screen_x = if is_empty {
            // At start of empty textbox
            inner.x
        } else {
            // At cursor position in text
            let cursor_rel = self.cursor_pos.saturating_sub(scroll_offset);
            inner.x + cursor_rel as u16
        };
        let cursor_screen_y = inner.y;

        // Store cursor position
        self.cursor_screen_pos
            .set(Some((cursor_screen_x, cursor_screen_y)));

        // Handle terminal cursor positioning and styling
        if focused {
            // Always set cursor position when textbox is focused
            frame.set_cursor_position(Position::new(cursor_screen_x, cursor_screen_y));

            if self.terminal_focused {
                // Textbox focused + window active: show blinking cursor
                let cursor_style_cmd = match self.config.cursor_shape {
                    CursorShape::Block => SetCursorStyle::BlinkingBlock,
                    CursorShape::Bar => SetCursorStyle::BlinkingBar,
                };

                if let Err(e) = crossterm::execute!(std::io::stdout(), Show, cursor_style_cmd) {
                    tracing::warn!("Failed to set terminal cursor: {}", e);
                }
            } else {
                // Textbox focused + window inactive: show cursor but don't set style
                // This allows terminal to show its default inactive (hollow) cursor
                if let Err(e) = crossterm::execute!(std::io::stdout(), Show) {
                    tracing::warn!("Failed to show terminal cursor: {}", e);
                }
            }
        } else {
            // Textbox not focused: hide cursor completely
            if let Err(e) = crossterm::execute!(std::io::stdout(), Hide) {
                tracing::warn!("Failed to hide terminal cursor: {}", e);
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
                                    #[cfg(feature = "selection")]
                                    {
                                        self.select_all();
                                    }
                                    #[cfg(not(feature = "selection"))]
                                    {
                                        self.move_home();
                                    }
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
                                #[cfg(all(feature = "selection", feature = "clipboard"))]
                                'c' => {
                                    // Copy selected text to clipboard
                                    if let Some(selected) = self.selected_text() {
                                        if let Err(e) =
                                            crate::selection::clipboard::copy_to_clipboard(
                                                &selected,
                                            )
                                        {
                                            tracing::warn!("Failed to copy to clipboard: {}", e);
                                        }
                                        self.emit(TextBoxEvent::Copied(selected));
                                        true
                                    } else {
                                        false
                                    }
                                }
                                #[cfg(all(feature = "selection", feature = "clipboard"))]
                                'x' => {
                                    // Cut selected text to clipboard
                                    if let Some(selected) = self.selected_text() {
                                        if let Err(e) =
                                            crate::selection::clipboard::copy_to_clipboard(
                                                &selected,
                                            )
                                        {
                                            tracing::warn!("Failed to copy to clipboard: {}", e);
                                        }
                                        self.delete_selection();
                                        self.emit(TextBoxEvent::Changed(self.text.clone()));
                                        self.emit(TextBoxEvent::Cut(selected));
                                        true
                                    } else {
                                        false
                                    }
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
                            #[cfg(feature = "selection")]
                            {
                                // If there's a selection, delete it first
                                if self.has_selection() {
                                    self.delete_selection();
                                    self.emit(TextBoxEvent::Changed(self.text.clone()));
                                }
                            }
                            self.insert_char(c);
                            true
                        }
                    }

                    // Navigation
                    KeyCode::Left => {
                        #[cfg(feature = "selection")]
                        {
                            let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                            if shift_pressed {
                                self.ensure_anchor();
                                self.move_left();
                            } else {
                                // Without shift, check if we need to collapse selection
                                if self.has_selection() {
                                    // Move cursor to the start of selection and clear selection
                                    if let Some((start, _)) = self.selection_range() {
                                        self.cursor_pos = start;
                                    }
                                    self.clear_selection();
                                } else {
                                    self.move_left();
                                }
                            }
                            if !shift_pressed
                                && !key.modifiers.contains(KeyModifiers::CONTROL)
                                && !key.modifiers.contains(KeyModifiers::ALT)
                            {
                                self.clear_selection();
                            }
                        }
                        #[cfg(not(feature = "selection"))]
                        {
                            self.move_left();
                        }
                        true
                    }
                    KeyCode::Right => {
                        #[cfg(feature = "selection")]
                        {
                            let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                            if shift_pressed {
                                self.ensure_anchor();
                                self.move_right();
                            } else {
                                // Without shift, check if we need to collapse selection
                                if self.has_selection() {
                                    // Move cursor to the end of selection and clear selection
                                    if let Some((_, end)) = self.selection_range() {
                                        self.cursor_pos = end;
                                    }
                                    self.clear_selection();
                                } else {
                                    self.move_right();
                                }
                            }
                            if !shift_pressed
                                && !key.modifiers.contains(KeyModifiers::CONTROL)
                                && !key.modifiers.contains(KeyModifiers::ALT)
                            {
                                self.clear_selection();
                            }
                        }
                        #[cfg(not(feature = "selection"))]
                        {
                            self.move_right();
                        }
                        true
                    }
                    KeyCode::Home => {
                        #[cfg(feature = "selection")]
                        let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                        #[cfg(feature = "selection")]
                        if shift_pressed {
                            self.ensure_anchor();
                        }
                        self.move_home();
                        #[cfg(feature = "selection")]
                        if !shift_pressed {
                            self.clear_selection();
                        }
                        true
                    }
                    KeyCode::End => {
                        #[cfg(feature = "selection")]
                        let shift_pressed = key.modifiers.contains(KeyModifiers::SHIFT);
                        #[cfg(feature = "selection")]
                        if shift_pressed {
                            self.ensure_anchor();
                        }
                        self.move_end();
                        #[cfg(feature = "selection")]
                        if !shift_pressed {
                            self.clear_selection();
                        }
                        true
                    }

                    // Editing
                    KeyCode::Backspace => {
                        #[cfg(feature = "selection")]
                        {
                            if self.has_selection() {
                                self.delete_selection();
                                self.emit(TextBoxEvent::Changed(self.text.clone()));
                            } else if key.modifiers.contains(KeyModifiers::CONTROL)
                                || key.modifiers.contains(KeyModifiers::ALT)
                            {
                                self.delete_word_back();
                            } else {
                                self.backspace();
                            }
                        }
                        #[cfg(not(feature = "selection"))]
                        {
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                || key.modifiers.contains(KeyModifiers::ALT)
                            {
                                self.delete_word_back();
                            } else {
                                self.backspace();
                            }
                        }
                        true
                    }
                    KeyCode::Delete => {
                        #[cfg(feature = "selection")]
                        {
                            if self.has_selection() {
                                self.delete_selection();
                                self.emit(TextBoxEvent::Changed(self.text.clone()));
                            } else {
                                self.delete_char();
                            }
                        }
                        #[cfg(not(feature = "selection"))]
                        {
                            self.delete_char();
                        }
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

            Event::FocusGained => {
                self.terminal_focused = true;
                EventResult::Handled
            }

            Event::FocusLost => {
                self.terminal_focused = false;
                EventResult::Handled
            }

            _ => EventResult::Unhandled,
        }
    }

    fn tick(&mut self, ctx: &mut ControlAnimationContext<'_>) -> bool {
        const TYPING_IDLE_THRESHOLD_MS: u64 = 500;

        // Skip animation when widget is not focused OR terminal has lost focus
        if !self.is_focused || !self.terminal_focused {
            return false;
        }

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
        // Set focus flag
        self.is_focused = true;
        self.emit(TextBoxEvent::FocusGained);
    }

    fn on_blur(&mut self) {
        // Clear focus flag
        self.is_focused = false;
        self.cursor_screen_pos.set(None);
        // Hide terminal cursor
        let _ = crossterm::execute!(std::io::stdout(), Hide);
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
