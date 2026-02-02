//! Common utilities for tabitha examples.
//!
//! This module provides CLI argument parsing and tracing setup
//! that can be shared across all examples.

#![allow(dead_code)]

use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Common CLI arguments for tabitha examples
#[derive(Parser, Debug)]
pub struct Args {
    /// Enable logging to a file
    #[arg(long, value_name = "PATH")]
    pub log_file: Option<PathBuf>,

    /// Enable developer console mode (press ` to toggle)
    #[arg(short, long)]
    pub dev: bool,
}

impl Args {
    /// Initialize tracing and return optional log receiver for dev console.
    ///
    /// This sets up tracing based on the CLI arguments:
    /// - `--log-file <PATH>`: Log to file with INFO level (respects RUST_LOG)
    /// - `--dev`: Enable dev console with tracing integration
    /// - Neither: No tracing initialization (tracing calls are no-ops)
    pub fn init_tracing(&self) -> Option<tokio::sync::mpsc::UnboundedReceiver<tabitha::LogLine>> {
        if let Some(path) = &self.log_file {
            // Mode 1: Log to file
            if let Ok(file) = std::fs::File::create(path) {
                tracing_subscriber::fmt()
                    .with_writer(file)
                    .with_ansi(false)
                    .with_env_filter(
                        tracing_subscriber::EnvFilter::builder()
                            .with_default_directive(tracing::Level::INFO.into())
                            .from_env_lossy(),
                    )
                    .init();
            } else {
                eprintln!("Warning: Failed to create log file at {:?}", path);
            }
            None
        } else if self.dev {
            // Mode 2: Dev console mode
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let dev_layer = tabitha::DevConsoleLayer::new(tx);

            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::builder()
                        .with_default_directive(tracing::Level::INFO.into())
                        .from_env_lossy(),
                )
                .with(dev_layer)
                .init();

            Some(rx)
        } else {
            // Mode 3: No logging - don't initialize tracing subscriber
            // Tracing calls become no-ops with minimal overhead
            None
        }
    }
}
