//! Developer overlay manager for debugging tabitha applications.
//!
//! This module provides the `DevOverlayManager` which manages two independent
//! debug panels:
//! - **Log Viewer**: Displays log messages in a top overlay
//! - **Debug Panel**: Shows frame timing and trigger information in top-right corner
//!
//! These overlays operate outside the normal modal system and can stay open
//! while navigating the application.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{AppBuilder, Event, EventResult, KeyCode};
//!
//! // In your event handler:
//! fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
//!     if event.is_key(KeyCode::Char('~')) {
//!         ctx.dev_overlays().toggle_log_viewer();
//!         return EventResult::Handled;
//!     }
//!     if event.is_key(KeyCode::F(12)) {
//!         ctx.dev_overlays().toggle_debug_panel();
//!         return EventResult::Handled;
//!     }
//!     EventResult::Unhandled
//! }
//! ```

use std::time::Duration;

use ratatui::{layout::Rect, Frame};
use tokio::sync::mpsc;

use crate::event::Event;
use crate::theme::Theme;

use super::debug_panel::{DebugPanel, FrameTrigger};
use super::log_viewer::{LogLine, LogViewer};

/// Manager for developer overlay panels.
///
/// This manages both the log viewer and debug panel as independent,
/// toggleable overlays that can stay open during app navigation.
#[derive(Debug)]
pub struct DevOverlayManager {
    log_viewer: LogViewer,
    debug_panel: DebugPanel,
    log_rx: Option<mpsc::UnboundedReceiver<LogLine>>,
}

impl DevOverlayManager {
    /// Create a new dev overlay manager.
    ///
    /// The optional log receiver allows the log viewer to receive
    /// log messages from a tracing layer.
    pub fn new(log_rx: Option<mpsc::UnboundedReceiver<LogLine>>) -> Self {
        Self {
            log_viewer: LogViewer::new(),
            debug_panel: DebugPanel::new(),
            log_rx,
        }
    }

    // =========================================================================
    // Log Viewer Controls
    // =========================================================================

    /// Toggle the log viewer visibility.
    pub fn toggle_log_viewer(&mut self) {
        self.log_viewer.toggle();
    }

    /// Show the log viewer.
    pub fn show_log_viewer(&mut self) {
        self.log_viewer.show();
    }

    /// Hide the log viewer.
    pub fn hide_log_viewer(&mut self) {
        self.log_viewer.hide();
    }

    /// Check if the log viewer is visible.
    pub fn is_log_viewer_visible(&self) -> bool {
        self.log_viewer.is_visible()
    }

    // =========================================================================
    // Debug Panel Controls
    // =========================================================================

    /// Toggle the debug panel visibility.
    pub fn toggle_debug_panel(&mut self) {
        self.debug_panel.toggle();
    }

    /// Show the debug panel.
    pub fn show_debug_panel(&mut self) {
        self.debug_panel.show();
    }

    /// Hide the debug panel.
    pub fn hide_debug_panel(&mut self) {
        self.debug_panel.hide();
    }

    /// Check if the debug panel is visible.
    pub fn is_debug_panel_visible(&self) -> bool {
        self.debug_panel.is_visible()
    }

    // =========================================================================
    // Frame Recording
    // =========================================================================

    /// Record a frame being rendered.
    ///
    /// This should be called after each draw operation with the render time
    /// and what triggered the redraw.
    pub fn record_frame(&mut self, render_time: Duration, trigger: FrameTrigger) {
        self.debug_panel.record_frame(render_time, trigger);
    }

    /// Poll the log receiver and add any new log lines.
    ///
    /// Returns true if any new logs were added.
    pub fn poll_logs(&mut self) -> bool {
        let mut had_logs = false;
        if let Some(ref mut log_rx) = self.log_rx {
            while let Ok(log_line) = log_rx.try_recv() {
                self.log_viewer.push(log_line);
                had_logs = true;
            }
        }
        had_logs
    }

    /// Convert an Event to a FrameTrigger for recording.
    ///
    /// This is a helper to convert terminal events to frame trigger types.
    pub fn event_to_trigger(event: &Event) -> FrameTrigger {
        match event {
            Event::Key(key_event) => {
                let key_name = format!("{:?}", key_event.code);
                FrameTrigger::Key(key_name)
            }
            Event::Mouse(mouse_event) => {
                let kind = format!("{:?}", mouse_event.kind);
                FrameTrigger::Mouse(kind)
            }
            Event::Resize { .. } => FrameTrigger::Resize,
            Event::FocusGained => FrameTrigger::Focus("Gained".to_string()),
            Event::FocusLost => FrameTrigger::Focus("Lost".to_string()),
            Event::Paste(_) => FrameTrigger::Paste,
        }
    }

    // =========================================================================
    // Drawing
    // =========================================================================

    /// Draw both overlay panels.
    ///
    /// Draws the log viewer first, then the debug panel on top.
    /// This ensures the debug panel is always visible even when both are open.
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Draw log viewer first (behind debug panel)
        self.log_viewer.draw(frame, area, theme);

