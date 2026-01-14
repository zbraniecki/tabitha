//! Focus management for the TUI framework.
//!
//! This module provides focus navigation and event propagation control.

/// Result of event handling that controls propagation.
///
/// Used as the return type for `handle_event` methods to indicate
/// whether the event was consumed and whether propagation should continue.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode};
///
/// impl Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///         if let Event::Key(key) = event {
///             match key.code {
///                 KeyCode::Down => {
///                     self.scroll_down();
///                     EventResult::Handled  // Consumed, stop propagation
///                 }
///                 _ => EventResult::Unhandled  // Let parent handle it
///             }
///         } else {
///             EventResult::Unhandled
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventResult {
    /// Event was not handled, continue propagation (bubble to parent).
    #[default]
    Unhandled,
    /// Event was handled, stop propagation.
    Handled,
    /// Stop propagation without marking as handled.
    ///
    /// Useful when you want to intercept an event but not "consume" it.
    StopPropagation,
}

impl EventResult {
    /// Check if the event was handled.
    #[inline]
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }

    /// Check if propagation should continue.
    ///
    /// Returns `true` only for `Unhandled`.
    #[inline]
    pub fn should_propagate(&self) -> bool {
        matches!(self, EventResult::Unhandled)
    }
}

impl From<bool> for EventResult {
    /// Convert from bool for backward compatibility.
    ///
    /// `true` maps to `Handled`, `false` maps to `Unhandled`.
    fn from(handled: bool) -> Self {
        if handled {
            EventResult::Handled
        } else {
            EventResult::Unhandled
        }
    }
}

impl From<EventResult> for bool {
    /// Convert to bool for backward compatibility.
    ///
    /// `Handled` maps to `true`, others to `false`.
    fn from(result: EventResult) -> Self {
        result.is_handled()
    }
}

/// Manages focus state and navigation.
///
/// The `FocusManager` tracks which UI elements are focusable and which
/// one currently has focus. It supports linear focus order navigation.
///
/// # Focus Model
///
/// - Each focusable element has a unique string ID
/// - Elements are registered in order (optionally with explicit order)
/// - Navigation moves through elements in registration order
/// - Only focused elements receive `handle_event` calls
///
/// # Example
///
/// ```ignore
/// // In your component:
/// impl Component for MyWidget {
///     fn focus_id(&self) -> Option<&str> {
///         Some("my_widget")
///     }
///
///     fn is_focusable(&self) -> bool {
///         true
///     }
/// }
///
/// // In your event handler:
/// if event.is_key(KeyCode::Tab) {
///     ctx.focus().focus_next();
///     return EventResult::Handled;
/// }
/// ```
pub struct FocusManager {
    /// Registered focusable elements in order.
    focus_order: Vec<String>,
    /// Index of the currently focused element in focus_order.
    focus_index: Option<usize>,
}

impl FocusManager {
    /// Create a new empty focus manager.
    pub fn new() -> Self {
        Self {
            focus_order: Vec::new(),
            focus_index: None,
        }
    }

    /// Get the ID of the currently focused element.
    pub fn focused_id(&self) -> Option<&str> {
        self.focus_index
            .and_then(|i| self.focus_order.get(i))
            .map(|s| s.as_str())
    }

    /// Check if a specific element is currently focused.
    pub fn is_focused(&self, id: &str) -> bool {
        self.focused_id() == Some(id)
    }

    /// Check if a specific element is in the focus chain.
    ///
    /// For flat focus, this is the same as `is_focused`.
    /// Future hierarchical focus could check ancestry.
    pub fn is_in_focus_chain(&self, id: &str) -> bool {
        self.is_focused(id)
    }

