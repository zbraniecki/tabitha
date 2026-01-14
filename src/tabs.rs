//! Tab management for the TUI framework.
//!
//! This module provides traits and types for building tabbed interfaces.

use std::collections::HashSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Tabs as RatatuiTabs},
    Frame,
};

use crate::context::TabEventContext;
use crate::event::Event;
use crate::focus::EventResult;

/// A tab that can be displayed in the application.
///
/// Implement this trait to create custom tabs that can be registered
/// with the application.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Tab, DrawContext, AppContext, Event};
/// use ratatui::{Frame, layout::Rect, widgets::Paragraph};
///
/// struct HomeTab {
///     message: String,
/// }
///
/// impl Tab for HomeTab {
///     fn id(&self) -> &str {
///         "home"
///     }
///
///     fn title(&self) -> &str {
///         "Home"
///     }
///
///     fn draw(&self, frame: &mut Frame, area: Rect) {
///         frame.render_widget(Paragraph::new(&*self.message), area);
///     }
/// }
/// ```
pub trait Tab: Send {
    /// Unique identifier for this tab.
    fn id(&self) -> &str;

    /// Display title for this tab (shown in the tab bar).
    fn title(&self) -> &str;

    /// Draw the tab content to the given frame area.
    fn draw(&self, frame: &mut Frame, area: Rect);

    /// Handle an input event while this tab is active.
    ///
    /// Returns `EventResult::Handled` if the event was consumed,
    /// `EventResult::Unhandled` to bubble to parent.
    ///
    /// Note: This uses `TabEventContext` instead of `AppContext` because
    /// tabs cannot modify tab selection (that would create circular borrows).
    /// Use the focus system for widget navigation within tabs.
    #[allow(unused_variables)]
    fn handle_event(&mut self, event: &Event, ctx: &mut TabEventContext) -> EventResult {
        EventResult::Unhandled
    }

    /// Check if this tab is enabled by default.
    ///
    /// This can be overridden at runtime via `TabsEventContext::set_enabled()`.
    /// Disabled tabs are shown grayed out and cannot be selected.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Called when this tab becomes the active tab.
    fn on_activate(&mut self) {}

    /// Called when this tab is deactivated (another tab becomes active).
    fn on_deactivate(&mut self) {}
}

/// A boxed tab for type-erased storage.
pub type BoxedTab = Box<dyn Tab>;

/// Information about a registered tab.
#[derive(Debug, Clone)]
pub struct TabInfo {
    /// The tab's unique identifier.
    pub id: String,
    /// The tab's display title.
    pub title: String,
    /// Whether the tab is currently enabled (considering overrides).
    pub enabled: bool,
    /// The index of this tab.
    pub index: usize,
}

/// Manager for registered tabs.
///
/// This is used internally by the framework to manage tabs.
pub struct TabManager {
    tabs: Vec<BoxedTab>,
    active_index: usize,
    /// Tabs that have been explicitly disabled via `set_enabled(id, false)`.
    disabled_overrides: HashSet<String>,
}

