//! Tab management for the TUI framework.
//!
//! This module provides types for building tabbed interfaces where tabs are
//! just Components. The Tab trait has been removed - tabs are now simply
//! Components registered with metadata.

use std::collections::HashSet;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Tabs as RatatuiTabs,
    Frame,
};

use crate::component::Component;
use crate::context::DrawContext;
use crate::event::Event;
use crate::focus::EventResult;

/// Metadata for a registered tab.
///
/// This struct holds the static information about a tab - its ID, title,
/// and enabled state. The actual tab content is a `Box<dyn Component>`.
///
/// # Example
///
/// ```ignore
/// use tabitha::{TabInfo, TabManager};
/// use tabitha::component::Component;
///
/// struct HomeTab;
/// impl Component for HomeTab { /* ... */ }
///
/// let mut manager = TabManager::new();
/// manager.add(TabInfo::new("home", "Home"), HomeTab);
/// ```
#[derive(Debug, Clone)]
pub struct TabInfo {
    id: String,
    title: String,
    enabled: bool,
}

impl TabInfo {
    /// Create a new TabInfo with the given ID and title.
    ///
    /// The tab is enabled by default.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            enabled: true,
        }
    }

    /// Get the tab's unique ID.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the tab's display title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Check if the tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Immutable reference to a tab and its metadata.
///
/// This handle provides read-only access to both the tab's metadata
/// and its underlying component.
pub struct TabRef<'a> {
    info: &'a TabInfo,
    component: &'a dyn Component,
}

impl<'a> TabRef<'a> {
    /// Create a new tab reference.
    pub(crate) fn new(info: &'a TabInfo, component: &'a dyn Component) -> Self {
        Self { info, component }
    }

    /// Get the tab's metadata.
    pub fn info(&self) -> &TabInfo {
        self.info
    }

    /// Get the tab's component.
    pub fn component(&self) -> &dyn Component {
        self.component
    }
}

/// Mutable reference to a tab and its metadata.
///
/// This handle provides mutable access to the tab's underlying component
/// while maintaining read-only access to its metadata.
pub struct TabMut<'a> {
    info: &'a TabInfo,
    component: &'a mut dyn Component,
}

impl<'a> TabMut<'a> {
    /// Create a new mutable tab reference.
    pub(crate) fn new(info: &'a TabInfo, component: &'a mut dyn Component) -> Self {
        Self { info, component }
    }

    /// Get the tab's metadata.
    pub fn info(&self) -> &TabInfo {
        self.info
    }

    /// Get a mutable reference to the tab's component.
    pub fn component_mut(&mut self) -> &mut dyn Component {
        self.component
    }
}

