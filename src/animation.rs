//! Animation framework for the TUI library.
//!
//! This module provides a framework for animations with zero CPU overhead when paused.
//!
//! # Key Features
//!
//! - **Zero CPU when paused**: When animations are disabled, no tick events are generated
//! - **Global speed control**: Adjust animation speed for all animations at once
//! - **Built-in animations**: Blink and Spinner animations included
//!
//! # Example
//!
//! ```ignore
//! use tabitha::animation::{AnimationController, BlinkAnimation};
//! use std::time::Duration;
//!
//! let mut controller = AnimationController::new();
//!
//! // Add a blinking animation
//! let blink = BlinkAnimation::new(Duration::from_millis(500));
//! controller.add("cursor", blink);
//!
//! // In your event loop
//! if controller.tick(Duration::from_millis(16)) {
//!     // Request redraw - some animation changed
//! }
//! ```

use std::collections::HashMap;
use std::time::Duration;

/// Animation mode for controlling animation intensity across the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationMode {
    /// Full animations - all effects enabled at normal speed
    #[default]
    Full,
    /// Reduced animations - minimal effects, faster transitions
    Reduced,
    /// No animations - instant transitions, static display
    None,
}

impl AnimationMode {
    /// Check if animations are completely disabled
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Check if in reduced mode
    pub fn is_reduced(&self) -> bool {
        matches!(self, Self::Reduced)
    }

    /// Get speed multiplier for this mode
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Self::Full => 1.0,
            Self::Reduced => 2.0, // 2x faster
            Self::None => 0.0,
        }
    }
}

/// Trait for animations that can be managed by the AnimationController.
///
/// Implement this trait to create custom animations.
pub trait Animation: Send {
    /// Update animation state with adjusted elapsed time.
    ///
    /// Returns `true` if the animation state changed and a redraw is needed.
    fn tick(&mut self, elapsed: Duration) -> bool;

    /// Reset animation to initial state.
    fn reset(&mut self);

    /// Check if the animation is currently visible/active.
    ///
    /// Default implementation always returns `true`.
    fn is_visible(&self) -> bool {
        true
    }

    /// Check if the animation is complete.
    ///
    /// Default implementation returns `false` (animations loop forever).
    fn is_complete(&self) -> bool {
        false
    }

    /// Pause the animation.
    ///
    /// Default implementation does nothing.
    fn pause(&mut self) {
        // Default: no-op
    }

    /// Resume the animation.
    ///
    /// Default implementation does nothing.
    fn resume(&mut self) {
        // Default: no-op
    }

    /// Check if the animation is paused.
    ///
    /// Default implementation returns `false`.
    fn is_paused(&self) -> bool {
        false
    }

    /// Set the animation progress directly (0.0 to 1.0).
    ///
    /// Default implementation does nothing.
    fn set_progress(&mut self, _progress: f32) {
        // Default: no-op
    }

    /// Get the current progress (0.0 to 1.0).
    ///
    /// Default implementation returns 0.0.
    fn progress(&self) -> f32 {
        0.0
    }

    /// Get the current color.
    ///
    /// Returns `None` for animations without color.
    fn current_color(&self) -> Option<ratatui::style::Color> {
        None
    }

    /// Set hold durations for full and dim states.
    ///
    /// Default implementation does nothing.
    fn set_hold_durations(&mut self, _full_duration: Duration, _dim_duration: Duration) {}
}

/// Global controller for all animations in the app.
///
/// Key feature: **Zero CPU when paused** - no tick events generated when disabled.
pub struct AnimationController {
    /// Whether animations are currently enabled.
    enabled: bool,
    /// Global speed multiplier (1.0 = normal, 0.5 = half speed, 2.0 = double).
    speed_multiplier: f32,
    /// Animation mode for controlling intensity.
    mode: AnimationMode,
    /// Registered animations by name.
    animations: HashMap<String, Box<dyn Animation>>,
}

impl std::fmt::Debug for AnimationController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnimationController")
            .field("enabled", &self.enabled)
            .field("speed_multiplier", &self.speed_multiplier)
            .field("mode", &self.mode)
            .field("animations", &self.animations.len())
            .finish()
    }
}

