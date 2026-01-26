//! Interactive widget controls for the TUI framework.
//!
//! This module provides the `Control` trait for building interactive,
//! stateful widgets like text boxes, select lists, trees, etc.
//!
//! # Design
//!
//! Controls differ from Components in that they are:
//! - Smaller, reusable building blocks
//! - Focused on user input and interaction
//! - Owned by Components and drawn/updated by them
//! - Able to emit events that parent Components can handle
//!
//! # Example
//!
//! ```ignore
//! use tabitha::widget::{Control, TextBox, TextBoxEvent};
//! use tabitha::{Component, DrawContext, AppContext, Event, EventResult};
//!
//! struct MyForm {
//!     username: TextBox,
//!     password: TextBox,
//! }
//!
//! impl Component for MyForm {
//!     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
//!         // Draw text boxes
//!         self.username.draw(frame, username_area, ctx.focus().is_focused("username"));
//!         self.password.draw(frame, password_area, ctx.focus().is_focused("password"));
//!     }
//!
//!     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
//!         // Forward to focused control
//!         if ctx.focus().is_focused("username") {
//!             let result = self.username.handle_event(event);
//!             // Check for emitted events
//!             for evt in self.username.take_events() {
//!                 match evt {
//!                     TextBoxEvent::Submit(text) => { /* handle submit */ }
//!                     _ => {}
//!                 }
//!             }
//!             return result;
//!         }
//!         EventResult::Unhandled
//!     }
//! }
//! ```

mod datatable;
mod modal;
mod textbox;

pub use datatable::{
    Column, ColumnAlign, ColumnType, ColumnWidth, DataTable, DataTableConfig, DataTableEvent,
    SelectionMode, SimpleRow, SortDirection, SortState, TableRow,
};
pub use modal::{Modal, ModalButton, ModalConfig, ModalInput, ModalManager, ModalResult};
pub use textbox::{TextBox, TextBoxBuilder, TextBoxConfig, TextBoxEvent};

use ratatui::{layout::Rect, Frame};

use crate::event::Event;
use crate::focus::EventResult;

// =============================================================================
// Control Trait
// =============================================================================

/// A marker trait for control-specific events.
///
/// Each control type defines its own event enum that implements this trait.
/// This allows type-safe event handling while providing a common interface.
pub trait ControlEvent: Send + 'static {}

/// An interactive widget control that can handle input and emit events.
///
/// Controls are the building blocks for interactive UI elements like
/// text inputs, select boxes, tables with selection, etc.
///
/// Unlike Components, Controls:
/// - Are owned and managed by a parent Component
/// - Don't directly access AppContext (they receive events from parent)
/// - Emit events that the parent can handle
/// - Have simpler lifecycle (no direct focus registration)
///
/// # Focus
///
/// Controls don't manage their own focus registration. Instead, the parent
/// Component registers focus IDs and forwards events to the appropriate
/// control based on current focus state.
///
/// # Events
///
/// Controls can emit events (like "text changed", "item selected", etc.)
/// that the parent Component can poll and handle. Use `take_events()` to
/// drain pending events after calling `handle_event()`.
pub trait Control: Send {
    /// The type of events this control can emit.
    type Event: ControlEvent;

    /// Draw the control to the given frame area.
    ///
    /// The `focused` parameter indicates whether this control currently
    /// has focus, allowing for visual differentiation (e.g., highlighted
    /// border, visible cursor).
    fn draw(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// Handle an input event.
    ///
    /// Returns `EventResult::Handled` if the event was consumed,
    /// `EventResult::Unhandled` otherwise.
    ///
    /// Events emitted by the control can be retrieved via `take_events()`.
    fn handle_event(&mut self, event: &Event) -> EventResult;

    /// Called on each tick cycle for animations (e.g., cursor blinking).
    ///
    /// Returns `true` if the control's visual state changed and a redraw
    /// is needed.
    fn tick(&mut self) -> bool {
        false
    }

    /// Take all pending events from the control.
    ///
    /// This drains the internal event queue. Call this after `handle_event()`
    /// to process any events the control emitted.
    fn take_events(&mut self) -> Vec<Self::Event>;

    /// Check if there are any pending events.
    fn has_events(&self) -> bool;

    /// Called when the control gains focus.
    ///
    /// Use this to reset cursor blink state, show cursor, etc.
    fn on_focus(&mut self) {}

    /// Called when the control loses focus.
    ///
    /// Use this to hide cursor, finalize input, etc.
    fn on_blur(&mut self) {}
}

/// Configuration for cursor blinking behavior.
#[derive(Debug, Clone)]
pub struct CursorBlinkConfig {
    /// Whether cursor blinking is enabled.
    pub enabled: bool,
    /// Duration the cursor is visible (in milliseconds).
    pub on_duration_ms: u64,
    /// Duration the cursor is hidden (in milliseconds).
    pub off_duration_ms: u64,
}

impl Default for CursorBlinkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_duration_ms: 530,
            off_duration_ms: 530,
        }
    }
}

impl CursorBlinkConfig {
    /// Create a config with blinking enabled (the default).
    pub fn enabled() -> Self {
        Self::default()
    }

    /// Create a config with blinking disabled.
    pub fn no_blink() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config with custom blink timing.
    pub fn with_timing(on_ms: u64, off_ms: u64) -> Self {
        Self {
            enabled: true,
            on_duration_ms: on_ms,
            off_duration_ms: off_ms,
        }
    }

    /// Total blink cycle duration.
    pub fn cycle_duration_ms(&self) -> u64 {
        self.on_duration_ms + self.off_duration_ms
    }
}

// =============================================================================
// Control Extension Trait
// =============================================================================

/// A type-erased control that can be stored in collections.
///
/// This is useful when you need to store multiple controls of different
/// concrete types in the same collection.
pub type BoxedControl<E> = Box<dyn Control<Event = E>>;

/// Extension trait for controls providing convenience methods.
pub trait ControlExt: Control + Sized + 'static {
    /// Box this control for type-erased storage.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::widget::{TextBox, ControlExt, BoxedControl, TextBoxEvent};
    ///
    /// let textbox = TextBox::new("my_input");
    /// let boxed: BoxedControl<TextBoxEvent> = textbox.boxed();
    /// ```
    fn boxed(self) -> BoxedControl<Self::Event> {
        Box::new(self)
    }
}

impl<T: Control + 'static> ControlExt for T {}
