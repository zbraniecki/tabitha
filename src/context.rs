//! Application contexts for event handlers and drawing.
//!
//! This module provides context types that are passed to event handlers
//! and draw methods, allowing components to control application behavior
//! and access shared state.

use ratatui::{layout::Rect, Frame};

use crate::focus::FocusManager;
use crate::tabs::{TabInfo, TabManager};
use crate::terminal::{Terminal, TerminalError};

// =============================================================================
// TabEventContext - Context for Tab event handlers (no TabManager access)
// =============================================================================

/// Context passed to Tab event handlers.
///
/// This is a subset of `AppContext` that doesn't include tab management,
/// allowing tabs to be called without circular borrow issues.
///
/// `TabEventContext` provides methods to:
/// - Request application quit
/// - Toggle mouse capture
/// - Access terminal state
/// - Navigate focus
pub struct TabEventContext<'a> {
    pub(crate) terminal: &'a mut Terminal,
    pub(crate) focus_manager: &'a mut FocusManager,
    pub(crate) should_quit: bool,
}

impl<'a> TabEventContext<'a> {
    /// Create a new tab event context.
    pub(crate) fn new(terminal: &'a mut Terminal, focus_manager: &'a mut FocusManager) -> Self {
        Self {
            terminal,
            focus_manager,
            should_quit: false,
        }
    }

    /// Request the application to quit.
    #[inline]
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Check if quit has been requested.
    #[inline]
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check if mouse capture is currently enabled.
    #[inline]
    pub fn mouse_capture_enabled(&self) -> bool {
        self.terminal.mouse_capture_enabled()
    }

    /// Enable or disable mouse capture at runtime.
    pub fn set_mouse_capture(&mut self, enabled: bool) -> Result<(), TerminalError> {
        self.terminal.set_mouse_capture(enabled)
    }

    /// Get the terminal size.
    pub fn terminal_size(&self) -> Result<Rect, TerminalError> {
        self.terminal.size()
    }

    /// Access focus controls for event handling.
    #[inline]
    pub fn focus(&mut self) -> FocusEventContext<'_> {
        FocusEventContext {
            manager: self.focus_manager,
        }
    }
}

// =============================================================================
// AppContext - Full context for MainUi and Component handlers
// =============================================================================

/// Context passed to event handlers for controlling the application.
///
/// `AppContext` provides methods to:
/// - Request application quit
/// - Toggle mouse capture
/// - Access terminal state
/// - Control tab selection
/// - Navigate focus
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode};
///
/// impl Component for MyApp {
///     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///         if event.is_quit() {
///             ctx.quit();
///             return EventResult::Handled;
///         }
///
///         // Navigate focus with Tab key
///         if event.is_key(KeyCode::Tab) {
///             ctx.focus().focus_next();
///             return EventResult::Handled;
///         }
///
///         EventResult::Unhandled
///     }
/// }
/// ```
pub struct AppContext<'a> {
    pub(crate) terminal: &'a mut Terminal,
    pub(crate) tab_manager: &'a mut TabManager,
    pub(crate) focus_manager: &'a mut FocusManager,
    pub(crate) should_quit: bool,
}

impl<'a> AppContext<'a> {
    /// Create a new application context.
    pub(crate) fn new(
        terminal: &'a mut Terminal,
        tab_manager: &'a mut TabManager,
        focus_manager: &'a mut FocusManager,
    ) -> Self {
        Self {
            terminal,
            tab_manager,
            focus_manager,
            should_quit: false,
        }
    }

    /// Request the application to quit.
    ///
    /// The application will exit gracefully after the current event
    /// is processed.
    #[inline]
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Check if quit has been requested.
    #[inline]
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Check if mouse capture is currently enabled.
    #[inline]
    pub fn mouse_capture_enabled(&self) -> bool {
        self.terminal.mouse_capture_enabled()
    }

    /// Enable or disable mouse capture at runtime.
    ///
    /// Returns an error if the terminal operation fails.
    pub fn set_mouse_capture(&mut self, enabled: bool) -> Result<(), TerminalError> {
        self.terminal.set_mouse_capture(enabled)
    }

    /// Get the terminal size.
    pub fn terminal_size(&self) -> Result<Rect, TerminalError> {
        self.terminal.size()
    }

