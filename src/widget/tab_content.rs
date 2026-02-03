//! TabContent widget for rendering the active tab's content.
//!
//! This widget reads tab state from context and renders the active tab's
//! content. Users control where the content is rendered by calling
//! `TabContent::draw()` in their component's draw method.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{Component, DrawContext, Event, EventResult, AppContext};
//! use tabitha::widget::{TabBar, TabContent};
//! use ratatui::{Frame, layout::{Rect, Layout, Direction, Constraint}};
//!
//! struct MyApp;
//!
//! impl Component for MyApp {
//!     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
//!         let [tab_bar, content] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
//!             .areas(area);
//!         
//!         // User controls tab placement
//!         TabBar::draw(frame, tab_bar, ctx);
//!         TabContent::draw(frame, content, ctx);
//!     }
//!
//!     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
//!         // TabBar handles tab switching
//!         if TabBar::handle_event(event, ctx).is_handled() {
//!             return EventResult::Handled;
//!         }
//!         
//!         // Forward to active tab
//!         TabContent::handle_event(event, ctx)
//!     }
//! }
//! ```

use ratatui::{layout::Rect, Frame};

use crate::context::DrawContext;
use crate::event::Event;
use crate::focus::EventResult;

/// Widget that renders the active tab's content.
///
/// This is a stateless widget - all tab state is stored in the context's
/// TabManager. Users control where the tab content is rendered by calling
/// `TabContent::draw()` in their component's draw method.
pub struct TabContent;

impl TabContent {
    /// Draw the active tab's content.
    ///
    /// Calls the active tab's `draw` method with the given area.
    /// Does nothing if there is no active tab.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Component for MyApp {
    ///     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
    ///         let [tab_bar, content] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)])
    ///             .areas(area);
    ///         
    ///         TabBar::draw(frame, tab_bar, ctx);
    ///         TabContent::draw(frame, content, ctx);
    ///     }
    /// }
    /// ```
    pub fn draw(frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        ctx.tabs().draw_active(frame, area);
    }

    /// Forward events to the active tab.
    ///
    /// Delegates event handling to the currently active tab.
    /// Returns `EventResult::Unhandled` if there is no active tab.
    ///
    /// # Note
    ///
    /// This method requires access to the application's event handling context.
    /// In the current architecture, tabs receive events through the framework's
    /// main event loop rather than through this widget. For custom event handling,
    /// access the tab manager directly through `ctx.tabs()`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Component for MyApp {
    ///     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
    ///         // Let TabBar handle its events first
    ///         if TabBar::handle_event(event, ctx).is_handled() {
    ///             return EventResult::Handled;
    ///         }
    ///         
    ///         // For active tab event handling, the framework handles this
    ///         // automatically through the main event loop
    ///         EventResult::Unhandled
    ///     }
    /// }
    /// ```
    #[allow(dead_code)]
    pub fn handle_event(_event: &Event) -> EventResult {
        // Note: In the current architecture, the framework handles tab events
        // through the main event loop. This method exists for API consistency
        // and future expansion.
        EventResult::Unhandled
    }
}