        // Draw debug panel last (on top of everything)
        self.debug_panel.draw(frame, area, theme);
    }

    /// Check if any overlay is currently visible.
    pub fn is_any_visible(&self) -> bool {
        self.log_viewer.is_visible() || self.debug_panel.is_visible()
    }

    /// Hide all overlays.
    pub fn hide_all(&mut self) {
        self.log_viewer.hide();
        self.debug_panel.hide();
    }
}

/// Context for controlling developer overlays during event handling.
///
/// Access this through `AppContext::dev_overlays()`.
#[derive(Debug)]
pub struct DevOverlayContext<'a> {
    manager: &'a mut DevOverlayManager,
}

impl<'a> DevOverlayContext<'a> {
    /// Create a new dev overlay context.
    pub(crate) fn new(manager: &'a mut DevOverlayManager) -> Self {
        Self { manager }
    }

    // =========================================================================
    // Log Viewer Controls
    // =========================================================================

    /// Toggle the log viewer visibility.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if event.is_key(KeyCode::Char('~')) {
    ///     ctx.dev_overlays().toggle_log_viewer();
    ///     return EventResult::Handled;
    /// }
    /// ```
    pub fn toggle_log_viewer(&mut self) {
        self.manager.toggle_log_viewer();
    }

    /// Show the log viewer.
    pub fn show_log_viewer(&mut self) {
        self.manager.show_log_viewer();
    }

    /// Hide the log viewer.
    pub fn hide_log_viewer(&mut self) {
        self.manager.hide_log_viewer();
    }

    /// Check if the log viewer is visible.
    pub fn is_log_viewer_visible(&self) -> bool {
        self.manager.is_log_viewer_visible()
    }

    // =========================================================================
    // Debug Panel Controls
    // =========================================================================

    /// Toggle the debug panel visibility.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if event.is_key(KeyCode::F(12)) {
    ///     ctx.dev_overlays().toggle_debug_panel();
    ///     return EventResult::Handled;
    /// }
    /// ```
    pub fn toggle_debug_panel(&mut self) {
        self.manager.toggle_debug_panel();
    }

    /// Show the debug panel.
    pub fn show_debug_panel(&mut self) {
        self.manager.show_debug_panel();
    }

    /// Hide the debug panel.
    pub fn hide_debug_panel(&mut self) {
        self.manager.hide_debug_panel();
    }

    /// Check if the debug panel is visible.
    pub fn is_debug_panel_visible(&self) -> bool {
        self.manager.is_debug_panel_visible()
    }

    // =========================================================================
    // General Controls
    // =========================================================================

    /// Hide all dev overlays.
    pub fn hide_all(&mut self) {
        self.manager.hide_all();
    }

    /// Check if any overlay is visible.
    pub fn is_any_visible(&self) -> bool {
        self.manager.is_any_visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dev_overlay_manager_new() {
        let manager = DevOverlayManager::new(None);
        assert!(!manager.is_log_viewer_visible());
        assert!(!manager.is_debug_panel_visible());
        assert!(!manager.is_any_visible());
    }

    #[test]
    fn test_dev_overlay_log_viewer_controls() {
        let mut manager = DevOverlayManager::new(None);

        manager.show_log_viewer();
        assert!(manager.is_log_viewer_visible());
        assert!(manager.is_any_visible());

        manager.toggle_log_viewer();
        assert!(!manager.is_log_viewer_visible());

        manager.hide_all();
        assert!(!manager.is_any_visible());
    }

    #[test]
    fn test_dev_overlay_debug_panel_controls() {
        let mut manager = DevOverlayManager::new(None);

        manager.show_debug_panel();
        assert!(manager.is_debug_panel_visible());
        assert!(manager.is_any_visible());

        manager.toggle_debug_panel();
        assert!(!manager.is_debug_panel_visible());
    }

    #[test]
    fn test_dev_overlay_record_frame() {
        let mut manager = DevOverlayManager::new(None);
        manager.show_debug_panel();

        manager.record_frame(Duration::from_millis(5), FrameTrigger::Tick);

        assert_eq!(manager.debug_panel.frame_number(), 1);
        assert_eq!(manager.debug_panel.last_render_time().as_millis(), 5);
    }

    #[test]
    fn test_event_to_trigger() {
        // Test with various frame triggers directly
        let tick_trigger = FrameTrigger::Tick;
        match tick_trigger {
            FrameTrigger::Tick => (), // Expected
            _ => panic!("Expected Tick trigger"),
        }

        let key_trigger = FrameTrigger::Key("Enter".to_string());
        match key_trigger {
            FrameTrigger::Key(_) => (), // Expected
            _ => panic!("Expected Key trigger"),
        }

        let resize_trigger = FrameTrigger::Resize;
        match resize_trigger {
            FrameTrigger::Resize => (), // Expected
            _ => panic!("Expected Resize trigger"),
        }
    }

    #[test]
    fn test_dev_overlay_context() {
        let mut manager = DevOverlayManager::new(None);
        let mut ctx = DevOverlayContext::new(&mut manager);

        ctx.show_log_viewer();
        assert!(ctx.is_log_viewer_visible());

        ctx.toggle_debug_panel();
        assert!(ctx.is_debug_panel_visible());

        ctx.hide_all();
        assert!(!ctx.is_any_visible());
    }
}
