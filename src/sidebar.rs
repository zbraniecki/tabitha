//! Sidebar management for tabitha TUI framework.
//!
//! This module provides a flexible sidebar system with support for:
//! - Multiple panels (tab-like switching)
//! - Configurable width with animated transitions
//! - Left/right positioning
//! - Show/hide with slide animations
//!
//! # Example
//!
//! ```ignore
//! use tabitha::sidebar::{SidebarState, Side, Sidebar};
//! use ratatui::layout::Constraint;
//!
//! // In your component
//! let mut sidebar_state = SidebarState::new(Side::Left)
//!     .with_width(Constraint::Percentage(25));
//!
//! // In your draw method
//! let [main_area, sidebar_area] = Sidebar::new(&sidebar_state, area);
//! // Draw your main content in main_area
//! // Draw sidebar content in sidebar_area
//!
//! // In your event handler
//! if event.is_key(KeyCode::Char('b')) && key.modifiers == KeyModifiers::CONTROL {
//!     sidebar_state.toggle();
//! }
//! sidebar_state.set_to_width(Constraint::Length(30)); // Animated if enabled
//! ```

use std::time::Duration;

use ratatui::layout::Constraint;

use crate::animation::AnimationController;

/// Which side of the screen the sidebar appears on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// Sidebar on the left side
    #[default]
    Left,
    /// Sidebar on the right side
    Right,
}

/// The display state of the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarVisibility {
    /// Sidebar is fully visible
    #[default]
    Visible,
    /// Sidebar is transitioning (animating)
    Transitioning,
    /// Sidebar is hidden/collapsed
    Hidden,
}

/// A panel that can be displayed in the sidebar.
pub trait SidebarPanel: Send {
    /// Unique identifier for the panel
    fn id(&self) -> &str;

    /// Display title for the panel
    fn title(&self) -> &str;

    /// Draw the panel content
    fn draw(&self, frame: &mut ratatui::Frame, area: ratatui::layout::Rect);
}

/// State management for the sidebar.
///
/// This struct holds all the state needed to manage a sidebar including
/// visibility, width, active panel, and animation state. It's designed
/// to be owned by the application/component, not by the framework.
pub struct SidebarState {
    /// Which side the sidebar appears on
    side: Side,
    /// Current width constraint
    width: Constraint,
    /// Target width constraint (for animation)
    target_width: Constraint,
    /// Current visibility state
    visibility: SidebarVisibility,
    /// Whether the sidebar is currently visible (accounting for animation)
    effective_visible: bool,
    /// Current effective width (accounting for animation)
    effective_width_percent: f32,
    /// Panels registered in the sidebar
    panels: Vec<Box<dyn SidebarPanel>>,
    /// Index of the currently active panel
    active_panel: usize,
    /// Animation controller (optional - if None, animations are immediate)
    animation_controller: Option<AnimationController>,
    /// Animation duration for transitions
    animation_duration: Duration,
    /// Whether animations are enabled
    animations_enabled: bool,
}

impl std::fmt::Debug for SidebarState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SidebarState")
            .field("side", &self.side)
            .field("width", &self.width)
            .field("target_width", &self.target_width)
            .field("visibility", &self.visibility)
            .field("effective_visible", &self.effective_visible)
            .field("effective_width_percent", &self.effective_width_percent)
            .field("panels", &self.panels.len())
            .field("active_panel", &self.active_panel)
            .field("animation_controller", &self.animation_controller)
            .field("animation_duration", &self.animation_duration)
            .field("animations_enabled", &self.animations_enabled)
            .finish()
    }
}

impl SidebarState {
    /// Create a new sidebar state with default settings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::{SidebarState, Side};
    ///
    /// let sidebar = SidebarState::new(Side::Left);
    /// ```
    pub fn new(side: Side) -> Self {
        Self {
            side,
            width: Constraint::Percentage(25),
            target_width: Constraint::Percentage(25),
            visibility: SidebarVisibility::Visible,
            effective_visible: true,
            effective_width_percent: 25.0,
            panels: Vec::new(),
            active_panel: 0,
            animation_controller: None,
            animation_duration: Duration::from_millis(200),
            animations_enabled: false,
        }
    }

    /// Set the sidebar width.
    ///
    /// This sets both current and target width immediately.
    /// For animated width changes, use [`set_to_width`](Self::set_to_width).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::SidebarState;
    /// use ratatui::layout::Constraint;
    ///
    /// let mut sidebar = SidebarState::default();
    /// sidebar.set_width(Constraint::Length(30));
    /// ```
    pub fn set_width(&mut self, width: Constraint) {
        self.width = width;
        self.target_width = width;
        self.update_effective_width();
    }

