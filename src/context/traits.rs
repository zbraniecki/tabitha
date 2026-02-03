//! Context traits for component interaction.
//!
//! This module defines traits that provide access to different capabilities
//! through context types. Components can use these traits to express their
//! requirements declaratively.

use ratatui::layout::Rect;

use crate::task::{BlockingHandle, CongestionController};
use crate::terminal::TerminalError;

use super::{FocusEventContext, ModalEventContext, TabsEventContext, TaskManagerContext};

/// Trait for types that provide access to focus controls.
///
/// This trait allows components to navigate and control focus
/// within the application.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode, HasFocus};
///
/// impl<T: HasFocus> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_key(KeyCode::Tab) {
///             ctx.focus().focus_next();
///             return EventResult::Handled;
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait HasFocus {
    /// Access focus controls for event handling.
    fn focus(&mut self) -> FocusEventContext<'_>;
}

/// Trait for types that provide access to tab controls.
///
/// This trait allows components to navigate and control tabs
/// within the application.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode, HasTabs};
///
/// impl<T: HasTabs> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_key(KeyCode::Tab) {
///             ctx.tabs().select_next();
///             return EventResult::Handled;
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait HasTabs {
    /// Access tab controls for event handling.
    fn tabs(&mut self) -> TabsEventContext<'_>;
}

/// Trait for types that provide access to terminal controls.
///
/// This trait allows components to query and control terminal state.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode, HasTerminal};
///
/// impl<T: HasTerminal> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_key(KeyCode::Char('m')) {
///             let enabled = !ctx.mouse_capture_enabled();
///             ctx.set_mouse_capture(enabled).ok();
///             return EventResult::Handled;
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait HasTerminal {
    /// Check if mouse capture is currently enabled.
    fn mouse_capture_enabled(&self) -> bool;

    /// Enable or disable mouse capture at runtime.
    fn set_mouse_capture(&mut self, enabled: bool) -> Result<(), TerminalError>;

    /// Get the terminal size.
    fn terminal_size(&self) -> Result<Rect, TerminalError>;
}

/// Trait for types that can request application quit.
///
/// This trait allows components to signal that the application
/// should exit gracefully.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, CanQuit};
///
/// impl<T: CanQuit> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_quit() {
///             ctx.quit();
///             return EventResult::Handled;
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait CanQuit {
    /// Request the application to quit.
    fn quit(&mut self);

    /// Check if quit has been requested.
    fn should_quit(&self) -> bool;
}

/// Trait for types that provide access to modal controls.
///
/// This trait allows components to open and manage modal dialogs.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, KeyCode, HasModal, Modal, ModalButton};
///
/// impl<T: HasModal> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_key(KeyCode::Char('?')) {
///             ctx.modal().open(
///                 Modal::new("help", "Press q to quit")
///                     .with_title("Help")
///                     .with_button(ModalButton::new("ok", "OK"))
///             );
///             return EventResult::Handled;
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait HasModal {
    /// Access modal controls for event handling.
    fn modal(&mut self) -> ModalEventContext<'_>;
}

/// Trait for types that provide access to the task manager.
///
/// This trait allows components to spawn and manage background tasks.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, HasTaskManager};
///
/// impl<T: HasTaskManager> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if let Some(mut task_ctx) = ctx.task_manager() {
///             task_ctx.spawn("worker", MyTask).ok();
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait HasTaskManager {
    /// Access the task manager for runtime task spawning.
    fn task_manager(&mut self) -> TaskManagerContext<'_>;

    /// Get a reference to the congestion controller if available.
    fn congestion(&self) -> Option<&CongestionController>;
}

/// Trait for types that can spawn blocking tasks.
///
/// This trait allows components to spawn CPU-intensive or blocking I/O
/// operations on a dedicated thread pool.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, EventResult, CanSpawnBlocking};
///
/// impl<T: CanSpawnBlocking> Component for MyWidget {
///     fn handle_event(&mut self, event: &Event, ctx: &mut T) -> EventResult {
///         if event.is_key(KeyCode::Char('c')) {
///             let handle = ctx.spawn_blocking(|| {
///                 // CPU-intensive computation
///                 42
///             });
///             // Store handle for later...
///             EventResult::Handled
///         }
///         EventResult::Unhandled
///     }
/// }
/// ```
pub trait CanSpawnBlocking {
    /// Spawn a blocking operation on the tokio blocking thread pool.
    fn spawn_blocking<F, T>(&self, f: F) -> BlockingHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
}
