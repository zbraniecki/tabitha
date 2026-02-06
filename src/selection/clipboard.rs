//! Clipboard integration for selection system.
//!
//! This module provides platform-agnostic clipboard access via the `arboard` crate.
//! It is only available when the `clipboard` feature is enabled.

use arboard::Clipboard;

/// Copy text to the system clipboard.
///
/// # Errors
///
/// Returns an error if:
/// - The clipboard cannot be accessed
/// - The text cannot be set in the clipboard
///
/// # Example
///
/// ```ignore
/// use tabitha::selection::clipboard::copy_to_clipboard;
///
/// if let Err(e) = copy_to_clipboard("Hello, clipboard!") {
///     eprintln!("Failed to copy: {}", e);
/// }
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;
    clipboard
        .set_text(text)
        .map_err(|e| ClipboardError::SetError(e.to_string()))?;
    Ok(())
}

/// Errors that can occur when interacting with the clipboard.
#[derive(Debug, Clone)]
pub enum ClipboardError {
    /// Failed to access the clipboard.
    AccessError(String),
    /// Failed to set text in the clipboard.
    SetError(String),
}

impl std::fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessError(e) => write!(f, "Failed to access clipboard: {}", e),
            Self::SetError(e) => write!(f, "Failed to set clipboard text: {}", e),
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_error_display() {
        let err = ClipboardError::AccessError("test error".to_string());
        assert!(err.to_string().contains("Failed to access clipboard"));
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_clipboard_error_display_set() {
        let err = ClipboardError::SetError("set failed".to_string());
        assert!(err.to_string().contains("Failed to set clipboard text"));
        assert!(err.to_string().contains("set failed"));
    }
}