    /// Get the current width constraint.
    pub fn width(&self) -> Constraint {
        self.width
    }

    /// Set the width with animation.
    ///
    /// If animations are enabled, the sidebar will smoothly transition
    /// to the new width. If disabled, this behaves like [`set_width`](Self::set_width).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::SidebarState;
    /// use ratatui::layout::Constraint;
    ///
    /// let mut sidebar = SidebarState::default()
    ///     .with_animations(true);
    ///
    /// // This will animate to 30 columns over 200ms
    /// sidebar.set_to_width(Constraint::Length(30));
    /// ```
    pub fn set_to_width(&mut self, width: Constraint) {
        self.target_width = width;

        if !self.animations_enabled || self.animation_controller.is_none() {
            // No animation, apply immediately
            self.width = width;
            self.update_effective_width();
        }
        // Animation will happen in tick() - width animation is handled separately
        // Don't change visibility state here, as that triggers show/hide animation
    }

    /// Toggle sidebar visibility.
    ///
    /// If the sidebar is visible, it will be hidden. If hidden, it will be shown.
    /// This respects animation settings.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::SidebarState;
    ///
    /// let mut sidebar = SidebarState::default();
    ///
    /// // Toggle on Ctrl+B
    /// if key.code == KeyCode::Char('b') && key.modifiers == KeyModifiers::CONTROL {
    ///     sidebar.toggle();
    /// }
    /// ```
    pub fn toggle(&mut self) {
        // If we're in the middle of an animation, reverse direction
        if self.visibility == SidebarVisibility::Transitioning {
            self.effective_visible = !self.effective_visible;
            return;
        }

        // Otherwise, normal toggle based on current state
        if self.effective_visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Show the sidebar.
    ///
    /// If animations are enabled, the sidebar will slide in.
    /// The sidebar will restore to its previous width when shown.
    pub fn show(&mut self) {
        if self.visibility == SidebarVisibility::Visible {
            return;
        }

        if !self.animations_enabled || self.animation_controller.is_none() {
            // No animation, show immediately
            self.visibility = SidebarVisibility::Visible;
            self.effective_visible = true;
            self.effective_width_percent = self.constraint_to_percent(self.width);
        } else {
            // Start show animation
            self.visibility = SidebarVisibility::Transitioning;
            self.effective_visible = true;
            // Animation will update effective_width_percent in tick()
        }
    }

    /// Hide the sidebar.
    ///
    /// If animations are enabled, the sidebar will slide out.
    /// The width is preserved for when the sidebar is shown again.
    pub fn hide(&mut self) {
        if self.visibility == SidebarVisibility::Hidden {
            return;
        }

        // Complete any ongoing width animation immediately
        if self.width != self.target_width {
            self.width = self.target_width;
            self.effective_width_percent = self.constraint_to_percent(self.width);
        }

        if !self.animations_enabled || self.animation_controller.is_none() {
            // No animation, hide immediately
            self.visibility = SidebarVisibility::Hidden;
            self.effective_visible = false;
            self.effective_width_percent = 0.0;
        } else {
            // Start hide animation
            self.visibility = SidebarVisibility::Transitioning;
            self.effective_visible = false;
            // Animation will update effective_width_percent to 0 in tick()
        }
    }

    /// Check if the sidebar is currently visible.
    ///
    /// This returns true if the sidebar should be rendered, accounting
    /// for any ongoing animations.
    pub fn is_visible(&self) -> bool {
        // Sidebar is visible if:
        // 1. It's marked as visible and has width, OR
        // 2. It's in a transition state (animating show/hide)
        (self.effective_visible && self.effective_width_percent > 0.0)
            || self.visibility == SidebarVisibility::Transitioning
    }

    /// Check if the sidebar is completely hidden.
    pub fn is_hidden(&self) -> bool {
        !self.is_visible()
    }

    /// Check if the sidebar is currently animating.
    pub fn is_animating(&self) -> bool {
        self.visibility == SidebarVisibility::Transitioning
    }

    /// Get which side the sidebar is on.
    pub fn side(&self) -> Side {
        self.side
    }

    /// Set which side the sidebar appears on.
    pub fn set_side(&mut self, side: Side) {
        self.side = side;
    }

    /// Enable or disable animations.
    ///
    /// When enabled, show/hide and width changes will be animated.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::SidebarState;
    ///
    /// let mut sidebar = SidebarState::default()
    ///     .with_animations(true)
    ///     .with_animation_duration(Duration::from_millis(300));
    /// ```
    pub fn with_animations(mut self, enabled: bool) -> Self {
        self.animations_enabled = enabled;
        if enabled && self.animation_controller.is_none() {
            self.animation_controller = Some(AnimationController::new());
        }
        self
    }

    /// Set the animation duration for transitions.
    ///
    /// Default is 200ms.
    pub fn with_animation_duration(mut self, duration: Duration) -> Self {
        self.animation_duration = duration;
        self
    }

    /// Get the current animation duration.
    pub fn animation_duration(&self) -> Duration {
        self.animation_duration
    }

    /// Set animation duration.
    pub fn set_animation_duration(&mut self, duration: Duration) {
        self.animation_duration = duration;
    }

    /// Set the width using builder pattern.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::SidebarState;
    /// use ratatui::layout::Constraint;
    ///
    /// let sidebar = SidebarState::default()
    ///     .with_width(Constraint::Length(30));
    /// ```
    pub fn with_width(mut self, width: Constraint) -> Self {
        self.set_width(width);
        self
    }

    /// Check if animations are enabled.
    pub fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }

