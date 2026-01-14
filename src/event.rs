//! Event types and handling for the TUI framework.
//!
//! This module wraps crossterm events and provides a unified event interface.

pub use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEventKind};

use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};

/// Unified event type for the TUI framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Keyboard event
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize event
    Resize { width: u16, height: u16 },
    /// Focus gained
    FocusGained,
    /// Focus lost
    FocusLost,
    /// Paste event (if enabled)
    Paste(String),
}

impl Event {
    /// Check if this is a quit event (Ctrl+C or Ctrl+Q)
    #[inline]
    pub fn is_quit(&self) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent {
                code: KeyCode::Char('c') | KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                ..
            })
        )
    }

    /// Check if this is a specific key press
    #[inline]
    pub fn is_key(&self, code: KeyCode) -> bool {
        matches!(self, Event::Key(KeyEvent { code: c, .. }) if *c == code)
    }

    /// Check if this is a key press with modifiers
    #[inline]
    pub fn is_key_with_modifiers(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        matches!(
            self,
            Event::Key(KeyEvent { code: c, modifiers: m, .. }) if *c == code && *m == modifiers
        )
    }

    /// Check if this is a mouse click
    #[inline]
    pub fn is_mouse_click(&self) -> bool {
        matches!(
            self,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(_),
                ..
            })
        )
    }

    /// Get mouse position if this is a mouse event
    #[inline]
    pub fn mouse_position(&self) -> Option<(u16, u16)> {
        match self {
            Event::Mouse(MouseEvent { column, row, .. }) => Some((*column, *row)),
            _ => None,
        }
    }
}

impl From<CrosstermEvent> for Event {
    fn from(event: CrosstermEvent) -> Self {
        match event {
            CrosstermEvent::Key(key) => Event::Key(key),
            CrosstermEvent::Mouse(mouse) => Event::Mouse(mouse),
            CrosstermEvent::Resize(width, height) => Event::Resize { width, height },
            CrosstermEvent::FocusGained => Event::FocusGained,
            CrosstermEvent::FocusLost => Event::FocusLost,
            CrosstermEvent::Paste(text) => Event::Paste(text),
        }
    }
}
