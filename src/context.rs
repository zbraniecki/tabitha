//! Application contexts for event handlers and drawing.
//!
//! This module provides context types that are passed to event handlers
//! and draw methods, allowing components to control application behavior
//! and access shared state.

use ratatui::{layout::Rect, Frame};

use crate::focus::FocusManager;
use crate::tabs::{TabInfo, TabManager};
use crate::task::{BlockingHandle, CongestionController};
use crate::task_manager::{TaskManager, TaskManagerContext};
use crate::terminal::{Terminal, TerminalError};
use crate::theme::Theme;
use crate::widget::{Modal, ModalManager, ModalResult};

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
    #[allow(dead_code)]
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

    /// Access modal controls for event handling.
    ///
    /// Use this to open modals and check results.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::{Modal, ModalButton, ModalResult};
    ///
    /// // Open a modal
    /// ctx.modal().open(
    ///     Modal::new("confirm", "Are you sure?")
    ///         .with_title("Confirm")
    ///         .with_button(ModalButton::new("yes", "Yes"))
    ///         .with_button(ModalButton::new("no", "No"))
    /// );
    ///
    /// // Check for results from closed modals
    /// if let Some((modal_id, result)) = ctx.modal().take_result() {
    ///     match (modal_id.as_str(), result) {
    ///         ("confirm", ModalResult::ButtonPressed(id)) if id == "yes" => {
    ///             // Handle confirmation
    ///         }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn modal(&mut self) -> ModalEventContext<'_> {
        ModalEventContext {
            manager: self.modal_manager,
        }
    }

    /// Access the task manager for runtime task spawning.
    ///
    /// Returns `Some(TaskManagerContext)` if the task manager is available,
    /// `None` otherwise. The task manager is only available during event
    /// handling when the application is running.
    ///
    /// Use this to spawn, monitor, and abort background tasks dynamically
    /// at runtime.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
    ///     if let Some(mut task_ctx) = ctx.task_manager() {
    ///         // Spawn a new task
    ///         if !task_ctx.is_running("worker") {
    ///             if let Err(e) = task_ctx.spawn("worker", WorkerTask::new()) {
    ///                 tracing::error!("Failed to spawn worker: {}", e);
    ///             }
    ///         }
    ///         
    ///         // List running tasks
    ///         for name in task_ctx.list_tasks() {
    ///             tracing::info!("Running task: {}", name);
    ///         }
    ///         
    ///         // Abort a task
    ///         if task_ctx.abort("worker") {
    ///             tracing::info!("Worker aborted");
    ///         }
    ///     }
    ///     EventResult::Unhandled
    /// }
    /// ```
    #[inline]
    pub fn task_manager(&mut self) -> Option<TaskManagerContext<'_>> {
        self.task_manager
            .as_mut()
            .map(|tm| TaskManagerContext::new(tm))
    }

    /// Spawn a blocking operation on the tokio blocking thread pool.
    ///
    /// This method is useful for CPU-intensive or blocking I/O operations
    /// that would otherwise block the async runtime. The task runs on a
    /// dedicated thread pool managed by tokio.
    ///
    /// Returns a `BlockingHandle<T>` that can be used to await the result
    /// or abort the task.
    ///
    /// # Type Parameters
    ///
    /// * `F` - The function type to execute
    /// * `T` - The return type of the function
    ///
    /// # Arguments
    ///
    /// * `f` - The function to execute on the blocking thread pool
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::AppContext;
    ///
    /// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
    ///     // Spawn a CPU-intensive computation
    ///     let handle = ctx.spawn_blocking(|| {
    ///         // This runs on a dedicated thread
    ///         let mut sum = 0u64;
    ///         for i in 1..1_000_000 {
    ///             sum += i;
    ///         }
    ///         sum
    ///     });
    ///
    ///     // Store the handle to await later
    ///     self.computation = Some(handle);
    ///     
    ///     EventResult::Unhandled
    /// }
    /// ```
    pub fn spawn_blocking<F, T>(&self, f: F) -> BlockingHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        tracing::trace!("spawning blocking task from AppContext");
        let handle = tokio::task::spawn_blocking(f);
        BlockingHandle::new(handle)
    }

    /// Get a reference to the congestion controller if available.
    ///
    /// Returns `Some(&CongestionController)` if the task manager is available,
    /// `None` otherwise. Use this to check backpressure state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
    ///     if let Some(congestion) = ctx.congestion() {
    ///         if congestion.is_congested() {
    ///             tracing::warn!("System is experiencing congestion");
    ///         }
    ///     }
    ///     EventResult::Unhandled
    /// }
    /// ```
    pub fn congestion(&self) -> Option<&CongestionController> {
        self.task_manager.as_ref().map(|tm| tm.congestion())
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
