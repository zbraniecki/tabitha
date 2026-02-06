//! Selection and clipboard integration.
//!
//! This module provides two layered features:
//!
//! 1. **`selection`** — Core selection support (no external deps):
//!    - TextBox keyboard selection (Shift+arrows, Ctrl+A select-all)
//!    - Region-based mouse selection (drag within widget boundaries)
//!
//! 2. **`clipboard`** — System clipboard integration (requires `arboard`):
//!    - Ctrl+C/X in TextBox copies/cuts to system clipboard
//!    - Mouse selection copy-to-clipboard (configurable: auto-copy on MouseUp or explicit Ctrl+C)
//!
//! ## Usage
//!
//! Enable features in `Cargo.toml`:
//! ```toml
//! [dependencies]
//! tabitha = { version = "0.2", features = ["selection"] }
//! # or
//! tabitha = { version = "0.2", features = ["clipboard"] }
//! ```

pub mod region;
pub mod types;

// These modules are gated behind features and will be created in later phases
#[cfg(feature = "selection")]
pub mod manager;

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(feature = "selection")]
pub mod rendering;

// Re-export core types
pub use region::{RegionId, RegionRegistry, SelectionRegion};
pub use types::{MouseSelectionPhase, SelectionPos, SelectionRange};

// Re-export manager types when selection feature is enabled
#[cfg(feature = "selection")]
pub use manager::{SelectionConfig, SelectionManager};

// Re-export clipboard function when the clipboard feature is enabled
#[cfg(feature = "clipboard")]
pub use clipboard::{copy_to_clipboard, ClipboardError};
