//! Theme system for consistent styling across components.
//!
//! The theme provides semantic color roles that components can use
//! for consistent styling. Themes can be customized and swapped at runtime.
//!
//! # Example
//!
//! ```ignore
//! use tabitha::{AppBuilder, Theme};
//!
//! let app = AppBuilder::new()
//!     .main_ui(MyApp)
//!     .with_theme(Theme::default())
//!     .build()?;
//! ```

use ratatui::style::{Color, Style};

/// A theme containing semantic color roles for UI styling.
///
/// Themes define colors for various UI states and elements:
/// - Base colors for background and text
/// - Accent colors for primary actions and highlights
/// - Secondary colors for less prominent elements
/// - Muted colors for disabled or inactive states
/// - Border colors for different focus states
/// - Highlight colors for selection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Primary background color.
    pub background: Color,
    /// Primary foreground/text color.
    pub foreground: Color,

    /// Accent color for primary actions and focus.
    pub accent: Color,
    /// Foreground color when on accent background.
    pub accent_foreground: Color,

    /// Secondary color for less prominent elements.
    pub secondary: Color,
    /// Foreground color when on secondary background.
    pub secondary_foreground: Color,

    /// Muted/disabled background color.
    pub muted: Color,
    /// Muted/disabled foreground color.
    pub muted_foreground: Color,

    /// Default border color.
    pub border: Color,
    /// Border color when focused.
    pub border_focused: Color,

    /// Highlight/selection background color.
    pub highlight: Color,
    /// Foreground color when on highlight background.
    pub highlight_foreground: Color,
}

impl Default for Theme {
    /// Returns a default dark theme.
    fn default() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,

            accent: Color::Blue,
            accent_foreground: Color::White,

            secondary: Color::DarkGray,
            secondary_foreground: Color::White,

            muted: Color::DarkGray,
            muted_foreground: Color::Gray,

            border: Color::DarkGray,
            border_focused: Color::Yellow,

            highlight: Color::Yellow,
            highlight_foreground: Color::Black,
        }
    }
}

impl Theme {
    /// Create a new theme with the given colors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a dimmed/grayscale version of this theme.
    ///
    /// This is useful for rendering background content when a modal
    /// is displayed, making the modal stand out.
    pub fn dimmed(&self) -> Self {
        Self {
            background: self.background,
            foreground: to_grayscale(self.foreground),

            accent: to_grayscale(self.accent),
            accent_foreground: to_grayscale(self.accent_foreground),

            secondary: to_grayscale(self.secondary),
            secondary_foreground: to_grayscale(self.secondary_foreground),

            muted: to_grayscale(self.muted),
            muted_foreground: to_grayscale(self.muted_foreground),

            border: to_grayscale(self.border),
            border_focused: to_grayscale(self.border_focused),

            highlight: to_grayscale(self.highlight),
            highlight_foreground: to_grayscale(self.highlight_foreground),
        }
    }

    // --- Convenience style methods ---

    /// Returns a style with the base foreground and background colors.
    pub fn style(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    /// Returns a style for accent elements.
    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent_foreground).bg(self.accent)
    }

    /// Returns a style for secondary elements.
    pub fn secondary_style(&self) -> Style {
        Style::default()
            .fg(self.secondary_foreground)
            .bg(self.secondary)
    }

    /// Returns a style for muted/disabled elements.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.muted_foreground).bg(self.muted)
    }

    /// Returns a style for highlighted/selected elements.
    pub fn highlight_style(&self) -> Style {
        Style::default()
            .fg(self.highlight_foreground)
            .bg(self.highlight)
    }

    /// Returns a style for borders (unfocused).
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Returns a style for focused borders.
    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.border_focused)
    }

    /// Returns a style with just the foreground color.
    pub fn fg_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    /// Returns a style with just the accent foreground color (no background).
    pub fn accent_fg_style(&self) -> Style {
        Style::default().fg(self.accent_foreground)
    }

    /// Returns a style with just the background color.
    pub fn bg_style(&self) -> Style {
        Style::default().bg(self.background)
    }
}

/// Convert a color to its grayscale equivalent.
fn to_grayscale(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            // Use standard luminance formula (use u32 to avoid overflow)
            let gray = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
            Color::Rgb(gray, gray, gray)
        }
        // For indexed colors, map to grayscale equivalents
        Color::Black => Color::Black,
        Color::White => Color::DarkGray,
        Color::Red => Color::DarkGray,
        Color::Green => Color::DarkGray,
        Color::Yellow => Color::Gray,
        Color::Blue => Color::DarkGray,
        Color::Magenta => Color::DarkGray,
        Color::Cyan => Color::Gray,
        Color::Gray => Color::Gray,
        Color::DarkGray => Color::DarkGray,
        Color::LightRed => Color::Gray,
        Color::LightGreen => Color::Gray,
        Color::LightYellow => Color::Gray,
        Color::LightBlue => Color::Gray,
        Color::LightMagenta => Color::Gray,
        Color::LightCyan => Color::Gray,
        // For other colors, return as-is
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme() {
        let theme = Theme::default();
        assert_eq!(theme.background, Color::Black);
        assert_eq!(theme.foreground, Color::White);
        assert_eq!(theme.accent, Color::Blue);
    }

    #[test]
    fn test_dimmed_theme() {
        let theme = Theme::default();
        let dimmed = theme.dimmed();

        // Background should stay the same
        assert_eq!(dimmed.background, theme.background);

        // Colors should be converted to grayscale
        assert_eq!(dimmed.foreground, Color::DarkGray); // White -> DarkGray
        assert_eq!(dimmed.accent, Color::DarkGray); // Blue -> DarkGray
    }

    #[test]
    fn test_style_methods() {
        let theme = Theme::default();

        let style = theme.style();
        assert_eq!(style.fg, Some(theme.foreground));
        assert_eq!(style.bg, Some(theme.background));

        let accent = theme.accent_style();
        assert_eq!(accent.fg, Some(theme.accent_foreground));
        assert_eq!(accent.bg, Some(theme.accent));
    }

    #[test]
    fn test_rgb_grayscale() {
        // Test RGB color grayscale conversion
        let gray = to_grayscale(Color::Rgb(255, 0, 0)); // Pure red
        if let Color::Rgb(r, g, b) = gray {
            assert_eq!(r, g);
            assert_eq!(g, b);
        } else {
            panic!("Expected RGB color");
        }
    }
}