    /// Set animations enabled/disabled.
    pub fn set_animations_enabled(&mut self, enabled: bool) {
        self.animations_enabled = enabled;
        if enabled && self.animation_controller.is_none() {
            self.animation_controller = Some(AnimationController::new());
        }
    }

    /// Add a panel to the sidebar.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::sidebar::{SidebarState, SidebarPanel};
    ///
    /// struct NavPanel;
    /// impl SidebarPanel for NavPanel {
    ///     fn id(&self) -> &str { "nav" }
    ///     fn title(&self) -> &str { "Navigation" }
    ///     fn draw(&self, frame: &mut Frame, area: Rect) { /* ... */ }
    /// }
    ///
    /// let mut sidebar = SidebarState::default();
    /// sidebar.add_panel(Box::new(NavPanel));
    /// ```
    pub fn add_panel(&mut self, panel: Box<dyn SidebarPanel>) {
        self.panels.push(panel);
    }

    /// Remove a panel by ID.
    ///
    /// Returns the removed panel if found.
    pub fn remove_panel(&mut self, id: &str) -> Option<Box<dyn SidebarPanel>> {
        if let Some(index) = self.panels.iter().position(|p| p.id() == id) {
            Some(self.panels.remove(index))
        } else {
            None
        }
    }

    /// Get the currently active panel index.
    pub fn active_panel_index(&self) -> usize {
        self.active_panel
    }

    /// Get the currently active panel.
    pub fn active_panel(&self) -> Option<&dyn SidebarPanel> {
        self.panels.get(self.active_panel).map(|p| p.as_ref())
    }

    /// Switch to the next panel.
    ///
    /// Wraps around to the first panel if at the end.
    pub fn next_panel(&mut self) {
        if !self.panels.is_empty() {
            self.active_panel = (self.active_panel + 1) % self.panels.len();
        }
    }

    /// Switch to the previous panel.
    ///
    /// Wraps around to the last panel if at the beginning.
    pub fn prev_panel(&mut self) {
        if !self.panels.is_empty() {
            self.active_panel = if self.active_panel == 0 {
                self.panels.len() - 1
            } else {
                self.active_panel - 1
            };
        }
    }

    /// Switch to a specific panel by ID.
    ///
    /// Returns true if the panel was found and activated.
    pub fn set_active_panel(&mut self, id: &str) -> bool {
        if let Some(index) = self.panels.iter().position(|p| p.id() == id) {
            self.active_panel = index;
            true
        } else {
            false
        }
    }

    /// Get all panel IDs.
    pub fn panel_ids(&self) -> impl Iterator<Item = &str> {
        self.panels.iter().map(|p| p.id())
    }

    /// Get the number of panels.
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    /// Check if there are any panels.
    pub fn has_panels(&self) -> bool {
        !self.panels.is_empty()
    }

