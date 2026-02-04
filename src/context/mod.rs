//! Application contexts for event handlers and drawing.
//!
//! This module provides context types that are passed to event handlers
//! and draw methods, allowing components to control application behavior
//! and access shared state.

pub mod traits;

use ratatui::{layout::Rect, Frame};

use crate::event::Event;
use crate::focus::{EventResult, FocusManager};
use crate::tabs::{TabInfo, TabManager, TabMut, TabRef};
use crate::task::{BlockingHandle, CongestionController};
use crate::task_manager::{TaskManager, TaskManagerContext};
use crate::terminal::{Terminal, TerminalError};
use crate::theme::Theme;
use crate::widget::{Modal, ModalManager, ModalResult};

// Import traits
pub use self::traits::*;

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
/// - Spawn and manage runtime tasks
/// - Spawn blocking tasks for CPU-intensive work
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
///         // Spawn a runtime task
///         if let Some(mut task_ctx) = ctx.task_manager() {
///             task_ctx.spawn("worker", MyTask).ok();
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
    pub(crate) modal_manager: &'a mut ModalManager,
    pub(crate) task_manager: Option<&'a mut TaskManager>,
    pub(crate) should_quit: bool,
}

impl<'a> AppContext<'a> {
    /// Create a new application context.
    #[allow(dead_code)]
    pub(crate) fn new(
        terminal: &'a mut Terminal,
        tab_manager: &'a mut TabManager,
        focus_manager: &'a mut FocusManager,
        modal_manager: &'a mut ModalManager,
    ) -> Self {
        Self {
            terminal,
            tab_manager,
            focus_manager,
            modal_manager,
            task_manager: None,
            should_quit: false,
        }
    }

    /// Create a new application context with task manager.
    pub(crate) fn with_task_manager(
        terminal: &'a mut Terminal,
        tab_manager: &'a mut TabManager,
        focus_manager: &'a mut FocusManager,
        modal_manager: &'a mut ModalManager,
        task_manager: &'a mut TaskManager,
    ) -> Self {
        Self {
            terminal,
            tab_manager,
            focus_manager,
            modal_manager,
            task_manager: Some(task_manager),
            should_quit: false,
        }
    }
}

impl HasFocus for AppContext<'_> {
    fn focus(&mut self) -> FocusEventContext<'_> {
        FocusEventContext {
            manager: self.focus_manager,
        }
    }
}

impl HasTabs for AppContext<'_> {
    fn tabs(&mut self) -> TabsEventContext<'_> {
        TabsEventContext {
            manager: self.tab_manager,
            focus_manager: self.focus_manager,
        }
    }
}

impl HasTerminal for AppContext<'_> {
    fn mouse_capture_enabled(&self) -> bool {
        self.terminal.mouse_capture_enabled()
    }

    fn set_mouse_capture(&mut self, enabled: bool) -> Result<(), TerminalError> {
        self.terminal.set_mouse_capture(enabled)
    }

    fn terminal_size(&self) -> Result<Rect, TerminalError> {
        self.terminal.size()
    }
}

impl CanQuit for AppContext<'_> {
    fn quit(&mut self) {
        self.should_quit = true;
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }
}

impl HasModal for AppContext<'_> {
    fn modal(&mut self) -> ModalEventContext<'_> {
        ModalEventContext {
            manager: self.modal_manager,
        }
    }
}

impl HasTaskManager for AppContext<'_> {
    fn task_manager(&mut self) -> TaskManagerContext<'_> {
        // This will panic if task_manager is None, which maintains backward compatibility
        // with existing code that called ctx.task_manager().unwrap()
        TaskManagerContext::new(
            self.task_manager
                .as_mut()
                .expect("task manager not available"),
        )
    }

    fn congestion(&self) -> Option<&CongestionController> {
        self.task_manager.as_ref().map(|tm| tm.congestion())
    }
}

impl CanSpawnBlocking for AppContext<'_> {
    fn spawn_blocking<F, T>(&self, f: F) -> BlockingHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        tracing::trace!("spawning blocking task from AppContext");
        let handle = tokio::task::spawn_blocking(f);
        BlockingHandle::new(handle)
    }
}

/// Context provided to component lifecycle hooks.
///
/// This is a minimal context containing only what lifecycle hooks need.
/// It provides access to focus management for auto-registering focus children.
pub struct LifecycleContext<'a> {
    focus_manager: &'a mut FocusManager,
}

impl<'a> LifecycleContext<'a> {
    /// Create a new lifecycle context.
    pub(crate) fn new(focus_manager: &'a mut FocusManager) -> Self {
        Self { focus_manager }
    }

    /// Access focus controls.
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

    /// Check if an element is focused or is an ancestor of the focused element.
    ///
    /// This is useful for highlighting parent containers when a child is focused.
    pub fn is_focused_or_within(&self, id: &str) -> bool {
        self.manager.is_focused_or_within(id)
    }