impl AnimationController {
    /// Create a new animation controller.
    ///
    /// Animations are enabled by default in Full mode.
    pub fn new() -> Self {
        Self {
            enabled: true,
            speed_multiplier: 1.0,
            mode: AnimationMode::Full,
            animations: HashMap::new(),
        }
    }

    /// Pause all animations (zero CPU usage).
    ///
    /// When paused, `tick()` returns `false` immediately without processing.
    pub fn pause(&mut self) {
        self.enabled = false;
    }

    /// Resume all animations.
    pub fn resume(&mut self) {
        self.enabled = true;
    }

    /// Check if animations are currently enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.mode.is_disabled()
    }

    /// Toggle animation state.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Get the current animation mode.
    pub fn mode(&self) -> AnimationMode {
        self.mode
    }

    /// Set the animation mode.
    pub fn set_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
        // Auto-enable/disable based on mode
        self.enabled = !mode.is_disabled();
    }

    /// Cycle to the next animation mode (Full -> Reduced -> None -> Full).
    pub fn cycle_mode(&mut self) -> AnimationMode {
        self.mode = match self.mode {
            AnimationMode::Full => AnimationMode::Reduced,
            AnimationMode::Reduced => AnimationMode::None,
            AnimationMode::None => AnimationMode::Full,
        };
        self.enabled = !self.mode.is_disabled();
        self.mode
    }

    /// Set global speed multiplier.
    ///
    /// Affects all animations. Use `1.0` for normal speed, `0.5` for half,
    /// `2.0` for double.
    pub fn set_speed(&mut self, multiplier: f32) {
        self.speed_multiplier = multiplier.clamp(0.1, 10.0);
    }

    /// Get current speed multiplier.
    pub fn speed_multiplier(&self) -> f32 {
        self.speed_multiplier
    }

    /// Register an animation.
    ///
    /// If an animation with this ID already exists, it is replaced.
    pub fn add(&mut self, id: impl Into<String>, animation: impl Animation + 'static) {
        self.animations.insert(id.into(), Box::new(animation));
    }

    /// Remove an animation by ID.
    ///
    /// Returns `true` if the animation was found and removed.
    pub fn remove(&mut self, id: &str) -> bool {
        self.animations.remove(id).is_some()
    }

    /// Get a reference to an animation.
    pub fn get(&self, id: &str) -> Option<&dyn Animation> {
        self.animations.get(id).map(|a| a.as_ref())
    }

    /// Get a mutable reference to an animation.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut (dyn Animation + '_)> {
        if let Some(a) = self.animations.get_mut(id) {
            Some(a.as_mut())
        } else {
            None
        }
    }

    /// Check if an animation exists.
    pub fn contains(&self, id: &str) -> bool {
        self.animations.contains_key(id)
    }

    /// Get number of registered animations.
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Check if no animations are registered.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Clear all animations.
    pub fn clear(&mut self) {
        self.animations.clear();
    }

    /// Tick all animations.
    ///
    /// Returns `true` if any animation changed state and needs a redraw.
    ///
    /// **When paused, this returns `false` immediately** (zero CPU).
    pub fn tick(&mut self, elapsed: Duration) -> bool {
        // Zero CPU when paused or disabled
        if !self.enabled || self.mode.is_disabled() {
            return false;
        }

        // Combine mode speed multiplier with global speed multiplier
        let total_multiplier = self.speed_multiplier * self.mode.speed_multiplier();

        // Apply combined speed multiplier
        let adjusted_elapsed = if total_multiplier == 1.0 {
            elapsed
        } else {
            Duration::from_nanos((elapsed.as_nanos() as f32 * total_multiplier) as u64)
        };

        // Tick all animations
        let mut needs_redraw = false;
        for animation in self.animations.values_mut() {
            if animation.tick(adjusted_elapsed) {
                needs_redraw = true;
            }
        }

        needs_redraw
    }

    /// Check if the controller needs periodic ticking.
    ///
    /// Returns `true` if animations are enabled and any animation
    /// is active (not paused and not complete).
    pub fn needs_ticking(&self) -> bool {
        self.is_enabled()
            && self
                .animations
                .values()
                .any(|a| !a.is_paused() && !a.is_complete())
    }

    /// Reset all animations to initial state.
    pub fn reset_all(&mut self) {
        for animation in self.animations.values_mut() {
            animation.reset();
        }
    }

    /// Get an iterator over animation IDs.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.animations.keys().map(|s| s.as_str())
    }

    /// Pause a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn pause_animation(&mut self, id: &str) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.pause();
            true
        } else {
            false
        }
    }

    /// Resume a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn resume_animation(&mut self, id: &str) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.resume();
            true
        } else {
            false
        }
    }

    /// Set progress for a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn set_animation_progress(&mut self, id: &str, progress: f32) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.set_progress(progress);
            true
        } else {
            false
        }
    }

    /// Get the current color for a fade animation.
    ///
    /// Returns `None` if the animation doesn't exist or isn't a fade animation.
    pub fn current_color(&self, id: &str) -> Option<ratatui::style::Color> {
        self.animations.get(id).and_then(|a| a.current_color())
    }

    /// Check if an animation is paused.
    ///
    /// Returns `None` if the animation doesn't exist.
    pub fn is_animation_paused(&self, id: &str) -> Option<bool> {
        self.animations.get(id).map(|a| a.is_paused())
    }

    /// Set hold durations for a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn set_animation_hold_durations(
        &mut self,
        id: &str,
        full_duration: Duration,
        dim_duration: Duration,
    ) -> bool {
        if let Some(anim) = self.animations.get_mut(id) {
            anim.set_hold_durations(full_duration, dim_duration);
            true
        } else {
            false
        }
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new()
    }
}