    /// Update animation state.
    ///
    /// Call this in your application's tick/update loop when animations are enabled.
    /// Returns true if a redraw is needed due to animation changes.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In your app's main loop
    /// if sidebar_state.tick(Duration::from_millis(16)) {
    ///     // Request redraw
    /// }
    /// ```
    pub fn tick(&mut self, elapsed: Duration) -> bool {
        if !self.animations_enabled {
            return false;
        }

        let mut needs_redraw = false;

        // Priority 1: Handle show/hide animation first
        // This takes precedence over width changes
        if self.visibility == SidebarVisibility::Transitioning {
            let target_percent = if self.effective_visible {
                self.constraint_to_percent(self.target_width)
            } else {
                0.0
            };

            let current_percent = self.effective_width_percent;
            let delta = target_percent - current_percent;

            if self.effective_visible {
                // Showing - animate from 0 to target
                let step = delta
                    * (elapsed.as_millis() as f32 / self.animation_duration.as_millis() as f32);

                if delta.abs() < 0.5 || current_percent >= target_percent {
                    self.effective_width_percent = target_percent;
                    self.visibility = SidebarVisibility::Visible;
                    // Update width to match target after show completes
                    self.width = self.target_width;
                } else {
                    self.effective_width_percent += step;
                }
            } else {
                // Hiding - animate from current to 0
                let step = delta
                    * (elapsed.as_millis() as f32 / self.animation_duration.as_millis() as f32);

                if delta.abs() < 0.5 || current_percent <= 0.0 {
                    self.effective_width_percent = 0.0;
                    self.visibility = SidebarVisibility::Hidden;
                } else {
                    self.effective_width_percent += step;
                }
            }
            needs_redraw = true;
        }
        // Priority 2: Handle width animation only when not transitioning
        else if self.width != self.target_width && self.visibility != SidebarVisibility::Hidden {
            // Animate towards target width
            let target_percent = self.constraint_to_percent(self.target_width);
            let current_percent = self.effective_width_percent;

            let delta = target_percent - current_percent;
            let step =
                delta * (elapsed.as_millis() as f32 / self.animation_duration.as_millis() as f32);

            if delta.abs() < 0.5 || elapsed >= self.animation_duration {
                // Animation complete
                self.effective_width_percent = target_percent;
                self.width = self.target_width;
            } else {
                self.effective_width_percent += step;
            }
            needs_redraw = true;
        }

        needs_redraw
    }

    /// Get the current effective width as a percentage.
    ///
    /// This accounts for animations and visibility state.
    pub(crate) fn effective_width_percent(&self) -> f32 {
        self.effective_width_percent
    }

    /// Update effective width from current constraint.
    fn update_effective_width(&mut self) {
        self.effective_width_percent = self.constraint_to_percent(self.width);
    }

    /// Convert a constraint to a percentage value.
    fn constraint_to_percent(&self, constraint: Constraint) -> f32 {
        match constraint {
            Constraint::Percentage(p) => p as f32,
            Constraint::Ratio(num, den) => (num as f32 / den as f32) * 100.0,
            // For Length/Max/Min, we'd need to know the container size
            // For now, treat as percentage or use a default
            _ => 25.0, // Default fallback
        }
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new(Side::Left)
    }
}

impl Clone for SidebarState {
    fn clone(&self) -> Self {
        // Note: Panels cannot be cloned, so we create an empty sidebar
        // This is primarily for allowing struct fields to derive Clone
        Self {
            side: self.side,
            width: self.width,
            target_width: self.target_width,
            visibility: self.visibility,
            effective_visible: self.effective_visible,
            effective_width_percent: self.effective_width_percent,
            panels: Vec::new(),
            active_panel: self.active_panel,
            animation_controller: self
                .animation_controller
                .as_ref()
                .map(|_| AnimationController::new()),
            animation_duration: self.animation_duration,
            animations_enabled: self.animations_enabled,
        }
    }
}

/// A widget that splits an area into main content and sidebar.
///
/// This is used in the draw method to calculate areas for layout.
///
/// # Example
///
/// ```ignore
/// use tabitha::sidebar::{SidebarState, Sidebar};
///
/// fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
///     let [main_area, sidebar_area] = Sidebar::new(&self.sidebar_state, area);
///     
///     // Draw main content
///     self.main_content.draw(frame, main_area, ctx);
///     
///     // Draw sidebar if visible
///     if let Some(panel) = self.sidebar_state.active_panel() {
///         panel.draw(frame, sidebar_area);
///     }
/// }
/// ```
pub struct Sidebar;

impl Sidebar {
    /// Create a new sidebar layout widget.
    ///
    /// Returns an array of two rectangles: `[main_area, sidebar_area]`.
    /// The order depends on which side the sidebar is on.
    ///
    /// If the sidebar is hidden, the sidebar_area will have zero width.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(state: &SidebarState, area: ratatui::layout::Rect) -> [ratatui::layout::Rect; 2] {
        if !state.is_visible() || state.effective_width_percent() <= 0.0 {
            // Sidebar is hidden, return full area for main content
            return [area, ratatui::layout::Rect::default()];
        }

