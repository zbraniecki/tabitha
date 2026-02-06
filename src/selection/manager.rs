//! Selection manager for mouse-based text selection.
//!
//! The `SelectionManager` tracks mouse selection state and coordinates between
//! registered selection regions and user mouse interactions.

use std::cell::RefCell;

use crate::event::{MouseButton, MouseEventKind};
use crate::selection::{
    region::{RegionId, RegionRegistry, SelectionRegion},
    types::{MouseSelectionPhase, SelectionPos, SelectionRange},
};

/// Configuration for selection behavior.
#[derive(Debug, Clone)]
pub struct SelectionConfig {
    /// Whether to auto-copy to clipboard on MouseUp (terminal-style).
    /// When false, requires explicit Ctrl+C. Default: true.
    pub auto_copy_on_finalize: bool,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            auto_copy_on_finalize: true,
        }
    }
}

impl SelectionConfig {
    /// Create a new selection config with auto-copy disabled.
    pub fn without_auto_copy() -> Self {
        Self {
            auto_copy_on_finalize: false,
        }
    }
}

/// Manages mouse-based text selection across registered regions.
pub struct SelectionManager {
    config: SelectionConfig,
    /// The region registry for tracking selectable regions.
    /// This is wrapped in RefCell to allow mutable access through shared references.
    pub(crate) registry: RefCell<RegionRegistry>,
    phase: MouseSelectionPhase,
    active_region: Option<RegionId>,
    selection: Option<SelectionRange>,
    selected_text: Option<String>,
    /// Whether auto-copy is pending (set on MouseUp, consumed after text extraction in draw).
    pending_auto_copy: bool,
}

impl std::fmt::Debug for SelectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectionManager")
            .field("config", &self.config)
            .field("registry", &self.registry.borrow())
            .field("phase", &self.phase)
            .field("active_region", &self.active_region)
            .field("selection", &self.selection)
            .field("selected_text", &self.selected_text)
            .field("pending_auto_copy", &self.pending_auto_copy)
            .finish()
    }
}

impl SelectionManager {
    /// Create a new selection manager with default configuration.
    pub fn new() -> Self {
        Self::with_config(SelectionConfig::default())
    }

    /// Create a new selection manager with the given configuration.
    pub fn with_config(config: SelectionConfig) -> Self {
        Self {
            config,
            registry: RefCell::new(RegionRegistry::new()),
            phase: MouseSelectionPhase::Idle,
            active_region: None,
            selection: None,
            selected_text: None,
            pending_auto_copy: false,
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &SelectionConfig {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut SelectionConfig {
        &mut self.config
    }

    /// Clear all registered regions. Called at the start of each frame.
    pub fn clear_regions(&self) {
        self.registry.borrow_mut().clear();
    }

    /// Register a selection region.
    pub fn register_region(&self, id: RegionId, rect: ratatui::layout::Rect, z_order: u16) {
        self.registry.borrow_mut().register(id, rect, z_order);
    }

    /// Get a reference to the region registry.
    pub fn registry(&self) -> std::cell::Ref<'_, RegionRegistry> {
        self.registry.borrow()
    }

    /// Get a mutable reference to the region registry.
    pub fn registry_mut(&self) -> std::cell::RefMut<'_, RegionRegistry> {
        self.registry.borrow_mut()
    }

    /// Find region at the given coordinates.
    pub fn region_at(&self, col: u16, row: u16) -> Option<std::cell::Ref<'_, SelectionRegion>> {
        std::cell::Ref::filter_map(self.registry.borrow(), |registry| {
            registry.region_at(col, row)
        })
        .ok()
    }

