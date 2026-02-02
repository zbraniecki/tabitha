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

// Submodules
mod dialog;
mod focus;
mod rendering;
mod types;

// Re-export all public types
pub use dialog::Modal;
pub use types::{ModalButton, ModalConfig, ModalInput, ModalResult};

use ratatui::{layout::Rect, Frame};

use crate::event::Event;
use crate::theme::Theme;

use focus::event_handlers::{ModalButtonHandlers, ModalInputHandlers, ModalNavigation};
use focus::ModalFocus;

/// Wrapper struct to implement event handler traits for Modal in ModalManager.
///
/// This allows ModalManager to use the shared event handling logic while
/// maintaining ownership of the modal.
struct ModalWrapper<'a>(&'a mut Modal);

impl ModalNavigation for ModalWrapper<'_> {
    fn focus_next(&mut self) {
        self.0.focus_next();
    }

    fn focus_prev(&mut self) {
        self.0.focus_prev();
    }

    fn focus_next_button(&mut self) {
        self.0.focus_next_button();
    }

    fn focus_prev_button(&mut self) {
        self.0.focus_prev_button();
    }

    fn is_buttons_focused(&self) -> bool {
        self.0.focus == ModalFocus::Buttons
    }
}

impl ModalInputHandlers for ModalWrapper<'_> {
    fn handle_input_char(&mut self, c: char) {
        self.0.handle_input_char(c);
    }

    fn handle_input_backspace(&mut self) {
        self.0.handle_input_backspace();
    }

    fn handle_input_delete(&mut self) {
        self.0.handle_input_delete();
    }

    fn move_input_cursor_left(&mut self) {
        self.0.move_input_cursor_left();
    }

    fn move_input_cursor_right(&mut self) {
        self.0.move_input_cursor_right();
    }

    fn move_input_cursor_home(&mut self) {
        self.0.move_input_cursor_home();
    }

    fn move_input_cursor_end(&mut self) {
        self.0.move_input_cursor_end();
    }

    fn move_focus_to_buttons(&mut self) {
        self.0.focus = ModalFocus::Buttons;
        self.0.focused_button = 0;
    }

    fn activate_first_button(&mut self) {
        self.0.activate_focused_button();
    }

    fn dismiss(&mut self) {
        self.0.result = ModalResult::Dismissed;
        self.0.open = false;
    }
}

impl ModalButtonHandlers for ModalWrapper<'_> {
    fn activate_focused_button(&mut self) {
        self.0.activate_focused_button();
    }

    fn dismiss(&mut self) {
        self.0.result = ModalResult::Dismissed;
        self.0.open = false;
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
            self.last_result = Some(modal.result.clone());
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
        use focus::event_handlers::{
            handle_button_keys, handle_input_keys, handle_navigation_keys,
        };

        let Some(modal) = self.current.as_mut() else {
            return false;
        };

        // Handle the event using shared handlers
        if let Event::Key(_) = event {
            let has_input = modal.input.is_some();

            // Handle input-specific keys when input is focused
            if modal.focus == ModalFocus::Input && has_input {
                if handle_input_keys(event, &mut ModalWrapper(modal)).is_some() {
                    // Input was handled
                }
            } else {
                // Handle button navigation and activation
                handle_button_keys(event, &mut ModalWrapper(modal));
            }

            // Handle navigation keys (works in both input and button contexts)
            handle_navigation_keys(event, &mut ModalWrapper(modal), has_input);
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
    pub(crate) fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        use ratatui::{
            text::{Line, Span},
            widgets::Paragraph,
        };

        if let Some(modal) = &self.current {
            // 1. Draw backdrop (full screen dark overlay)
            rendering::draw_backdrop(frame, area, &modal.config);

            // 2. Calculate centered modal rect and draw modal box
            let modal_rect = rendering::calculate_modal_rect(area, &modal.config);
            let inner = rendering::draw_modal_box(frame, modal_rect, modal.title.as_deref(), theme);

            // 3. Layout: message (top) + optional input + buttons (bottom)
            let is_inline = modal.input.as_ref().map(|i| i.inline).unwrap_or(false);
            let input_height: u16 = if modal.input.is_some() && !is_inline {
                3
            } else {
                0
            };
            let button_lines: u16 = 2;
            let message_height = inner.height.saturating_sub(input_height + button_lines);

            let mut current_y = inner.y;

            // 4. Render message (and inline input if applicable)
            if message_height > 0 {
                let message_area = Rect::new(inner.x, current_y, inner.width, message_height);

                if is_inline {
                    // Render message and input on the same line
                    if let Some(ref input) = modal.input {
                        let is_focused = modal.focus == focus::ModalFocus::Input;
                        rendering::draw_inline_input(
                            frame,
                            message_area,
                            &modal.message,
                            input,
                            is_focused,
                            theme,
                        );
                    }
                } else {
                    let content_style = theme.fg_style();
                    let message_lines: Vec<Line> = modal
                        .message
                        .lines()
                        .map(|line| Line::from(Span::styled(line, content_style)))
                        .collect();
                    let paragraph = Paragraph::new(message_lines);
                    frame.render_widget(paragraph, message_area);
                }
                current_y += message_height;
            }

            // 5. Render input field (if present and not inline)
            if let Some(ref input) = modal.input {
                if input_height > 0 && !is_inline {
                    let input_area = Rect::new(inner.x, current_y, inner.width, input_height);
                    let is_focused = modal.focus == focus::ModalFocus::Input;
                    rendering::draw_input(frame, input_area, input, is_focused, theme);
                    current_y += input_height;
                }
            }

            // 6. Render buttons
            if current_y < inner.y + inner.height {
                let button_area = Rect::new(inner.x, current_y, inner.width, 1);
                let buttons_focused = modal.focus == focus::ModalFocus::Buttons;
                rendering::draw_buttons(
                    frame,
                    button_area,
                    &modal.buttons,
                    modal.focused_button,
                    buttons_focused,
                    theme,
                );
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
