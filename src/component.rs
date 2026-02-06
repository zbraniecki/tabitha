//! Component traits for the TUI framework.
//!
//! This module defines the core traits for UI components.

use ratatui::{layout::Rect, Frame};

use crate::context::{AppContext, DrawContext, LifecycleContext};
use crate::event::Event;
use crate::focus::EventResult;

/// A UI component that can draw itself and handle events.
///
/// Components are the building blocks of your TUI application.
/// They can be composed together to create complex interfaces.
///
/// # Focus Model
///
/// Components can participate in focus navigation by implementing:
/// - `focus_id()` - Return a unique ID to make this component focusable
/// - `is_focusable()` - Whether this component can currently receive focus
/// - `on_focus()` / `on_blur()` - Lifecycle callbacks for focus changes
/// - `focus_children()` - Child focus IDs for hierarchical focus
///
/// **Important**: `handle_event` is only called on components in the focus chain.
/// You don't need to check if you're focused - if you're called, you are.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, Event, AppContext, DrawContext, EventResult, KeyCode};
/// use ratatui::{Frame, layout::Rect, widgets::Paragraph};
///
/// struct Counter {
///     count: u32,
/// }
///
/// impl Component for Counter {
///     fn focus_id(&self) -> Option<&str> {
///         Some("counter")
///     }
///
///     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
///         let text = format!("Count: {}", self.count);
///         frame.render_widget(Paragraph::new(text), area);
///     }
///
///     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///         if event.is_key(KeyCode::Up) {
///             self.count = self.count.saturating_add(1);
///             EventResult::Handled
///         } else if event.is_key(KeyCode::Down) {
///             self.count = self.count.saturating_sub(1);
///             EventResult::Handled
///         } else if event.is_quit() {
///             ctx.quit();
///             EventResult::Handled
///         } else {
///             EventResult::Unhandled
///         }
///     }
/// }
/// ```
pub trait Component: Send {
    /// Draw the component to the given frame area.
    ///
    /// This method should render the component's current state to the terminal.
    /// The `area` parameter defines the rectangular region where the component
    /// should draw itself.
    ///
    /// The `ctx` parameter provides access to tabs, focus state, and other
    /// drawing utilities.
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext);

    /// Handle an input event.
    ///
    /// **Note**: This method is only called if this component is in the focus chain.
    /// You don't need to check if you're focused.
    ///
    /// The `ctx` parameter provides access to application-level controls
    /// like quitting the app, toggling mouse capture, or navigating tabs/focus.
    ///
    /// Returns:
    /// - `EventResult::Handled` - Event consumed, stop propagation
    /// - `EventResult::Unhandled` - Bubble to parent component
    /// - `EventResult::StopPropagation` - Stop propagation without handling
    ///
    /// The default implementation does nothing and returns `Unhandled`.
    #[allow(unused_variables)]
    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        EventResult::Unhandled
    }

    /// Called on each tick cycle if the app has a tick rate configured.
    ///
    /// The `ctx` parameter provides access to application-level controls.
    ///
    /// Use this for periodic updates like animations or polling.
    /// Returns `true` if a redraw is needed. When this returns `false`
    /// consistently and no framework animations are active, the framework
    /// will stop generating tick events until the next input event arrives.
    /// The default implementation does nothing and returns `false`.
    #[allow(unused_variables)]
    fn tick(&mut self, ctx: &mut AppContext) -> bool {
        false
    }

    // --- Focus methods ---

    /// Unique identifier for focus tracking.
    ///
    /// Return `Some("id")` to make this component focusable.
    /// Return `None` (default) if this component should not receive focus.
    fn focus_id(&self) -> Option<&str> {
        None
    }

    /// Whether this component can currently receive focus.
    ///
    /// By default, returns `true` if `focus_id()` returns `Some`.
    /// Override to dynamically enable/disable focus (e.g., for disabled widgets).
    fn is_focusable(&self) -> bool {
        self.focus_id().is_some()
    }

    /// Called when this component gains focus.
    ///
    /// Use this to update visual state, start animations, etc.
    fn on_focus(&mut self) {}

    /// Called when this component loses focus.
    ///
    /// Use this to update visual state, stop animations, etc.
    fn on_blur(&mut self) {}

    /// List of focusable child IDs in navigation order.
    ///
    /// Override this if your component contains other focusable components
    /// and you want to control their navigation order. The framework will
    /// automatically register these IDs when the component is mounted.
    ///
    /// The default returns an empty list (no focusable children).
    fn focus_children(&self) -> &[&str] {
        &[]
    }

    /// Called when this component is mounted to the UI.
    ///
    /// Override this to perform initialization, register focusable elements,
    /// or set up resources. The default implementation automatically registers
    /// all IDs returned by `focus_children()` with the focus manager.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn on_mount(&mut self, ctx: &mut LifecycleContext) {
    ///     // Register focus children (auto-called by default)
    ///     for id in self.focus_children() {
    ///         ctx.focus().register(id);
    ///     }
    ///
    ///     // Custom initialization
    ///     self.load_data();
    /// }
    /// ```
    fn on_mount(&mut self, ctx: &mut LifecycleContext) {
        // Auto-register focusable children
        let children = self.focus_children();
        if let Some(first) = children.first() {
            ctx.focus().register(first);
            for child in &children[1..] {
                ctx.focus().register_child(first, child);
            }
        }
    }

    /// Called when this component is unmounted from the UI.
    ///
    /// Override this to perform cleanup, unregister focusable elements,
    /// or release resources. The default implementation automatically
    /// unregisters all IDs returned by `focus_children()`.
    fn on_unmount(&mut self, ctx: &mut LifecycleContext) {
        // Auto-unregister focusable children
        for id in self.focus_children() {
            ctx.focus().unregister(id);
        }
    }
}