        let total_width = area.width as f32;
        let sidebar_width = (total_width * state.effective_width_percent() / 100.0) as u16;

        // Ensure minimum width of 1 if visible
        let sidebar_width = if state.is_visible() && sidebar_width == 0 {
            1
        } else {
            sidebar_width.min(area.width)
        };

        let main_width = area.width.saturating_sub(sidebar_width);

        match state.side() {
            Side::Left => {
                let sidebar_area = ratatui::layout::Rect {
                    x: area.x,
                    y: area.y,
                    width: sidebar_width,
                    height: area.height,
                };
                let main_area = ratatui::layout::Rect {
                    x: area.x + sidebar_width,
                    y: area.y,
                    width: main_width,
                    height: area.height,
                };
                [main_area, sidebar_area]
            }
            Side::Right => {
                let main_area = ratatui::layout::Rect {
                    x: area.x,
                    y: area.y,
                    width: main_width,
                    height: area.height,
                };
                let sidebar_area = ratatui::layout::Rect {
                    x: area.x + main_width,
                    y: area.y,
                    width: sidebar_width,
                    height: area.height,
                };
                [main_area, sidebar_area]
            }
        }
    }

    /// Get just the sidebar area (useful if you already have main area).
    ///
    /// Returns `None` if the sidebar is hidden.
    pub fn sidebar_area(
        state: &SidebarState,
        area: ratatui::layout::Rect,
    ) -> Option<ratatui::layout::Rect> {
        if !state.is_visible() {
            return None;
        }

        let total_width = area.width as f32;
        let sidebar_width = (total_width * state.effective_width_percent() / 100.0) as u16;
        let sidebar_width = sidebar_width.max(1).min(area.width);

        let x = match state.side() {
            Side::Left => area.x,
            Side::Right => area.x + area.width - sidebar_width,
        };

        Some(ratatui::layout::Rect {
            x,
            y: area.y,
            width: sidebar_width,
            height: area.height,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_state_default() {
        let state = SidebarState::default();
        assert_eq!(state.side(), Side::Left);
        assert!(state.is_visible());
        assert_eq!(state.panel_count(), 0);
    }

    #[test]
    fn test_sidebar_state_new() {
        let state = SidebarState::new(Side::Right);
        assert_eq!(state.side(), Side::Right);
        assert!(state.is_visible());
    }

    #[test]
    fn test_toggle_visibility() {
        let mut state = SidebarState::default();

        assert!(state.is_visible());
        state.toggle();
        assert!(!state.is_visible());
        state.toggle();
        assert!(state.is_visible());
    }

    #[test]
    fn test_toggle_visibility_with_animation() {
        let mut state = SidebarState::default()
            .with_animations(true)
            .with_animation_duration(Duration::from_millis(100));

        assert!(state.is_visible());
        assert!(!state.is_animating());

        // Start hiding
        state.toggle();
        assert!(state.is_animating());

        // Tick multiple times to complete animation (ease-out style)
        for _ in 0..20 {
            state.tick(Duration::from_millis(20));
            if !state.is_animating() {
                break;
            }
        }
        assert!(!state.is_animating());
        assert!(!state.is_visible());

        // Start showing
        state.toggle();
        assert!(state.is_animating());

        // Tick multiple times to complete animation
        for _ in 0..20 {
            state.tick(Duration::from_millis(20));
            if !state.is_animating() {
                break;
            }
        }
        assert!(!state.is_animating());
        assert!(state.is_visible());
    }

    #[test]
    fn test_sidebar_layout_left() {
        let state = SidebarState::new(Side::Left).with_width(Constraint::Percentage(25));

        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let [main, sidebar] = Sidebar::new(&state, area);

        assert_eq!(sidebar.width, 25); // 25% of 100
        assert_eq!(main.width, 75);
        assert_eq!(sidebar.x, 0);
        assert_eq!(main.x, 25);
    }

    #[test]
    fn test_sidebar_layout_right() {
        let state = SidebarState::new(Side::Right).with_width(Constraint::Percentage(25));

        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let [main, sidebar] = Sidebar::new(&state, area);

        assert_eq!(sidebar.width, 25); // 25% of 100
        assert_eq!(main.width, 75);
        assert_eq!(main.x, 0);
        assert_eq!(sidebar.x, 75);
    }

    #[test]
    fn test_hidden_sidebar() {
        let mut state = SidebarState::default();
        state.hide();

        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let [main, sidebar] = Sidebar::new(&state, area);

        assert_eq!(main.width, 100);
        assert_eq!(sidebar.width, 0);
    }
}