impl TabManager {
    /// Create a new empty tab manager.
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_index: 0,
            disabled_overrides: HashSet::new(),
        }
    }

    /// Add a tab to the manager.
    pub fn add<T: Tab + 'static>(&mut self, tab: T) {
        self.tabs.push(Box::new(tab));
    }

    /// Get the number of tabs.
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// Check if there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Get the active tab index.
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Get the active tab, if any.
    pub fn active_tab(&self) -> Option<&dyn Tab> {
        self.tabs.get(self.active_index).map(|t| t.as_ref())
    }

    /// Get a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> Option<&mut BoxedTab> {
        self.tabs.get_mut(self.active_index)
    }

    /// Check if a tab at the given index is enabled.
    ///
    /// A tab is enabled if both:
    /// - The tab's own `is_enabled()` returns true
    /// - The tab has not been disabled via `set_enabled(id, false)`
    fn is_tab_enabled(&self, index: usize) -> bool {
        if let Some(tab) = self.tabs.get(index) {
            tab.is_enabled() && !self.disabled_overrides.contains(tab.id())
        } else {
            false
        }
    }

    /// Check if a tab with the given ID is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        if let Some(index) = self.tabs.iter().position(|t| t.id() == id) {
            self.is_tab_enabled(index)
        } else {
            false
        }
    }

    /// Enable or disable a tab by ID.
    ///
    /// When `enabled` is `false`, the tab is added to the disabled overrides.
    /// When `enabled` is `true`, the tab is removed from overrides (reverting
    /// to the tab's own `is_enabled()` state).
    ///
    /// Returns `true` if the tab was found.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        // Check if the tab exists
        if !self.tabs.iter().any(|t| t.id() == id) {
            return false;
        }

        if enabled {
            self.disabled_overrides.remove(id);
        } else {
            self.disabled_overrides.insert(id.to_string());
        }

        true
    }

    /// Get information about all tabs.
    pub fn list(&self) -> Vec<TabInfo> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| TabInfo {
                id: tab.id().to_string(),
                title: tab.title().to_string(),
                enabled: self.is_tab_enabled(index),
                index,
            })
            .collect()
    }

    /// Select a tab by index.
    ///
    /// Returns `true` if the tab was selected, `false` if the index is invalid
    /// or the tab is disabled.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        if !self.is_tab_enabled(index) {
            return false;
        }

        if index != self.active_index {
            // Deactivate old tab
            if let Some(old_tab) = self.tabs.get_mut(self.active_index) {
                old_tab.on_deactivate();
            }

            self.active_index = index;

            // Activate new tab
            if let Some(new_tab) = self.tabs.get_mut(self.active_index) {
                new_tab.on_activate();
            }
        }

        true
    }

    /// Select a tab by ID.
    ///
    /// Returns `true` if the tab was found and selected.
    pub fn select_by_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.tabs.iter().position(|t| t.id() == id) {
            self.select(index)
        } else {
            false
        }
    }

    /// Select the next enabled tab.
    ///
    /// Wraps around to the first tab if at the end.
    pub fn select_next(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        let start = self.active_index;
        let mut index = (start + 1) % self.tabs.len();

        while index != start {
            if self.is_tab_enabled(index) {
                return self.select(index);
            }
            index = (index + 1) % self.tabs.len();
        }

        false
    }

    /// Select the previous enabled tab.
    ///
    /// Wraps around to the last tab if at the beginning.
    pub fn select_prev(&mut self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        let start = self.active_index;
        let len = self.tabs.len();
        let mut index = (start + len - 1) % len;

        while index != start {
            if self.is_tab_enabled(index) {
                return self.select(index);
            }
            index = (index + len - 1) % len;
        }

        false
    }

    /// Draw the tab bar.
    pub fn draw_tabbar(&self, frame: &mut Frame, area: Rect) {
        if self.tabs.is_empty() {
            return;
        }

        let titles: Vec<Line> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let enabled = self.is_tab_enabled(i);
                let style = if !enabled {
                    Style::default().fg(Color::DarkGray)
                } else if i == self.active_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::from(Span::styled(tab.title(), style))
            })
            .collect();

        let tabs_widget = RatatuiTabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.active_index)
            .highlight_style(Style::default().fg(Color::Yellow));

        frame.render_widget(tabs_widget, area);
    }

    /// Draw the content of the active tab.
    pub fn draw_content(&self, frame: &mut Frame, area: Rect) {
        if let Some(tab) = self.active_tab() {
            tab.draw(frame, area);
        }
    }

    /// Handle an event for the active tab.
    pub fn handle_event(&mut self, event: &Event, ctx: &mut TabEventContext) -> EventResult {
        if let Some(tab) = self.active_tab_mut() {
            tab.handle_event(event, ctx)
        } else {
            EventResult::Unhandled
        }
    }
}

impl Default for TabManager {
    fn default() -> Self {
        Self::new()
    }
}