    /// Set focus to a specific element by ID.
    ///
    /// Returns `true` if the element was found and focused.
    pub fn set_focus(&mut self, id: &str) -> bool {
        if let Some(index) = self.focus_order.iter().position(|s| s == id) {
            self.focus_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Clear focus (no element is focused).
    pub fn clear_focus(&mut self) {
        self.focus_index = None;
    }

    /// Move focus to the next element.
    ///
    /// Returns `true` if focus moved, `false` if there are no focusable elements.
    pub fn focus_next(&mut self) -> bool {
        if self.focus_order.is_empty() {
            return false;
        }

        let new_index = match self.focus_index {
            Some(i) => (i + 1) % self.focus_order.len(),
            None => 0,
        };

        self.focus_index = Some(new_index);
        true
    }

    /// Move focus to the previous element.
    ///
    /// Returns `true` if focus moved, `false` if there are no focusable elements.
    pub fn focus_prev(&mut self) -> bool {
        if self.focus_order.is_empty() {
            return false;
        }

        let len = self.focus_order.len();
        let new_index = match self.focus_index {
            Some(i) => (i + len - 1) % len,
            None => len - 1,
        };

        self.focus_index = Some(new_index);
        true
    }

    /// Register a focusable element.
    ///
    /// Elements are focused in registration order unless an explicit
    /// order is provided. If the element is already registered, this
    /// does nothing.
    pub fn register(&mut self, id: &str) {
        if !self.focus_order.iter().any(|s| s == id) {
            self.focus_order.push(id.to_string());
        }
    }

    /// Register a focusable element at a specific position.
    ///
    /// If `order` is `None`, appends to the end.
    /// If the element is already registered, this does nothing.
    pub fn register_at(&mut self, id: &str, order: Option<usize>) {
        if self.focus_order.iter().any(|s| s == id) {
            return;
        }

        match order {
            Some(pos) if pos < self.focus_order.len() => {
                self.focus_order.insert(pos, id.to_string());
                // Adjust focus index if needed
                if let Some(ref mut focus_idx) = self.focus_index {
                    if *focus_idx >= pos {
                        *focus_idx += 1;
                    }
                }
            }
            _ => self.focus_order.push(id.to_string()),
        }
    }

    /// Unregister a focusable element.
    ///
    /// If the element was focused, focus is cleared.
    pub fn unregister(&mut self, id: &str) {
        if let Some(index) = self.focus_order.iter().position(|s| s == id) {
            self.focus_order.remove(index);

            // Adjust focus index
            if let Some(ref mut focus_idx) = self.focus_index {
                if *focus_idx == index {
                    // Currently focused element was removed
                    if self.focus_order.is_empty() {
                        self.focus_index = None;
                    } else if *focus_idx >= self.focus_order.len() {
                        *focus_idx = self.focus_order.len() - 1;
                    }
                } else if *focus_idx > index {
                    *focus_idx -= 1;
                }
            }
        }
    }

    /// Get the list of registered focusable elements in order.
    pub fn focus_order(&self) -> &[String] {
        &self.focus_order
    }

    /// Get the number of registered focusable elements.
    pub fn len(&self) -> usize {
        self.focus_order.len()
    }

    /// Check if there are no registered focusable elements.
    pub fn is_empty(&self) -> bool {
        self.focus_order.is_empty()
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_result_default() {
        assert_eq!(EventResult::default(), EventResult::Unhandled);
    }

    #[test]
    fn test_event_result_from_bool() {
        assert_eq!(EventResult::from(true), EventResult::Handled);
        assert_eq!(EventResult::from(false), EventResult::Unhandled);
    }

    #[test]
    fn test_event_result_to_bool() {
        assert!(bool::from(EventResult::Handled));
        assert!(!bool::from(EventResult::Unhandled));
        assert!(!bool::from(EventResult::StopPropagation));
    }

    #[test]
    fn test_focus_manager_navigation() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");

        // Initially no focus
        assert!(fm.focused_id().is_none());

        // Focus next goes to first
        assert!(fm.focus_next());
        assert_eq!(fm.focused_id(), Some("a"));

        // Focus next wraps
        assert!(fm.focus_next());
        assert_eq!(fm.focused_id(), Some("b"));
        assert!(fm.focus_next());
        assert_eq!(fm.focused_id(), Some("c"));
        assert!(fm.focus_next());
        assert_eq!(fm.focused_id(), Some("a"));

        // Focus prev wraps
        assert!(fm.focus_prev());
        assert_eq!(fm.focused_id(), Some("c"));
    }

    #[test]
    fn test_focus_manager_set_focus() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");

        assert!(fm.set_focus("b"));
        assert_eq!(fm.focused_id(), Some("b"));

        assert!(!fm.set_focus("unknown"));
        assert_eq!(fm.focused_id(), Some("b"));
    }

    #[test]
    fn test_focus_manager_unregister() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.set_focus("b");

        fm.unregister("a");
        assert_eq!(fm.focused_id(), Some("b"));
        assert_eq!(fm.focus_order(), &["b", "c"]);

        fm.unregister("b");
        // Focus should move to remaining element
        assert!(fm.focused_id().is_some());
    }
}
