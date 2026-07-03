//! Modal dialog types and focus management.
//!
//! This module provides shared types used by the modal system.

use crate::event::{KeyCode, KeyModifiers};

/// Which element is currently focused within the modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalFocus {
    /// Focus is on the input field.
    Input,
    /// Focus is on the buttons.
    #[default]
    Buttons,
}

/// Shared event handling logic for modals.
///
/// This module extracts common event handling code between Modal and ModalManager
/// to avoid duplication.
pub mod event_handlers {
    use crossterm::event::KeyEventKind;

use super::*;

    /// Handle navigation keys that work in both input and button contexts.
    pub fn handle_navigation_keys<T>(
        key: &crate::event::Event,
        modal: &mut T,
        has_input: bool,
    ) -> bool
    where
        T: ModalNavigation,
    {
        if let crate::event::Event::Key(key_event) = key && key_event.kind == KeyEventKind::Press {
            match key_event.code {
                // Tab -> next element
                KeyCode::Tab if !key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                    if has_input {
                        modal.focus_next();
                    } else {
                        modal.focus_next_button();
                    }
                    true
                }
                // Shift+Tab or BackTab -> previous element
                KeyCode::Tab if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                    if has_input {
                        modal.focus_prev();
                    } else {
                        modal.focus_prev_button();
                    }
                    true
                }
                KeyCode::BackTab => {
                    if has_input {
                        modal.focus_prev();
                    } else {
                        modal.focus_prev_button();
                    }
                    true
                }
                // Right arrow -> next button (when buttons focused)
                KeyCode::Right if modal.is_buttons_focused() => {
                    modal.focus_next_button();
                    true
                }
                // Left arrow -> previous button (when buttons focused)
                KeyCode::Left if modal.is_buttons_focused() => {
                    modal.focus_prev_button();
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Handle input-specific keys.
    pub fn handle_input_keys<T>(key: &crate::event::Event, modal: &mut T) -> Option<InputAction>
    where
        T: ModalInputHandlers,
    {
        if let crate::event::Event::Key(key_event) = key && key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Char(c) => {
                    modal.handle_input_char(c);
                    Some(InputAction::Handled)
                }
                KeyCode::Backspace => {
                    modal.handle_input_backspace();
                    Some(InputAction::Handled)
                }
                KeyCode::Delete => {
                    modal.handle_input_delete();
                    Some(InputAction::Handled)
                }
                KeyCode::Left => {
                    modal.move_input_cursor_left();
                    Some(InputAction::Handled)
                }
                KeyCode::Right => {
                    modal.move_input_cursor_right();
                    Some(InputAction::Handled)
                }
                KeyCode::Home => {
                    modal.move_input_cursor_home();
                    Some(InputAction::Handled)
                }
                KeyCode::End => {
                    modal.move_input_cursor_end();
                    Some(InputAction::Handled)
                }
                KeyCode::Enter => {
                    modal.move_focus_to_buttons();
                    modal.activate_first_button();
                    Some(InputAction::Handled)
                }
                KeyCode::Esc => {
                    modal.dismiss();
                    Some(InputAction::Dismissed)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Handle button-specific keys.
    pub fn handle_button_keys<T>(key: &crate::event::Event, modal: &mut T) -> Option<ButtonAction>
    where
        T: ModalButtonHandlers,
    {
        if let crate::event::Event::Key(key_event) = key  && key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    modal.activate_focused_button();
                    Some(ButtonAction::Activated)
                }
                KeyCode::Esc => {
                    modal.dismiss();
                    Some(ButtonAction::Dismissed)
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Actions that can result from input key handling.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InputAction {
        /// The input was handled.
        Handled,
        /// The modal was dismissed.
        Dismissed,
    }

    /// Actions that can result from button key handling.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ButtonAction {
        /// A button was activated.
        Activated,
        /// The modal was dismissed.
        Dismissed,
    }

    /// Trait for modal navigation operations.
    pub trait ModalNavigation {
        /// Move focus to the next element (input <-> buttons).
        fn focus_next(&mut self);
        /// Move focus to the previous element (input <-> buttons).
        fn focus_prev(&mut self);
        /// Move to the next button.
        fn focus_next_button(&mut self);
        /// Move to the previous button.
        fn focus_prev_button(&mut self);
        /// Check if buttons are currently focused.
        fn is_buttons_focused(&self) -> bool;
    }

    /// Trait for modal input handling operations.
    pub trait ModalInputHandlers {
        /// Handle a character input.
        fn handle_input_char(&mut self, c: char);
        /// Handle backspace.
        fn handle_input_backspace(&mut self);
        /// Handle delete.
        fn handle_input_delete(&mut self);
        /// Move cursor left.
        fn move_input_cursor_left(&mut self);
        /// Move cursor right.
        fn move_input_cursor_right(&mut self);
        /// Move cursor to start.
        fn move_input_cursor_home(&mut self);
        /// Move cursor to end.
        fn move_input_cursor_end(&mut self);
        /// Move focus to buttons.
        fn move_focus_to_buttons(&mut self);
        /// Activate the first button.
        fn activate_first_button(&mut self);
        /// Dismiss the modal.
        fn dismiss(&mut self);
    }

    /// Trait for modal button handling operations.
    pub trait ModalButtonHandlers {
        /// Activate the currently focused button.
        fn activate_focused_button(&mut self);
        /// Dismiss the modal.
        fn dismiss(&mut self);
    }
}
