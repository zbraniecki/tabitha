//! TabBar widget for rendering tab navigation.
//!
//! This widget reads tab state from context and renders a horizontal tab bar.
//! Users control where tabs are rendered by calling `TabBar::draw()` in their
//! component's draw method.
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

use ratatui::{layout::Rect, text::Line, widgets::Tabs as RatatuiTabs, Frame};

use crate::context::traits::HasTabs;
use crate::context::{AppContext, DrawContext};
use crate::event::{Event, KeyCode};
use crate::focus::EventResult;

/// Widget that renders the tab bar. Reads tab state from context.
///
/// This is a stateless widget - all tab state is stored in the context's
/// TabManager. Users control where the tab bar is rendered by calling
/// `TabBar::draw()` in their component's draw method.
pub struct TabBar;

impl TabBar {
    /// Draw horizontal tab bar.
    ///
    /// Renders a horizontal tab bar showing all registered tabs with the
    /// active tab highlighted using theme colors.
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
        let tabs = ctx.tabs();
        let theme = ctx.theme();

        if tabs.is_empty() {
            return;
        }

        let titles: Vec<Line> = tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let style = if i == tabs.active_index() {
                    theme.highlight_style()
                } else if !tab.is_enabled() {
                    theme.muted_style()
                } else {
                    theme.fg_style()
                };
                Line::from(format!(" {} ", tab.title())).style(style)
            })
            .collect();

        let tabs_widget = RatatuiTabs::new(titles)
            .select(tabs.active_index())
            .divider("|");

        frame.render_widget(tabs_widget, area);
    }

    /// Handle tab bar events.
    ///
    /// Handles standard tab navigation shortcuts:
    /// - `Tab`: Select next tab
    /// - `Shift+Tab`: Select previous tab
    /// - `1-9`: Direct tab selection by index
    ///
    /// Returns `EventResult::Handled` if the event was consumed.
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
    ///         // Handle other events...
    ///         EventResult::Unhandled
    ///     }
    /// }
    /// ```
    pub fn handle_event(event: &Event, ctx: &mut AppContext) -> EventResult {
        match event {
            Event::Key(key) => match key.code {
                // Tab for next tab
                KeyCode::Tab => {
                    ctx.tabs().select_next();
                    EventResult::Handled
                }
                // Shift+Tab (BackTab) for previous tab
                KeyCode::BackTab => {
                    ctx.tabs().select_prev();
                    EventResult::Handled
                }
                // 1-9 for direct tab selection
                KeyCode::Char(c @ '1'..='9') => {
                    let index = (c as usize) - ('1' as usize);
                    ctx.tabs().select_by_index(index);
                    EventResult::Handled
                }
                _ => EventResult::Unhandled,
            },
            _ => EventResult::Unhandled,
        }
    }
}