    /// Access tab controls for event handling.
    ///
    /// Use this to select tabs, navigate between tabs, etc.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Select next tab
    /// ctx.tabs().select_next();
    ///
    /// // Select a specific tab by ID
    /// ctx.tabs().select_by_id("settings");
    ///
    /// // Select previous tab
    /// ctx.tabs().select_prev();
    /// ```
    #[inline]
    pub fn tabs(&mut self) -> TabsEventContext<'_> {
        TabsEventContext {
            manager: self.tab_manager,
        }
    }

    /// Access focus controls for event handling.
    ///
    /// Use this to navigate focus, check focused state, etc.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Move focus to next element
    /// ctx.focus().focus_next();
    ///
    /// // Set focus to a specific element
    /// ctx.focus().set_focus("my_widget");
    ///
    /// // Check what's focused
    /// if let Some(id) = ctx.focus().focused_id() {
    ///     println!("Focused: {}", id);
    /// }
    /// ```
    #[inline]
    pub fn focus(&mut self) -> FocusEventContext<'_> {
        FocusEventContext {
            manager: self.focus_manager,
        }
    }
}

/// Focus controls available during event handling.
///
/// Access this through `AppContext::focus()`.
pub struct FocusEventContext<'a> {
    manager: &'a mut FocusManager,
}

impl FocusEventContext<'_> {
    /// Get the ID of the currently focused element.
    pub fn focused_id(&self) -> Option<&str> {
        self.manager.focused_id()
    }

    /// Check if a specific element is currently focused.
    pub fn is_focused(&self, id: &str) -> bool {
        self.manager.is_focused(id)
    }

    /// Set focus to a specific element by ID.
    ///
    /// Returns `true` if the element was found and focused.
    pub fn set_focus(&mut self, id: &str) -> bool {
        self.manager.set_focus(id)
    }

    /// Clear focus (no element is focused).
    pub fn clear_focus(&mut self) {
        self.manager.clear_focus();
    }

    /// Move focus to the next element.
    ///
    /// Returns `true` if focus moved.
    pub fn focus_next(&mut self) -> bool {
        self.manager.focus_next()
    }

    /// Move focus to the previous element.
    ///
    /// Returns `true` if focus moved.
    pub fn focus_prev(&mut self) -> bool {
        self.manager.focus_prev()
    }

    /// Register a focusable element.
    ///
    /// Elements are focused in registration order.
    pub fn register(&mut self, id: &str) {
        self.manager.register(id);
    }

    /// Unregister a focusable element.
    pub fn unregister(&mut self, id: &str) {
        self.manager.unregister(id);
    }
}

/// Tab controls available during event handling.
///
/// Access this through `AppContext::tabs()`.
pub struct TabsEventContext<'a> {
    manager: &'a mut TabManager,
}

impl TabsEventContext<'_> {
    /// Get the list of all registered tabs.
    pub fn list(&self) -> Vec<TabInfo> {
        self.manager.list()
    }

    /// Get the index of the currently active tab.
    pub fn active_index(&self) -> usize {
        self.manager.active_index()
    }

    /// Get the ID of the currently active tab, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.manager.active_tab().map(|t| t.id())
    }

    /// Select a tab by index.
    ///
    /// Returns `true` if the tab was selected, `false` if the index is invalid
    /// or the tab is disabled.
    pub fn select(&mut self, index: usize) -> bool {
        self.manager.select(index)
    }

    /// Select a tab by its unique ID.
    ///
    /// Returns `true` if the tab was found and selected.
    pub fn select_by_id(&mut self, id: &str) -> bool {
        self.manager.select_by_id(id)
    }

    /// Select the next enabled tab.
    ///
    /// Wraps around to the first tab if at the end.
    pub fn select_next(&mut self) -> bool {
        self.manager.select_next()
    }

    /// Select the previous enabled tab.
    ///
    /// Wraps around to the last tab if at the beginning.
    pub fn select_prev(&mut self) -> bool {
        self.manager.select_prev()
    }

    /// Check if there are any registered tabs.
    pub fn is_empty(&self) -> bool {
        self.manager.is_empty()
    }

    /// Get the number of registered tabs.
    pub fn len(&self) -> usize {
        self.manager.len()
    }

    /// Check if a tab is enabled.
    ///
    /// A tab is enabled if both:
    /// - The tab's own `is_enabled()` returns true
    /// - The tab has not been disabled via `set_enabled(id, false)`
    pub fn is_enabled(&self, id: &str) -> bool {
        self.manager.is_enabled(id)
    }

    /// Enable or disable a tab by ID.
    ///
    /// When `enabled` is `false`, the tab is disabled and cannot be selected.
    /// When `enabled` is `true`, the tab reverts to its own `is_enabled()` state.
    ///
    /// Returns `true` if the tab was found.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Disable the settings tab
    /// ctx.tabs().set_enabled("settings", false);
    ///
    /// // Re-enable it
    /// ctx.tabs().set_enabled("settings", true);
    ///
    /// // Toggle
    /// let enabled = ctx.tabs().is_enabled("settings");
    /// ctx.tabs().set_enabled("settings", !enabled);
    /// ```
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        self.manager.set_enabled(id, enabled)
    }
}