    /// Get the current focus path.
    ///
    /// Returns a slice of node IDs from root to the currently focused element.
    pub fn focus_path(&self) -> &[String] {
        self.manager.focus_path()
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

    /// Move focus to the next sibling element at the current level.
    ///
    /// Returns `true` if focus moved.
    pub fn next_sibling(&mut self) -> bool {
        self.manager.next_sibling()
    }

    /// Move focus to the previous sibling element at the current level.
    ///
    /// Returns `true` if focus moved.
    pub fn prev_sibling(&mut self) -> bool {
        self.manager.prev_sibling()
    }

    /// Move focus to the next element.
    ///
    /// This is an alias for `next_sibling()`.
    /// Returns `true` if focus moved.
    pub fn focus_next(&mut self) -> bool {
        self.manager.focus_next()
    }

    /// Move focus to the previous element.
    ///
    /// This is an alias for `prev_sibling()`.
    /// Returns `true` if focus moved.
    pub fn focus_prev(&mut self) -> bool {
        self.manager.focus_prev()
    }

    /// Enter the focused element's child scope.
    ///
    /// If the currently focused element has children, focuses the first child.
    /// Returns `true` if focus moved into a child.
    pub fn focus_into(&mut self) -> bool {
        self.manager.focus_into()
    }

    /// Exit the current focus scope to the parent.
    ///
    /// Returns `true` if focus moved to a parent.
    pub fn focus_out(&mut self) -> bool {
        self.manager.focus_out()
    }

    /// Register a focusable element as a root-level node.
    ///
    /// Elements are focused in registration order.
    pub fn register(&mut self, id: &str) {
        self.manager.register(id);
    }

    /// Register a child node under a parent.
    ///
    /// Creates the parent if it doesn't exist.
    pub fn register_child(&mut self, parent_id: &str, child_id: &str) {
        self.manager.register_child(parent_id, child_id);
    }

    /// Register multiple children under a parent.
    pub fn register_children(&mut self, parent_id: &str, children: &[&str]) {
        self.manager.register_children(parent_id, children);
    }

    /// Unregister a focusable element and all its children.
    pub fn unregister(&mut self, id: &str) {
        self.manager.unregister(id);
    }
}

/// Modal controls available during event handling.
///
/// Access this through `AppContext::modal()`.
pub struct ModalEventContext<'a> {
    manager: &'a mut ModalManager,
}

impl ModalEventContext<'_> {
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
    pub fn open(&mut self, modal: Modal) {
        self.manager.open(modal);
    }

    /// Close the current modal programmatically.
    ///
    /// Sets the result to `ModalResult::Closed`. Does nothing if no modal is open.
    pub fn close(&mut self) {
        self.manager.close();
    }

    /// Check if a modal is currently open.
    pub fn is_open(&self) -> bool {
        self.manager.is_open()
    }

    /// Get the ID of the currently open modal.
    pub fn current_id(&self) -> Option<&str> {
        self.manager.current_id()
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
        self.manager.take_result()
    }

    /// Peek at the result without consuming it.
    pub fn result(&self) -> Option<(&str, &ModalResult)> {
        self.manager.result()
    }

    /// Get the ID of the last closed modal.
    pub fn last_id(&self) -> Option<&str> {
        self.manager.last_id()
    }
}

/// Tab controls available during event handling.
///
/// Access this through `AppContext::tabs()`.
pub struct TabsEventContext<'a> {
    manager: &'a mut TabManager,
    focus_manager: &'a mut FocusManager,
}

impl<'a> TabsEventContext<'a> {
    /// Create a new TabsEventContext for testing.
    #[cfg(test)]
    pub(crate) fn new(manager: &'a mut TabManager, focus_manager: &'a mut FocusManager) -> Self {
        Self {
            manager,
            focus_manager,
        }
    }

    /// Get the list of all registered tabs.
    pub fn list(&self) -> Vec<TabInfo> {
        self.manager.list().into_iter().cloned().collect()
    }

    /// Get the index of the currently active tab.
    pub fn active_index(&self) -> usize {
        self.manager.active_index()
    }

    /// Get the ID of the currently active tab, if any.
    pub fn active_id(&self) -> Option<&str> {
        self.manager.active().map(|(info, _)| info.id())
    }

