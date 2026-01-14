//! Terminal management for the TUI framework.
//!
//! This module handles raw mode setup/teardown and provides a safe wrapper
//! around the ratatui terminal.

use std::io::{self, Stdout};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal as RatatuiTerminal};

/// Error type for terminal operations
#[derive(Debug)]
pub enum TerminalError {
    /// IO error from crossterm or ratatui
    Io(io::Error),
}

impl std::fmt::Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalError::Io(e) => write!(f, "Terminal IO error: {}", e),
        }
    }
}

impl std::error::Error for TerminalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TerminalError::Io(e) => Some(e),
        }
    }
}

impl From<io::Error> for TerminalError {
    fn from(err: io::Error) -> Self {
        TerminalError::Io(err)
    }
}

/// Configuration for terminal initialization.
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    /// Whether to enable mouse capture. Default: `true`.
    pub mouse_capture: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            mouse_capture: true,
        }
    }
}

/// Terminal wrapper that manages raw mode and alternate screen.
///
/// This struct ensures proper cleanup on drop, restoring the terminal
/// to its original state even if the application panics.
pub struct Terminal {
    terminal: RatatuiTerminal<CrosstermBackend<Stdout>>,
    mouse_capture_enabled: bool,
}

impl Terminal {
    /// Create a new terminal instance with default configuration.
    ///
    /// This enables raw mode, enters the alternate screen, and enables mouse capture.
    pub fn new() -> Result<Self, TerminalError> {
        Self::with_config(TerminalConfig::default())
    }

    /// Create a new terminal instance with custom configuration.
    pub fn with_config(config: TerminalConfig) -> Result<Self, TerminalError> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();

        if config.mouse_capture {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = RatatuiTerminal::new(backend)?;

        Ok(Self {
            terminal,
            mouse_capture_enabled: config.mouse_capture,
        })
    }

    /// Get a mutable reference to the underlying ratatui terminal.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut RatatuiTerminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Get the terminal size as a Rect
    #[inline]
    pub fn size(&self) -> Result<ratatui::layout::Rect, TerminalError> {
        let size = self.terminal.size().map_err(TerminalError::from)?;
        Ok(ratatui::layout::Rect::new(0, 0, size.width, size.height))
    }

    /// Draw to the terminal using the provided closure.
    #[inline]
    pub fn draw<F>(&mut self, f: F) -> Result<(), TerminalError>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    /// Clear the terminal screen.
    #[inline]
    pub fn clear(&mut self) -> Result<(), TerminalError> {
        self.terminal.clear()?;
        Ok(())
    }

    /// Check if mouse capture is currently enabled.
    #[inline]
    pub fn mouse_capture_enabled(&self) -> bool {
        self.mouse_capture_enabled
    }

    /// Enable or disable mouse capture at runtime.
    ///
    /// This only sends the command if the state actually changes.
    pub fn set_mouse_capture(&mut self, enabled: bool) -> Result<(), TerminalError> {
        if enabled != self.mouse_capture_enabled {
            if enabled {
                execute!(self.terminal.backend_mut(), EnableMouseCapture)?;
            } else {
                execute!(self.terminal.backend_mut(), DisableMouseCapture)?;
            }
            self.mouse_capture_enabled = enabled;
        }
        Ok(())
    }

    /// Restore the terminal to its original state.
    ///
    /// This is called automatically on drop, but can be called manually
    /// if you need to restore the terminal before the struct is dropped.
    pub fn restore(&mut self) -> Result<(), TerminalError> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Best effort to restore terminal state
        let _ = self.restore();
    }
}

/// Install a panic hook that restores the terminal before printing the panic message.
///
/// Call this early in your application to ensure the terminal is restored
/// even if a panic occurs.
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Best effort to restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

        original_hook(panic_info);
    }));
}