    /// Find a region by its ID.
    pub fn region_by_id(&self, id: &RegionId) -> Option<std::cell::Ref<'_, SelectionRegion>> {
        std::cell::Ref::filter_map(self.registry.borrow(), |registry| registry.region_by_id(id))
            .ok()
    }

    /// Get the current selection phase.
    pub fn phase(&self) -> MouseSelectionPhase {
        self.phase
    }

    /// Get the currently active region ID, if any.
    pub fn active_region(&self) -> Option<&RegionId> {
        self.active_region.as_ref()
    }

    /// Get the current selection range, if any.
    pub fn selection(&self) -> Option<&SelectionRange> {
        self.selection.as_ref()
    }

    /// Get the currently selected text, if any.
    pub fn selected_text(&self) -> Option<&str> {
        self.selected_text.as_deref()
    }

    /// Check if there is an active selection.
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// Clear the current selection.
    pub fn clear_selection(&mut self) {
        self.phase = MouseSelectionPhase::Idle;
        self.active_region = None;
        self.selection = None;
        self.selected_text = None;
        self.pending_auto_copy = false;
    }

    /// Handle a mouse event and update selection state.
    ///
    /// Returns `true` if the event was consumed by the selection manager.
    pub fn handle_mouse_event(&mut self, event: &crate::event::MouseEvent) -> bool {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if we're clicking inside a region
                let maybe_region_id = {
                    let registry = self.registry.borrow();
                    registry
                        .region_at(event.column, event.row)
                        .map(|r| r.id.clone())
                };

                if let Some(region_id) = maybe_region_id {
                    let registry = self.registry.borrow();
                    if let Some(region) = registry.region_by_id(&region_id) {
                        self.phase = MouseSelectionPhase::Dragging;
                        self.active_region = Some(region_id.clone());

                        // Calculate position within region
                        let pos = self.region_local_pos(region, event.column, event.row);
                        self.selection = Some(SelectionRange::new(pos, pos));
                        self.selected_text = None;
                        return true;
                    }
                }

                // Click outside any region - clear selection
                self.clear_selection();
                false
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.phase == MouseSelectionPhase::Dragging {
                    if let Some(ref region_id) = self.active_region {
                        let registry = self.registry.borrow();
                        if let Some(region) = registry.region_by_id(region_id) {
                            // Check if drag is still within the active region
                            if region.contains(event.column, event.row) {
                                let pos = self.region_local_pos(region, event.column, event.row);
                                drop(registry); // Release borrow before mutable access
                                if let Some(ref mut sel) = self.selection {
                                    sel.cursor = pos;
                                }
                            }
                        }
                    }
                    true
                } else {
                    false
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.phase == MouseSelectionPhase::Dragging {
                    self.phase = MouseSelectionPhase::Finalized;

                    // Mark auto-copy as pending — the actual copy happens after
                    // draw() extracts the selected text from the buffer.
                    if self.config.auto_copy_on_finalize {
                        self.pending_auto_copy = true;
                    }

                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Convert screen coordinates to region-local position.
    fn region_local_pos(&self, region: &SelectionRegion, col: u16, row: u16) -> SelectionPos {
        let x = region.rect.x as u32;
        let y = region.rect.y as u32;
        SelectionPos::new((col as u32 - x) as usize, (row as u32 - y) as usize)
    }

    /// Set the selected text (called after rendering to extract text from buffer).
    ///
    /// If auto-copy is pending (from a MouseUp with `auto_copy_on_finalize`),
    /// this will also copy the text to the system clipboard.
    pub fn set_selected_text(&mut self, text: String) {
        tracing::debug!(len = text.len(), "selection: extracted text from buffer");
        self.selected_text = Some(text);

        // Perform deferred auto-copy if pending
        if self.pending_auto_copy {
            self.pending_auto_copy = false;
            #[cfg(feature = "clipboard")]
            {
                if let Some(ref text) = self.selected_text {
                    tracing::info!(len = text.len(), "selection: auto-copying to clipboard");
                    match crate::selection::clipboard::copy_to_clipboard(text) {
                        Ok(()) => {
                            tracing::info!("selection: clipboard copy succeeded");
                        }
                        Err(e) => {
                            tracing::warn!("selection: clipboard copy failed: {}", e);
                        }
                    }
                }
            }
        }
    }
}

impl Default for SelectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_selection_config_default() {
        let config = SelectionConfig::default();
        assert!(config.auto_copy_on_finalize);
    }

    #[test]
    fn test_selection_config_without_auto_copy() {
        let config = SelectionConfig::without_auto_copy();
        assert!(!config.auto_copy_on_finalize);
    }

    #[test]
    fn test_selection_manager_new() {
        let manager = SelectionManager::new();
        assert!(matches!(manager.phase(), MouseSelectionPhase::Idle));
        assert!(!manager.has_selection());
        assert!(manager.active_region().is_none());
    }

    #[test]
    fn test_selection_manager_clear_regions() {
        let manager = SelectionManager::new();
        manager.register_region("test".into(), Rect::new(0, 0, 10, 10), 0);
        assert_eq!(manager.registry().len(), 1);

        manager.clear_regions();
        assert_eq!(manager.registry().len(), 0);
    }

    #[test]
    fn test_selection_manager_clear_selection() {
        let mut manager = SelectionManager::new();
        manager.phase = MouseSelectionPhase::Finalized;
        manager.selection = Some(SelectionRange::new(
            SelectionPos::new(0, 0),
            SelectionPos::new(5, 0),
        ));

        manager.clear_selection();
        assert!(matches!(manager.phase(), MouseSelectionPhase::Idle));
        assert!(!manager.has_selection());
    }

    #[test]
    fn test_selection_manager_set_selected_text() {
        let mut manager = SelectionManager::new();
        manager.set_selected_text("Hello".to_string());
        assert_eq!(manager.selected_text(), Some("Hello"));
    }
}