/// Animation context available during event handling.
///
/// Access through `AppContext::animations()`.
pub struct AnimationEventContext<'a> {
    controller: &'a mut AnimationController,
}

impl<'a> AnimationEventContext<'a> {
    /// Create a new animation context.
    pub(crate) fn new(controller: &'a mut AnimationController) -> Self {
        Self { controller }
    }

    /// Check if animations are enabled.
    pub fn is_enabled(&self) -> bool {
        self.controller.is_enabled()
    }

    /// Pause animations.
    pub fn pause(&mut self) {
        self.controller.pause();
    }

    /// Resume animations.
    pub fn resume(&mut self) {
        self.controller.resume();
    }

    /// Toggle animation state.
    pub fn toggle(&mut self) {
        self.controller.toggle();
    }

    /// Set global speed multiplier.
    pub fn set_speed(&mut self, multiplier: f32) {
        self.controller.set_speed(multiplier);
    }

    /// Get speed multiplier.
    pub fn speed_multiplier(&self) -> f32 {
        self.controller.speed_multiplier()
    }

    /// Register an animation.
    pub fn add(&mut self, id: impl Into<String>, animation: impl Animation + 'static) {
        self.controller.add(id, animation);
    }

    /// Remove an animation.
    pub fn remove(&mut self, id: &str) -> bool {
        self.controller.remove(id)
    }

    /// Get an animation.
    pub fn get(&self, id: &str) -> Option<&dyn Animation> {
        self.controller.get(id)
    }

    /// Get a mutable animation.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut (dyn Animation + '_)> {
        self.controller.get_mut(id)
    }

    /// Check if animation exists.
    pub fn contains(&self, id: &str) -> bool {
        self.controller.contains(id)
    }

    /// Reset all animations.
    pub fn reset_all(&mut self) {
        self.controller.reset_all();
    }

    /// Clear all animations.
    pub fn clear(&mut self) {
        self.controller.clear();
    }

    /// Get number of animations.
    pub fn len(&self) -> usize {
        self.controller.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.controller.is_empty()
    }
}

/// Animation context available to controls during tick() for per-animation operations.
///
/// This context provides control over individual animations, allowing controls
/// to pause, resume, and query animation state.
///
/// Access this through `AnimationContext` passed to the `tick()` method.
pub struct ControlAnimationContext<'a> {
    controller: &'a mut AnimationController,
}