/// Manager for registered tabs.
///
/// The TabManager stores tabs as `(TabInfo, Box<dyn Component>)` pairs.
/// It handles tab selection, lifecycle management (on_mount/on_unmount),
/// and drawing.
///
/// # Example
///
/// ```ignore
/// use tabitha::{TabInfo, TabManager};
/// use tabitha::component::Component;
///
/// struct HomeTab;
/// impl Component for HomeTab { /* ... */ }
///
/// let mut manager = TabManager::new();
/// manager.add(TabInfo::new("home", "Home"), HomeTab);
/// manager.add(TabInfo::new("settings", "Settings"), SettingsTab);
///
/// // Select a tab by index
/// manager.select(0);
///
/// // Select a tab by ID
/// manager.select_by_id("settings");
/// ```
pub struct TabManager {
    tabs: Vec<(TabInfo, Box<dyn Component>)>,
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
    ///
    /// The tab is added with the given metadata and component.
    /// The component is boxed for type-erased storage.
    pub fn add(&mut self, info: TabInfo, component: impl Component + 'static) {
        self.tabs.push((info, Box::new(component)));
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

    /// Get the active tab's info and component, if any.
    pub fn active(&self) -> Option<(&TabInfo, &dyn Component)> {
        self.tabs
            .get(self.active_index)
            .map(|(info, comp)| (info, comp.as_ref()))
    }

    /// Get a mutable reference to the active tab's component.
    pub fn active_component_mut(&mut self) -> Option<&mut (dyn Component + '_)> {
        match self.tabs.get_mut(self.active_index) {
            Some((_, comp)) => Some(comp.as_mut()),
            None => None,
        }
    }

    /// Get a component by index (mutable).
    pub(crate) fn get_component_mut(&mut self, index: usize) -> Option<&mut (dyn Component + '_)> {
        match self.tabs.get_mut(index) {
            Some((_, comp)) => Some(comp.as_mut()),
            None => None,
        }
    }

    /// Check if a tab at the given index is enabled.
    ///
    /// A tab is enabled if both:
    /// - The tab's own `is_enabled()` returns true
    /// - The tab has not been disabled via `set_enabled(id, false)`
    pub fn is_enabled(&self, index: usize) -> bool {
        if let Some((info, _)) = self.tabs.get(index) {
            info.enabled && !self.disabled_overrides.contains(&info.id)
        } else {
            false
        }
    }

    /// Check if a tab with the given ID is enabled.
    pub fn is_enabled_by_id(&self, id: &str) -> bool {
        if let Some(index) = self.index_by_id(id) {
            self.is_enabled(index)
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
        if !self.tabs.iter().any(|(info, _)| info.id == id) {
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
    pub fn list(&self) -> Vec<&TabInfo> {
        self.tabs.iter().map(|(info, _)| info).collect()
    }

    /// Get a tab by ID (immutable).
    pub fn get(&self, id: &str) -> Option<TabRef<'_>> {
        self.tabs
            .iter()
            .find(|(info, _)| info.id == id)
            .map(|(info, comp)| TabRef::new(info, comp.as_ref()))
    }

    /// Get a tab by ID (mutable).
    pub fn get_mut(&mut self, id: &str) -> Option<TabMut<'_>> {
        self.tabs
            .iter_mut()
            .find(|(info, _)| info.id == id)
            .map(|(info, comp)| TabMut::new(info, comp.as_mut()))
    }

    /// Get the index of a tab by ID.
    pub fn index_by_id(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|(info, _)| info.id == id)
    }

    /// Select a tab by index.
    ///
    /// Returns `Some(())` if the tab was selected, `None` if the index is invalid
    /// or the tab is disabled.
    pub fn select(&mut self, index: usize) -> Option<()> {
        if index >= self.tabs.len() {
            return None;
        }

        if !self.is_enabled(index) {
            return None;
        }

        self.active_index = index;
        Some(())
    }

    /// Select a tab by ID.
    ///
    /// Returns `true` if the tab was found and selected.
    pub fn select_by_id(&mut self, id: &str) -> bool {
        if let Some(index) = self.index_by_id(id) {
            self.select(index).is_some()
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
            if self.is_enabled(index) {
                self.active_index = index;
                return true;
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
            if self.is_enabled(index) {
                self.active_index = index;
                return true;
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
            .map(|(i, (info, _))| {
                let enabled = self.is_enabled(i);
                let style = if !enabled {
                    Style::default().fg(Color::DarkGray)
                } else if i == self.active_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::from(Span::styled(info.title.clone(), style))
            })
            .collect();

        let tabs_widget = RatatuiTabs::new(titles)
            .select(self.active_index)
            .highlight_style(Style::default().fg(Color::Yellow));

        frame.render_widget(tabs_widget, area);
    }

    /// Draw the content of the active tab.
    pub fn draw_content(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if let Some((_, comp)) = self.active() {
            comp.draw(frame, area, ctx);
        }
    }

    /// Draw the active tab's content (alias for draw_content).
    pub fn draw_active(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        self.draw_content(frame, area, ctx);
    }

    /// Handle an event for the active tab.
    pub fn handle_event(
        &mut self,
        event: &Event,
        ctx: &mut crate::context::AppContext,
    ) -> EventResult {
        if let Some(comp) = self.active_component_mut() {
            comp.handle_event(event, ctx)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Component;
    use crate::context::{LifecycleContext, TabsEventContext};
    use crate::event::Event;
    use crate::focus::{EventResult, FocusManager};
    use ratatui::{layout::Rect, Frame};
    use std::sync::{Arc, Mutex};

    /// Shared state to track lifecycle calls across boxed components
    #[derive(Clone, Default)]
    struct LifecycleState {
        mount_counts: Arc<Mutex<std::collections::HashMap<String, usize>>>,
        unmount_counts: Arc<Mutex<std::collections::HashMap<String, usize>>>,
    }

    impl LifecycleState {
        fn new() -> Self {
            Self::default()
        }

        fn record_mount(&self, id: &str) {
            let mut counts = self.mount_counts.lock().unwrap();
            *counts.entry(id.to_string()).or_insert(0) += 1;
        }

        fn record_unmount(&self, id: &str) {
            let mut counts = self.unmount_counts.lock().unwrap();
            *counts.entry(id.to_string()).or_insert(0) += 1;
        }

        fn mount_count(&self, id: &str) -> usize {
            self.mount_counts
                .lock()
                .unwrap()
                .get(id)
                .copied()
                .unwrap_or(0)
        }

        fn unmount_count(&self, id: &str) -> usize {
            self.unmount_counts
                .lock()
                .unwrap()
                .get(id)
                .copied()
                .unwrap_or(0)
        }
    }

    /// Test component that tracks lifecycle calls via shared state
    struct LifecycleTracker {
        id: String,
        state: LifecycleState,
    }

    impl LifecycleTracker {
        fn new(id: &str, state: LifecycleState) -> Self {
            Self {
                id: id.to_string(),
                state,
            }
        }
    }

    impl Component for LifecycleTracker {
        fn draw(&self, _frame: &mut Frame, _area: Rect, _ctx: &DrawContext) {}

        fn handle_event(
            &mut self,
            _event: &Event,
            _ctx: &mut crate::context::AppContext,
        ) -> EventResult {
            EventResult::Unhandled
        }

        fn on_mount(&mut self, _ctx: &mut LifecycleContext) {
            self.state.record_mount(&self.id);
        }

        fn on_unmount(&mut self, _ctx: &mut LifecycleContext) {
            self.state.record_unmount(&self.id);
        }
    }

    #[test]
    fn test_tab_lifecycle_hooks_called_on_switch() {
        let state = LifecycleState::new();
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        // Create tabs with shared state
        let tab1 = LifecycleTracker::new("tab1", state.clone());
        let tab2 = LifecycleTracker::new("tab2", state.clone());

        // Add tabs to manager
        manager.add(TabInfo::new("tab1", "Tab 1"), tab1);
        manager.add(TabInfo::new("tab2", "Tab 2"), tab2);

        // Create TabsEventContext
        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Initially tab1 is at index 0, but hasn't been "mounted" yet
        // Switch to tab1 first to trigger its mount
        tabs_ctx.select(0);

        // Verify tab1 was mounted
        assert_eq!(state.mount_count("tab1"), 1, "tab1 should be mounted once");
        assert_eq!(
            state.unmount_count("tab1"),
            0,
            "tab1 should not be unmounted yet"
        );

        // Switch to tab2
        tabs_ctx.select(1);

        // Verify lifecycle hooks were called
        assert_eq!(
            state.mount_count("tab1"),
            1,
            "tab1 mount count should remain 1"
        );
        assert_eq!(
            state.unmount_count("tab1"),
            1,
            "tab1 should be unmounted once"
        );
        assert_eq!(state.mount_count("tab2"), 1, "tab2 should be mounted once");
        assert_eq!(
            state.unmount_count("tab2"),
            0,
            "tab2 should not be unmounted yet"
        );

        // Switch back to tab1
        tabs_ctx.select(0);

        // Verify lifecycle hooks were called again
        assert_eq!(state.mount_count("tab1"), 2, "tab1 should be mounted twice");
        assert_eq!(
            state.unmount_count("tab1"),
            1,
            "tab1 unmount count should remain 1"
        );
        assert_eq!(
            state.mount_count("tab2"),
            1,
            "tab2 mount count should remain 1"
        );
        assert_eq!(
            state.unmount_count("tab2"),
            1,
            "tab2 should be unmounted once"
        );
    }

    #[test]
    fn test_tab_select_next_calls_lifecycle() {
        let state = LifecycleState::new();
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        // Create three tabs
        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", state.clone()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", state.clone()),
        );
        manager.add(
            TabInfo::new("tab3", "Tab 3"),
            LifecycleTracker::new("tab3", state.clone()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Mount first tab
        tabs_ctx.select(0);
        assert_eq!(state.mount_count("tab1"), 1);

        // Use select_next
        tabs_ctx.select_next();

        assert_eq!(state.unmount_count("tab1"), 1);
        assert_eq!(state.mount_count("tab2"), 1);

        // Use select_next again
        tabs_ctx.select_next();

        assert_eq!(state.unmount_count("tab2"), 1);
        assert_eq!(state.mount_count("tab3"), 1);
    }

    #[test]
    fn test_tab_select_prev_calls_lifecycle() {
        let state = LifecycleState::new();
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        // Create three tabs
        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", state.clone()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", state.clone()),
        );
        manager.add(
            TabInfo::new("tab3", "Tab 3"),
            LifecycleTracker::new("tab3", state.clone()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Start at tab 2 (index 1)
        tabs_ctx.select(1);
        assert_eq!(state.mount_count("tab2"), 1);

        // Use select_prev to go to tab1
        tabs_ctx.select_prev();

        assert_eq!(state.unmount_count("tab2"), 1);
        assert_eq!(state.mount_count("tab1"), 1);
    }

    #[test]
    fn test_tab_select_by_id_calls_lifecycle() {
        let state = LifecycleState::new();
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        manager.add(
            TabInfo::new("home", "Home"),
            LifecycleTracker::new("home", state.clone()),
        );
        manager.add(
            TabInfo::new("settings", "Settings"),
            LifecycleTracker::new("settings", state.clone()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Mount home
        tabs_ctx.select_by_id("home");
        assert_eq!(state.mount_count("home"), 1);

        // Switch to settings by ID
        tabs_ctx.select_by_id("settings");

        assert_eq!(state.unmount_count("home"), 1);
        assert_eq!(state.mount_count("settings"), 1);
    }

    #[test]
    fn test_tab_info_new() {
        let info = TabInfo::new("test", "Test Tab");
        assert_eq!(info.id(), "test");
        assert_eq!(info.title(), "Test Tab");
        assert!(info.is_enabled());
    }

    #[test]
    fn test_tab_manager_add_and_list() {
        let mut manager = TabManager::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", LifecycleState::new()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", LifecycleState::new()),
        );

        assert_eq!(manager.len(), 2);
        assert!(!manager.is_empty());

        let list = manager.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id(), "tab1");
        assert_eq!(list[1].id(), "tab2");
    }

    #[test]
    fn test_tab_manager_get() {
        let mut manager = TabManager::new();
        let state = LifecycleState::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", state.clone()),
        );

        // Test get
        let tab_ref = manager.get("tab1");
        assert!(tab_ref.is_some());
        assert_eq!(tab_ref.unwrap().info().id(), "tab1");

        // Test get_mut
        let tab_mut = manager.get_mut("tab1");
        assert!(tab_mut.is_some());
        assert_eq!(tab_mut.unwrap().info().id(), "tab1");

        // Test non-existent tab
        assert!(manager.get("nonexistent").is_none());
        assert!(manager.get_mut("nonexistent").is_none());
    }

    #[test]
    fn test_tab_manager_select_by_index() {
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", LifecycleState::new()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", LifecycleState::new()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Select by index
        assert!(tabs_ctx.select_by_index(1));
        assert_eq!(tabs_ctx.active_index(), 1);

        // Invalid index
        assert!(!tabs_ctx.select_by_index(10));
    }

    #[test]
    fn test_tab_manager_set_enabled() {
        let mut manager = TabManager::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", LifecycleState::new()),
        );

        // Initially enabled
        assert!(manager.is_enabled_by_id("tab1"));

        // Disable
        assert!(manager.set_enabled("tab1", false));
        assert!(!manager.is_enabled_by_id("tab1"));

        // Re-enable
        assert!(manager.set_enabled("tab1", true));
        assert!(manager.is_enabled_by_id("tab1"));

        // Non-existent tab
        assert!(!manager.set_enabled("nonexistent", false));
    }

    #[test]
    fn test_cannot_select_disabled_tab() {
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", LifecycleState::new()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", LifecycleState::new()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Select first tab
        tabs_ctx.select(0);
        assert_eq!(tabs_ctx.active_index(), 0);

        // Disable second tab
        tabs_ctx.set_enabled("tab2", false);

        // Try to select disabled tab
        assert!(!tabs_ctx.select(1));
        assert_eq!(tabs_ctx.active_index(), 0); // Still on first tab
    }

    #[test]
    fn test_select_next_skips_disabled() {
        let mut manager = TabManager::new();
        let mut focus_manager = FocusManager::new();

        manager.add(
            TabInfo::new("tab1", "Tab 1"),
            LifecycleTracker::new("tab1", LifecycleState::new()),
        );
        manager.add(
            TabInfo::new("tab2", "Tab 2"),
            LifecycleTracker::new("tab2", LifecycleState::new()),
        );
        manager.add(
            TabInfo::new("tab3", "Tab 3"),
            LifecycleTracker::new("tab3", LifecycleState::new()),
        );

        let mut tabs_ctx = TabsEventContext::new(&mut manager, &mut focus_manager);

        // Start at tab1
        tabs_ctx.select(0);

        // Disable tab2
        tabs_ctx.set_enabled("tab2", false);

        // select_next should skip tab2 and go to tab3
        tabs_ctx.select_next();
        assert_eq!(tabs_ctx.active_index(), 2);
    }
}
