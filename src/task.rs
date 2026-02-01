//! Background task support for the TUI framework.
//!
//! This module provides traits and utilities for running background
//! async tasks that communicate with the main UI via typed messages.

use std::future::Future;
use std::pin::Pin;

use thiserror::Error;
use tokio::task::JoinHandle;

use crate::bus::TaskSender;

/// Error returned when a blocking task fails.
#[derive(Debug, Error)]
pub enum BlockingTaskError {
    /// The blocking task panicked.
    #[error("blocking task panicked")]
    Panicked,
    /// The blocking task was aborted.
    #[error("blocking task was aborted")]
    Aborted,
}

impl From<tokio::task::JoinError> for BlockingTaskError {
    fn from(err: tokio::task::JoinError) -> Self {
        if err.is_panic() {
            BlockingTaskError::Panicked
        } else {
            BlockingTaskError::Aborted
        }
    }
}

/// Context provided to running tasks.
///
/// This provides access to utilities and cancellation signals.
pub struct TaskContext {
    /// Cancellation token for cooperative shutdown.
    cancel_rx: tokio::sync::watch::Receiver<bool>,
}

impl TaskContext {
    /// Create a new task context.
    pub(crate) fn new(cancel_rx: tokio::sync::watch::Receiver<bool>) -> Self {
        Self { cancel_rx }
    }

    /// Check if the task should stop.
    ///
    /// Tasks should periodically check this and exit gracefully
    /// when it returns `true`.
    #[inline]
    pub fn is_cancelled(&self) -> bool {
        *self.cancel_rx.borrow()
    }

    /// Wait until cancellation is requested.
    ///
    /// This is useful in `tokio::select!` to handle shutdown.
    pub async fn cancelled(&mut self) {
        // Wait for the value to change to true
        while !*self.cancel_rx.borrow() {
            if self.cancel_rx.changed().await.is_err() {
                // Sender dropped, treat as cancellation
                return;
            }
        }
    }

    /// Create a clone of this context for use in spawned subtasks.
    pub fn clone_context(&self) -> Self {
        Self {
            cancel_rx: self.cancel_rx.clone(),
        }
    }
}

impl Clone for TaskContext {
    fn clone(&self) -> Self {
        self.clone_context()
    }
}

/// A background task that runs asynchronously and sends typed messages.
///
/// Tasks are spawned when the application starts and run concurrently
/// with the main event loop. They communicate with the UI by sending
/// typed messages through their provided sender.
///
/// # Example
///
/// ```ignore
/// use tabitha::{Task, TaskContext, TaskSender};
/// use std::time::Duration;
///
/// struct TickerTask {
///     interval: Duration,
/// }
///
/// #[derive(Debug)]
/// struct TickMessage(u64);
///
/// impl Task for TickerTask {
///     type Message = TickMessage;
///
///     async fn run(self, sender: TaskSender<Self::Message>, mut ctx: TaskContext) {
///         let mut count = 0u64;
///         let mut interval = tokio::time::interval(self.interval);
///
///         loop {
///             tokio::select! {
///                 _ = interval.tick() => {
///                     count += 1;
///                     if sender.send(TickMessage(count)).await.is_err() {
///                         break;
///                     }
///                 }
///                 _ = ctx.cancelled() => {
///                     break;
///                 }
///             }
///         }
///     }
/// }
/// ```
pub trait Task: Send + 'static {
    /// The message type this task sends to the UI.
    type Message: Send + 'static;

    /// Run the task.
    ///
    /// The task receives a sender for its message type and a context
    /// for cancellation. The task should exit when either:
    /// - The sender fails (channel closed, app shutting down)
    /// - The context signals cancellation
    fn run(
        self,
        sender: TaskSender<Self::Message>,
        ctx: TaskContext,
    ) -> impl Future<Output = ()> + Send;
}

/// A type-erased boxed task future.
pub type BoxedTaskFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A factory function that creates a task future.
pub type TaskFactory = Box<dyn FnOnce(TaskContext) -> BoxedTaskFuture + Send>;

/// Handle to a spawned background task.
pub struct TaskHandle {
    /// The task name.
    pub name: &'static str,
    /// The join handle for the spawned task.
    pub handle: JoinHandle<()>,
}

impl TaskHandle {
    /// Create a new task handle.
    pub fn new(name: &'static str, handle: JoinHandle<()>) -> Self {
        Self { name, handle }
    }

    /// Check if the task has finished.
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Abort the task.
    pub fn abort(&self) {
        self.handle.abort();
    }

    /// Wait for the task to complete.
    pub async fn join(self) -> Result<(), tokio::task::JoinError> {
        self.handle.await
    }
}

/// Spawn a blocking operation on a dedicated thread pool.
///
/// Use this for CPU-intensive or blocking I/O operations that would
/// block the async runtime.
///
/// This function is only available with the `blocking-tasks` feature.
#[cfg(feature = "blocking-tasks")]
pub async fn spawn_blocking<F, T>(f: F) -> Result<T, BlockingTaskError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tracing::trace!("spawning blocking task");
    tokio::task::spawn_blocking(f).await.map_err(BlockingTaskError::from)
}

/// Spawn a blocking operation, returning the error if it fails.
///
/// This is a convenience wrapper around [`spawn_blocking`] that returns
/// the blocking task's result or a [`BlockingTaskError`] if the task
/// panicked or was aborted.
///
/// # Errors
///
/// Returns [`BlockingTaskError::Panicked`] if the blocking task panicked,
/// or [`BlockingTaskError::Aborted`] if it was aborted.
#[cfg(feature = "blocking-tasks")]
pub async fn spawn_blocking_or_err<F, T>(f: F) -> Result<T, BlockingTaskError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_blocking(f).await
}