/// The main UI trait for the root component of your application.
///
/// This trait extends `Component` with additional methods for handling
/// background task messages.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Component, MainUi, Event, AppContext, DrawContext, EventResult, KeyCode};
/// use ratatui::{Frame, layout::{Rect, Layout, Direction, Constraint}};
///
/// struct MyApp;
///
/// impl Component for MyApp {
///     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
///         // Split area for tabs
///         let chunks = Layout::default()
///             .direction(Direction::Vertical)
///             .constraints([Constraint::Length(2), Constraint::Min(0)])
///             .split(area);
///
///         // Draw tab bar and content
///         ctx.tabs().draw_tabbar(frame, chunks[0]);
///         ctx.tabs().draw_content(frame, chunks[1]);
///     }
///
///     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///         if event.is_quit() {
///             ctx.quit();
///             return EventResult::Handled;
///         }
///
///         // Navigate tabs with Tab key
///         if event.is_key(KeyCode::Tab) {
///             ctx.tabs().select_next();
///             return EventResult::Handled;
///         }
///
///         EventResult::Unhandled
///     }
/// }
///
/// impl MainUi for MyApp {}
/// ```
pub trait MainUi: Component {
    /// Handle a message from a background task.
    ///
    /// Override this method to process messages from your background tasks.
    /// The `task_name` identifies which task sent the message.
    /// The `ctx` parameter provides access to application-level controls.
    ///
    /// Returns `true` if a redraw is needed after processing the message.
    #[allow(unused_variables)]
    fn handle_task_message(
        &mut self,
        task_name: &str,
        message: Box<dyn std::any::Any + Send>,
        ctx: &mut AppContext,
    ) -> bool {
        false
    }
}

/// A boxed component for type-erased storage.
pub type BoxedComponent = Box<dyn Component>;

/// Extension trait for creating boxed components.
pub trait ComponentExt: Component + Sized + 'static {
    /// Box this component for type-erased storage.
    fn boxed(self) -> BoxedComponent {
        Box::new(self)
    }
}

impl<T: Component + 'static> ComponentExt for T {}