/// Context passed to draw methods for rendering.
///
/// `DrawContext` provides access to:
/// - Tab bar and content drawing
/// - Tab information
/// - Focus state (for visual highlighting)
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, DrawContext};
/// use ratatui::{Frame, layout::Rect, style::{Color, Style}};
///
/// impl Component for MyWidget {
///     fn focus_id(&self) -> Option<&str> {
///         Some("my_widget")
///     }
///
///     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
///         // Highlight when focused
///         let style = if ctx.focus().is_focused("my_widget") {
///             Style::default().fg(Color::Yellow)
///         } else {
///             Style::default()
///         };
///         // Draw with style...
///     }
/// }
/// ```
pub struct DrawContext<'a> {
    pub(crate) tab_manager: &'a TabManager,
    pub(crate) focus_manager: &'a FocusManager,
}

impl<'a> DrawContext<'a> {
    /// Create a new draw context.
    pub(crate) fn new(tab_manager: &'a TabManager, focus_manager: &'a FocusManager) -> Self {
        Self {
            tab_manager,
            focus_manager,
        }
    }

    /// Access tab information and drawing methods.
    #[inline]
    pub fn tabs(&self) -> TabsDrawContext<'_> {
        TabsDrawContext {
            manager: self.tab_manager,
        }
    }

    /// Access focus state for visual rendering.
    ///
    /// Use this to check if elements are focused for highlighting.
    #[inline]
    pub fn focus(&self) -> FocusDrawContext<'_> {
        FocusDrawContext {
            manager: self.focus_manager,
        }
    }
}

/// Focus drawing context available during rendering.
///
/// Access this through `DrawContext::focus()`.
pub struct FocusDrawContext<'a> {
    manager: &'a FocusManager,
}

impl FocusDrawContext<'_> {
    /// Get the ID of the currently focused element.
    pub fn focused_id(&self) -> Option<&str> {
        self.manager.focused_id()
    }

    /// Check if a specific element is currently focused.
    ///
    /// Use this to apply visual highlighting to focused elements.
    pub fn is_focused(&self, id: &str) -> bool {
        self.manager.is_focused(id)
    }

    /// Check if a specific element is in the focus chain.
    ///
    /// For flat focus, this is the same as `is_focused`.
    pub fn is_in_focus_chain(&self, id: &str) -> bool {
        self.manager.is_in_focus_chain(id)
    }
}

/// Tab drawing context available during rendering.
///
/// Access this through `DrawContext::tabs()`.
pub struct TabsDrawContext<'a> {
    manager: &'a TabManager,
}

impl TabsDrawContext<'_> {
    /// Get the list of all registered tabs.
    pub fn list(&self) -> Vec<TabInfo> {
        self.manager.list()
    }

    /// Get the index of the currently active tab.
    pub fn active_index(&self) -> usize {
        self.manager.active_index()
    }

    /// Get the ID of the currently active tab, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.manager.active_tab().map(|t| t.id())
    }

    /// Check if there are any registered tabs.
    pub fn is_empty(&self) -> bool {
        self.manager.is_empty()
    }

    /// Get the number of registered tabs.
    pub fn len(&self) -> usize {
        self.manager.len()
    }

    /// Draw the tab bar to the given area.
    ///
    /// This renders a horizontal tab bar showing all registered tabs,
    /// with the active tab highlighted.
    pub fn draw_tabbar(&self, frame: &mut Frame, area: Rect) {
        self.manager.draw_tabbar(frame, area);
    }

    /// Draw the content of the currently active tab.
    ///
    /// This calls the active tab's `draw` method with the given area.
    pub fn draw_content(&self, frame: &mut Frame, area: Rect) {
        self.manager.draw_content(frame, area);
    }
}