impl<'a> ControlAnimationContext<'a> {
    /// Create a new control animation context.
    pub fn new(controller: &'a mut AnimationController) -> Self {
        Self { controller }
    }

    /// Pause a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn pause(&mut self, id: &str) -> bool {
        self.controller.pause_animation(id)
    }

    /// Resume a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn resume(&mut self, id: &str) -> bool {
        self.controller.resume_animation(id)
    }

    /// Set the progress for a specific animation.
    ///
    /// Progress is clamped to 0.0-1.0 range.
    /// Returns `true` if the animation was found.
    pub fn set_progress(&mut self, id: &str, progress: f32) -> bool {
        self.controller.set_animation_progress(id, progress)
    }

    /// Get the current color for a fade animation.
    ///
    /// Returns `None` if the animation doesn't exist or isn't a fade animation.
    pub fn current_color(&self, id: &str) -> Option<ratatui::style::Color> {
        self.controller.current_color(id)
    }

    /// Check if an animation is paused.
    ///
    /// Returns `None` if the animation doesn't exist.
    pub fn is_paused(&self, id: &str) -> Option<bool> {
        self.controller.is_animation_paused(id)
    }

    /// Set hold durations for a specific animation.
    ///
    /// Returns `true` if the animation was found.
    pub fn set_hold_durations(
        &mut self,
        id: &str,
        full_duration: Duration,
        dim_duration: Duration,
    ) -> bool {
        self.controller
            .set_animation_hold_durations(id, full_duration, dim_duration)
    }

    /// Register a new animation with the controller.
    pub fn add(&mut self, id: impl Into<String>, animation: impl Animation + 'static) {
        self.controller.add(id, animation);
    }

    /// Check if an animation exists.
    pub fn contains(&self, id: &str) -> bool {
        self.controller.contains(id)
    }

    /// Check if animations are globally enabled.
    pub fn is_enabled(&self) -> bool {
        self.controller.is_enabled()
    }

    /// Get the global speed multiplier.
    pub fn speed_multiplier(&self) -> f32 {
        self.controller.speed_multiplier()
    }

    /// Get the current animation mode.
    pub fn mode(&self) -> AnimationMode {
        self.controller.mode()
    }

    /// Set the animation mode.
    pub fn set_mode(&mut self, mode: AnimationMode) {
        self.controller.set_mode(mode);
    }

    /// Cycle to the next animation mode (Full -> Reduced -> None -> Full).
    pub fn cycle_mode(&mut self) -> AnimationMode {
        self.controller.cycle_mode()
    }
}

// =============================================================================
// Standard Animations
// =============================================================================

/// A simple blinking animation.
///
/// Toggles between visible and hidden states at regular intervals.
pub struct BlinkAnimation {
    visible: bool,
    elapsed: Duration,
    interval: Duration,
}

impl BlinkAnimation {
    /// Create a new blink animation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::animation::BlinkAnimation;
    /// use std::time::Duration;
    ///
    /// // Blink every 500ms
    /// let blink = BlinkAnimation::new(Duration::from_millis(500));
    /// ```
    pub fn new(interval: Duration) -> Self {
        Self {
            visible: true,
            elapsed: Duration::ZERO,
            interval,
        }
    }

    /// Create with custom initial state.
    pub fn with_initial_state(interval: Duration, initially_visible: bool) -> Self {
        Self {
            visible: initially_visible,
            elapsed: Duration::ZERO,
            interval,
        }
    }

    /// Check if currently visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set blink interval.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Get current interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

impl Animation for BlinkAnimation {
    fn tick(&mut self, elapsed: Duration) -> bool {
        self.elapsed += elapsed;

        if self.elapsed >= self.interval {
            self.elapsed -= self.interval;
            self.visible = !self.visible;
            true // State changed, needs redraw
        } else {
            false // No change
        }
    }