    /// Get a tab by ID (immutable).
    pub fn get(&self, id: &str) -> Option<TabRef<'_>> {
        self.manager.get(id)
    }

    /// Get a tab by ID (mutable).
    pub fn get_mut(&mut self, id: &str) -> Option<TabMut<'_>> {
        self.manager.get_mut(id)
    }

    /// Select a tab by index.
    ///
    /// Returns `true` if the tab was selected, `false` if the index is invalid
    /// or the tab is disabled.
    /// Automatically calls `on_unmount` on the old tab and `on_mount` on the new tab.
    pub fn select(&mut self, index: usize) -> bool {
        let old_index = self.manager.active_index();

        // Try to select the tab
        if self.manager.select(index).is_none() {
            return false;
        }

        // Create lifecycle context
        let mut ctx = LifecycleContext::new(self.focus_manager);

        // Call on_unmount on old tab (only if we're actually switching tabs)
        if old_index != index {
            if let Some(comp) = self.manager.get_component_mut(old_index) {
                comp.on_unmount(&mut ctx);
            }
        }

        // Call on_mount on new tab
        if let Some(comp) = self.manager.active_component_mut() {
            comp.on_mount(&mut ctx);
        }

        true
    }

    /// Select a tab by its unique ID.
    ///
    /// Returns `true` if the tab was found and selected.
    /// Automatically calls `on_unmount` on the old tab and `on_mount` on the new tab.
    pub fn select_by_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.manager.index_by_id(id) {
            self.select(index)
        } else {
            false
        }
    }

    /// Select the next enabled tab.
    ///
    /// Wraps around to the first tab if at the end.
    /// Automatically calls `on_unmount` on the old tab and `on_mount` on the new tab.
    pub fn select_next(&mut self) -> bool {
        let old_index = self.manager.active_index();
        let len = self.manager.len();
        if len == 0 {
            return false;
        }

        let mut index = (old_index + 1) % len;
        let start = index;

        loop {
            if self.manager.is_enabled(index) {
                return self.select(index);
            }
            index = (index + 1) % len;
            if index == start {
                return false;
            }
        }
    }

    /// Select the previous enabled tab.
    ///
    /// Wraps around to the last tab if at the beginning.
    /// Automatically calls `on_unmount` on the old tab and `on_mount` on the new tab.
    pub fn select_prev(&mut self) -> bool {
        let old_index = self.manager.active_index();
        let len = self.manager.len();
        if len == 0 {
            return false;
        }

        let mut index = (old_index + len - 1) % len;
        let start = index;

        loop {
            if self.manager.is_enabled(index) {
                return self.select(index);
            }
            index = (index + len - 1) % len;
            if index == start {
                return false;
            }
        }
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
        self.manager.is_enabled_by_id(id)
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

    /// Select a tab by index (0-based).
    ///
    /// Returns `true` if the tab was selected, `false` if the index is invalid
    /// or the tab is disabled.
    /// Automatically calls `on_unmount` on the old tab and `on_mount` on the new tab.
    pub fn select_by_index(&mut self, index: usize) -> bool {
        self.select(index)
    }

    /// Forward an event to the active tab.
    ///
    /// This allows the TabContent widget to delegate events to the active tab.
    /// Returns `EventResult::Unhandled` if there is no active tab.
    pub fn forward_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        self.manager.handle_event(event, ctx)
    }
}

/// Context passed to draw methods for rendering.
///
/// `DrawContext` provides access to:
/// - Theme colors and styles
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
///         // Use theme colors for consistent styling
///         let style = if ctx.focus().is_focused("my_widget") {
///             ctx.theme().highlight_style()
///         } else {
///             ctx.theme().style()
///         };
///         // Draw with style...
///     }
/// }
/// ```
pub struct DrawContext<'a> {
    pub(crate) tab_manager: &'a TabManager,
    pub(crate) focus_manager: &'a FocusManager,
    pub(crate) theme: &'a Theme,
}

impl<'a> DrawContext<'a> {
    /// Create a new draw context.
    #[allow(dead_code)]
    pub(crate) fn new(
        tab_manager: &'a TabManager,
        focus_manager: &'a FocusManager,
        theme: &'a Theme,
    ) -> Self {
        Self {
            tab_manager,
            focus_manager,
            theme,
        }
    }

    /// Access the current theme for styling.
    ///
    /// The theme provides semantic color roles that components can use
    /// for consistent styling across the application.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Get a style for highlighted elements
    /// let style = ctx.theme().highlight_style();
    ///
    /// // Get individual colors
    /// let accent = ctx.theme().accent;
    /// ```
    #[inline]
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Access tab information and drawing methods.
    #[inline]
    pub fn tabs(&self) -> TabsDrawContext<'_> {
        TabsDrawContext::new(self.tab_manager)
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

impl<'a> TabsDrawContext<'a> {
    pub(crate) fn new(manager: &'a TabManager) -> Self {
        Self { manager }
    }

    /// Get the list of all registered tabs.
    pub fn list(&self) -> Vec<&'a TabInfo> {
        self.manager.list()
    }

    /// Get the index of the currently active tab.
    pub fn active_index(&self) -> usize {
        self.manager.active_index()
    }

    /// Get the ID of the currently active tab, if any.
    pub fn active_id(&self) -> Option<&'a str> {
        self.manager.active().map(|(info, _)| info.id())
    }

    /// Check if there are any registered tabs.
    pub fn is_empty(&self) -> bool {
        self.manager.is_empty()
    }

    /// Get the number of registered tabs.
    pub fn len(&self) -> usize {
        self.manager.len()
    }

    /// Draw the content of the currently active tab.
    ///
    /// This calls the active tab's `draw` method with the given area.
    pub fn draw_content(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        self.manager.draw_active(frame, area, ctx);
    }

    /// Iterate over tab metadata.
    ///
    /// Returns an iterator over `TabInfo` for all registered tabs.
    pub fn iter(&self) -> impl Iterator<Item = &'a TabInfo> + '_ {
        self.manager.list().into_iter()
    }

    /// Draw the active tab's content.
    ///
    /// This is an alias for `draw_content` for API consistency with the new widget pattern.
    pub fn draw_active(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        self.manager.draw_active(frame, area, ctx);
    }
}