    fn reset(&mut self) {
        self.visible = true;
        self.elapsed = Duration::ZERO;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl Default for BlinkAnimation {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

/// A spinner animation with rotating characters.
///
/// Cycles through a sequence of characters at regular intervals.
pub struct SpinnerAnimation {
    frames: &'static [char],
    current: usize,
    elapsed: Duration,
    interval: Duration,
}

impl SpinnerAnimation {
    /// Create a new spinner animation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::animation::SpinnerAnimation;
    /// use std::time::Duration;
    ///
    /// // Classic spinner
    /// let spinner = SpinnerAnimation::classic(Duration::from_millis(100));
    ///
    /// // Custom frames
    /// const FRAMES: &[char] = &['◐', '◓', '◑', '◒'];
    /// let spinner = SpinnerAnimation::new(FRAMES, Duration::from_millis(100));
    /// ```
    pub fn new(frames: &'static [char], interval: Duration) -> Self {
        Self {
            frames,
            current: 0,
            elapsed: Duration::ZERO,
            interval,
        }
    }

    /// Create a classic spinner: `|/-\`
    pub fn classic(interval: Duration) -> Self {
        const FRAMES: &[char] = &['|', '/', '-', '\\'];
        Self::new(FRAMES, interval)
    }

    /// Create a dot spinner: `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`
    pub fn dots(interval: Duration) -> Self {
        const FRAMES: &[char] = &['�', '�', '�', '⠑', '⠉', '⠈', '⠐', '⠠'];
        Self::new(FRAMES, interval)
    }

    /// Create a circle spinner: `◐◓◑◒`
    pub fn circle(interval: Duration) -> Self {
        const FRAMES: &[char] = &['◐', '◓', '◑', '◒'];
        Self::new(FRAMES, interval)
    }

    /// Get current frame character.
    pub fn current(&self) -> char {
        self.frames[self.current]
    }

    /// Set animation interval.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Get current interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Get frame count.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

impl Animation for SpinnerAnimation {
    fn tick(&mut self, elapsed: Duration) -> bool {
        self.elapsed += elapsed;

        if self.elapsed >= self.interval {
            self.elapsed -= self.interval;
            self.current = (self.current + 1) % self.frames.len();
            true // State changed, needs redraw
        } else {
            false // No change
        }
    }

    fn reset(&mut self) {
        self.current = 0;
        self.elapsed = Duration::ZERO;
    }
}

impl Default for SpinnerAnimation {
    fn default() -> Self {
        Self::classic(Duration::from_millis(100))
    }
}

/// Direction of fade animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeDirection {
    /// Fading in (from dim to full brightness)
    In,
    /// Fading out (from full brightness to dim)
    Out,
}

/// A fade animation that interpolates between two RGB colors.
///
/// This creates smooth color transitions by interpolating between
/// a dim color and a full color over a specified duration.
pub struct FadeAnimation {
    /// Current progress from 0.0 (dim) to 1.0 (full)
    progress: f32,
    /// Elapsed time since last progress update
    elapsed: Duration,
    /// Duration for fading in (dim to full)
    fade_in_duration: Duration,
    /// Duration for fading out (full to dim)
    fade_out_duration: Duration,
    /// Current fade direction
    direction: FadeDirection,
    /// Color at 0% progress (dim)
    dim_color: (u8, u8, u8),
    /// Color at 100% progress (full brightness)
    full_color: (u8, u8, u8),
    /// Whether animation loops (fade in then out)
    loop_animation: bool,
    /// Whether the animation is paused
    paused: bool,
    /// Hold time remaining (when at dim/full before switching)
    hold_remaining: Option<Duration>,
    /// Duration to hold at full brightness after fade in
    hold_full_duration: Duration,
    /// Duration to hold at dim brightness after fade out
    hold_dim_duration: Duration,
}

impl FadeAnimation {
    /// Create a new fade animation with RGB colors.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::animation::{FadeAnimation, FadeDirection};
    /// use std::time::Duration;
    ///
    /// // Fade from dim gray to bright white over 300ms
    /// let fade = FadeAnimation::new(
    ///     (50, 50, 50),     // dim gray
    ///     (255, 255, 255),  // bright white
    ///     Duration::from_millis(300),
    /// );
    /// ```
    pub fn new(dim_color: (u8, u8, u8), full_color: (u8, u8, u8), duration: Duration) -> Self {
        Self {
            progress: 0.0,
            elapsed: Duration::ZERO,
            fade_in_duration: duration,
            fade_out_duration: duration,
            direction: FadeDirection::In,
            dim_color,
            full_color,
            loop_animation: true,
            paused: false,
            hold_remaining: None,
            hold_full_duration: Duration::ZERO,
            hold_dim_duration: Duration::ZERO,
        }
    }

    /// Create a new fade animation with hold times at dim and full.
    ///
    /// This is useful for cursor animations that should linger at
    /// full brightness before fading out.
    pub fn with_hold_times(
        dim_color: (u8, u8, u8),
        full_color: (u8, u8, u8),
        fade_in_duration: Duration,
        hold_full: Duration,
        hold_dim: Duration,
    ) -> Self {
        Self {
            progress: 0.0,
            elapsed: Duration::ZERO,
            fade_in_duration,
            fade_out_duration: fade_in_duration,
            direction: FadeDirection::In,
            dim_color,
            full_color,
            loop_animation: true,
            paused: false,
            hold_remaining: None,
            hold_full_duration: hold_full,
            hold_dim_duration: hold_dim,
        }
    }

    /// Set the fade out duration separately from fade in.
    pub fn set_fade_out_duration(&mut self, duration: Duration) {
        self.fade_out_duration = duration;
    }

    /// Get the fade in duration.
    pub fn fade_in_duration(&self) -> Duration {
        self.fade_in_duration
    }

    /// Get the fade out duration.
    pub fn fade_out_duration(&self) -> Duration {
        self.fade_out_duration
    }

    /// Create a fade animation from theme colors.
    ///
    /// Automatically calculates the dim color at 50% brightness
    /// from the provided full color.
    pub fn from_theme_color(full_color: ratatui::style::Color, duration: Duration) -> Self {
        let (r, g, b) = match full_color {
            ratatui::style::Color::Rgb(r, g, b) => (r, g, b),
            _ => (255, 255, 255), // Default to white for non-RGB colors
        };

        let dim_color = (r / 2, g / 2, b / 2);
        Self::new(dim_color, (r, g, b), duration)
    }

    /// Pause the animation at its current state.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume the animation from its current state.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if the animation is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set the progress directly (0.0 to 1.0).
    ///
    /// This is useful for forcing a specific brightness level,
    /// such as when typing pauses the cursor animation at full brightness.
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
        self.elapsed = Duration::ZERO;
    }

    /// Set the hold durations for full and dim states.
    pub fn set_hold_durations(&mut self, hold_full: Duration, hold_dim: Duration) {
        self.hold_full_duration = hold_full;
        self.hold_dim_duration = hold_dim;
    }

    /// Get the current interpolated color with direction-aware easing.
    pub fn current_color(&self) -> ratatui::style::Color {
        let (dr, dg, db) = self.dim_color;
        let (fr, fg, fb) = self.full_color;

        let fading_in = matches!(self.direction, FadeDirection::In);

        let r = Self::lerp_eased(dr, fr, self.progress, fading_in);
        let g = Self::lerp_eased(dg, fg, self.progress, fading_in);
        let b = Self::lerp_eased(db, fb, self.progress, fading_in);

        ratatui::style::Color::Rgb(r, g, b)
    }

    /// Get the current progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Get the current fade direction.
    pub fn direction(&self) -> FadeDirection {
        self.direction
    }

    /// Set whether the animation should loop.
    pub fn set_loop(&mut self, loop_animation: bool) {
        self.loop_animation = loop_animation;
    }

    /// Check if animation loops.
    pub fn is_looping(&self) -> bool {
        self.loop_animation
    }

    /// Linear interpolation between two values.
    fn lerp(start: u8, end: u8, t: f32) -> u8 {
        let start_f = start as f32;
        let end_f = end as f32;
        let result = start_f + (end_f - start_f) * t;
        result.clamp(0.0, 255.0) as u8
    }

    /// Ease-out: fast start, slow end (for fade in).
    /// Quadratic ease-out for snappy start from zero and smooth landing at max.
    fn ease_out(t: f32) -> f32 {
        1.0 - (1.0 - t) * (1.0 - t)
    }

    /// Ease-in: slow start, fast end (for fade out).
    /// Quadratic ease-in for gentle start from max and quick exit to zero.
    fn ease_in(t: f32) -> f32 {
        t * t
    }

    /// Apply appropriate easing based on direction.
    fn lerp_eased(start: u8, end: u8, t: f32, fading_in: bool) -> u8 {
        let eased_t = if fading_in {
            Self::ease_out(t) // Fast from zero, slow to max
        } else {
            Self::ease_in(t) // Slow from max, fast to zero
        };
        Self::lerp(start, end, eased_t)
    }
}

impl Animation for FadeAnimation {
    fn tick(&mut self, elapsed: Duration) -> bool {
        // When paused, don't update but still return true if we have progress
        // to show (needed for initial rendering after pause)
        if self.paused {
            return false;
        }

        // Handle hold state first
        if let Some(hold_remaining) = self.hold_remaining {
            if elapsed >= hold_remaining {
                // Hold complete, start fading in opposite direction
                self.hold_remaining = None;
                self.elapsed = elapsed - hold_remaining;
                self.direction = match self.direction {
                    FadeDirection::In => FadeDirection::Out,
                    FadeDirection::Out => FadeDirection::In,
                };
                self.progress = match self.direction {
                    FadeDirection::In => 0.0,
                    FadeDirection::Out => 1.0,
                };
                return true;
            } else {
                // Still holding
                self.hold_remaining = Some(hold_remaining - elapsed);
                return false;
            }
        }

        self.elapsed += elapsed;

        // Get the appropriate duration based on direction
        let current_duration = match self.direction {
            FadeDirection::In => self.fade_in_duration,
            FadeDirection::Out => self.fade_out_duration,
        };

        if self.elapsed >= current_duration {
            // Fade complete, check if we should hold or switch immediately
            self.elapsed -= current_duration;

            let hold_duration = match self.direction {
                FadeDirection::In => self.hold_full_duration,
                FadeDirection::Out => self.hold_dim_duration,
            };

            if hold_duration > Duration::ZERO && self.loop_animation {
                // Enter hold state
                self.hold_remaining = Some(hold_duration);
                self.progress = match self.direction {
                    FadeDirection::In => 1.0,
                    FadeDirection::Out => 0.0,
                };
                true
            } else {
                // Switch immediately
                match self.direction {
                    FadeDirection::In => {
                        self.progress = 1.0;
                        if self.loop_animation {
                            self.direction = FadeDirection::Out;
                        }
                    }
                    FadeDirection::Out => {
                        self.progress = 0.0;
                        if self.loop_animation {
                            self.direction = FadeDirection::In;
                        }
                    }
                }
                true
            }
        } else {
            // Update progress within current phase
            let t = self.elapsed.as_secs_f32() / current_duration.as_secs_f32();
            self.progress = match self.direction {
                FadeDirection::In => t,
                FadeDirection::Out => 1.0 - t,
            };
            true // Always needs redraw during fade for smoothness
        }
    }

    fn reset(&mut self) {
        self.progress = 0.0;
        self.elapsed = Duration::ZERO;
        self.direction = FadeDirection::In;
        self.hold_remaining = None;
        self.paused = false;
    }

    fn is_visible(&self) -> bool {
        // Always visible during fade, just at different brightness
        true
    }

    fn is_complete(&self) -> bool {
        // Only complete if not looping and we're at end of fade out
        !self.loop_animation && self.direction == FadeDirection::Out && self.progress <= 0.0
    }

    fn pause(&mut self) {
        self.paused = true;
    }

    fn resume(&mut self) {
        self.paused = false;
    }

    fn is_paused(&self) -> bool {
        self.paused
    }

    fn set_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
        self.elapsed = Duration::ZERO;
    }

    fn progress(&self) -> f32 {
        self.progress
    }

    fn current_color(&self) -> Option<ratatui::style::Color> {
        Some(self.current_color())
    }

    fn set_hold_durations(&mut self, full_duration: Duration, dim_duration: Duration) {
        self.hold_full_duration = full_duration;
        self.hold_dim_duration = dim_duration;
    }
}

impl Default for FadeAnimation {
    fn default() -> Self {
        Self {
            progress: 0.0,
            elapsed: Duration::ZERO,
            fade_in_duration: Duration::from_millis(300),
            fade_out_duration: Duration::from_millis(300),
            direction: FadeDirection::In,
            dim_color: (50, 50, 50),
            full_color: (255, 255, 255),
            loop_animation: true,
            paused: false,
            hold_remaining: None,
            hold_full_duration: Duration::ZERO,
            hold_dim_duration: Duration::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_controller_enabled_by_default() {
        let controller = AnimationController::new();
        assert!(controller.is_enabled());
        assert_eq!(controller.speed_multiplier(), 1.0);
    }

    #[test]
    fn test_animation_controller_pause_resume() {
        let mut controller = AnimationController::new();

        controller.pause();
        assert!(!controller.is_enabled());

        controller.resume();
        assert!(controller.is_enabled());
    }

    #[test]
    fn test_animation_controller_zero_cpu_when_paused() {
        let mut controller = AnimationController::new();
        controller.add("test", BlinkAnimation::default());

        // When paused, tick should return false immediately
        controller.pause();
        assert!(!controller.tick(Duration::from_millis(1000)));
    }

    #[test]
    fn test_blink_animation() {
        let mut blink = BlinkAnimation::new(Duration::from_millis(100));

        // Initially visible
        assert!(blink.is_visible());

        // After 50ms, still visible
        assert!(!blink.tick(Duration::from_millis(50)));
        assert!(blink.is_visible());

        // After another 60ms (total 110ms), should toggle
        assert!(blink.tick(Duration::from_millis(60)));
        assert!(!blink.is_visible());

        // After another 100ms, should toggle back
        assert!(blink.tick(Duration::from_millis(100)));
        assert!(blink.is_visible());
    }

    #[test]
    fn test_blink_animation_reset() {
        let mut blink = BlinkAnimation::new(Duration::from_millis(100));

        // Toggle to invisible
        blink.tick(Duration::from_millis(150));
        assert!(!blink.is_visible());

        // Reset
        blink.reset();
        assert!(blink.is_visible());
    }

    #[test]
    fn test_spinner_animation() {
        let mut spinner = SpinnerAnimation::classic(Duration::from_millis(100));

        // Initially at frame 0
        assert_eq!(spinner.current(), '|');

        // After 100ms, advance to next frame
        assert!(spinner.tick(Duration::from_millis(100)));
        assert_eq!(spinner.current(), '/');

        // After another 100ms, advance to next frame
        assert!(spinner.tick(Duration::from_millis(100)));
        assert_eq!(spinner.current(), '-');

        // After another 100ms, advance to next frame
        assert!(spinner.tick(Duration::from_millis(100)));
        assert_eq!(spinner.current(), '\\');

        // After another 100ms, wrap back to first frame
        assert!(spinner.tick(Duration::from_millis(100)));
        assert_eq!(spinner.current(), '|');
    }

    #[test]
    fn test_speed_multiplier() {
        let mut controller = AnimationController::new();
        controller.add("blink", BlinkAnimation::new(Duration::from_millis(100)));

        // Set half speed - animations should take twice as long
        controller.set_speed(0.5);
        assert_eq!(controller.speed_multiplier(), 0.5);

        // At half speed, 100ms of real time = 50ms of animation time
        // So after 100ms, no change yet
        assert!(!controller.tick(Duration::from_millis(100)));

        // After another 100ms (total 200ms real = 100ms animation), should change
        assert!(controller.tick(Duration::from_millis(100)));
    }

    #[test]
    fn test_animation_registration() {
        let mut controller = AnimationController::new();

        assert!(!controller.contains("test"));
        assert_eq!(controller.len(), 0);

        controller.add("test", BlinkAnimation::default());

        assert!(controller.contains("test"));
        assert_eq!(controller.len(), 1);

        controller.remove("test");

        assert!(!controller.contains("test"));
        assert_eq!(controller.len(), 0);
    }
}
