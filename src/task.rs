//! Background task support for the TUI framework.
//!
//! This module provides traits and utilities for running background
//! async tasks that communicate with the main UI via typed messages.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use thiserror::Error;
use tokio::task::JoinHandle;

use crate::bus::TaskSender;

/// Default maximum number of pending messages before applying backpressure.
const DEFAULT_MAX_PENDING: usize = 1000;

/// Default threshold at which tasks should yield control.
const DEFAULT_YIELD_THRESHOLD: usize = 100;

/// System-level messages about task lifecycle events.
///
/// These messages are emitted by the framework to notify about task
/// completion and errors. They can be consumed by the UI to display
/// task status or handle failures.
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// A task completed successfully.
    TaskCompleted {
        /// The name of the task that completed.
        name: &'static str,
    },
    /// A task encountered an error.
    TaskError {
        /// The name of the task that failed.
        name: &'static str,
        /// The error message.
        error: String,
    },
}

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

/// Controller for managing backpressure in the message system.
///
/// `CongestionController` tracks the number of pending messages and
/// provides signals to tasks when they should yield or slow down
/// to prevent overwhelming the message bus.
///
/// # Example
///
/// ```ignore
/// use tabitha::task::CongestionController;
///
/// let controller = CongestionController::new(1000, 100);
/// controller.increment_pending();
///
/// if controller.should_yield() {
///     // Yield control to allow message processing
///     tokio::task::yield_now().await;
/// }
///
/// controller.decrement_pending();
/// ```
pub struct CongestionController {
    /// Current number of pending messages.
    pending_messages: AtomicUsize,
    /// Maximum number of pending messages before backpressure.
    max_pending: usize,
    /// Threshold at which tasks should yield.
    yield_threshold: usize,
}

impl CongestionController {
    /// Create a new congestion controller.
    ///
    /// # Arguments
    ///
    /// * `max_pending` - Maximum number of pending messages before backpressure
    /// * `yield_threshold` - Threshold at which tasks should yield control
    pub fn new(max_pending: usize, yield_threshold: usize) -> Self {
        Self {
            pending_messages: AtomicUsize::new(0),
            max_pending,
            yield_threshold,
        }
    }

    /// Check if the task should yield based on current congestion.
    ///
    /// Returns `true` if the number of pending messages exceeds
    /// the yield threshold.
    pub fn should_yield(&self) -> bool {
        self.pending_messages.load(Ordering::Relaxed) >= self.yield_threshold
    }

    /// Check if the system is in a high congestion state.
    ///
    /// Returns `true` if the number of pending messages exceeds
    /// 75% of the maximum pending limit.
    pub fn is_congested(&self) -> bool {
        self.pending_messages.load(Ordering::Relaxed) >= (self.max_pending * 3 / 4)
    }

    /// Increment the pending message count.
    ///
    /// Should be called when a message is queued.
    pub fn increment_pending(&self) {
        self.pending_messages.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the pending message count.
    ///
    /// Should be called when a message is processed.
    pub fn decrement_pending(&self) {
        // Use SeqCst to ensure proper synchronization when reaching zero
        let prev = self.pending_messages.fetch_sub(1, Ordering::SeqCst);
        // Ensure we don't underflow (shouldn't happen in correct usage)
        if prev == 0 {
            // Restore to 0 if we underflowed
            self.pending_messages.store(0, Ordering::Relaxed);
        }
    }

    /// Get the current number of pending messages.
    pub fn pending_count(&self) -> usize {
        self.pending_messages.load(Ordering::Relaxed)
    }

    /// Get the maximum pending messages limit.
    pub fn max_pending(&self) -> usize {
        self.max_pending
    }

    /// Get the yield threshold.
    pub fn yield_threshold(&self) -> usize {
        self.yield_threshold
    }
}

impl Default for CongestionController {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PENDING, DEFAULT_YIELD_THRESHOLD)
    }
}

/// Context provided to running tasks.
///
/// This provides access to utilities, cancellation signals, and
/// backpressure control.
pub struct TaskContext {
    /// Cancellation token for cooperative shutdown.
    cancel_rx: tokio::sync::watch::Receiver<bool>,
    /// Congestion controller for backpressure management.
    congestion: Arc<CongestionController>,
}

impl TaskContext {
    /// Create a new task context.
    pub(crate) fn new(
        cancel_rx: tokio::sync::watch::Receiver<bool>,
        congestion: Arc<CongestionController>,
    ) -> Self {
        Self {
            cancel_rx,
            congestion,
        }
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
            congestion: Arc::clone(&self.congestion),
        }
    }

    /// Yield if the system is experiencing congestion.
    ///
    /// This method checks the congestion controller and yields control
    /// if the pending message count exceeds the yield threshold. This
    /// allows the system to process queued messages before continuing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn process_items(&mut self, ctx: &TaskContext) {
    ///     for item in items {
    ///         // Periodically yield if congested
    ///         ctx.yield_if_needed().await;
    ///
    ///         self.process(item).await;
    ///     }
    /// }
    /// ```
    pub async fn yield_if_needed(&self) {
        if self.congestion.should_yield() {
            tokio::task::yield_now().await;
        }
    }

    /// Check if the task should slow down due to congestion.
    ///
    /// Returns `true` if the system is experiencing high congestion
    /// and the task should reduce its message rate.
    ///
    /// # Example
    ///
    /// ```ignore
    /// async fn generate_data(&self, sender: TaskSender<Data>, ctx: &TaskContext) {
    ///     loop {
    ///         if ctx.should_slow_down() {
    ///             // Reduce generation rate
    ///             tokio::time::sleep(Duration::from_millis(10)).await;
    ///         }
    ///
    ///         let data = self.produce_data();
    ///         sender.send(data).await.ok();
    ///     }
    /// }
    /// ```
    pub fn should_slow_down(&self) -> bool {
        self.congestion.should_yield()
    }

    /// Get a reference to the congestion controller.
    ///
    /// This allows direct access to congestion metrics if needed.
    pub fn congestion(&self) -> &CongestionController {
        &self.congestion
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
///     type Error = std::convert::Infallible;
///
///     async fn run(self, sender: TaskSender<Self::Message>, mut ctx: TaskContext) -> Result<(), Self::Error> {
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
///         Ok(())
///     }
/// }
/// ```
pub trait Task: Send + 'static {
    /// The message type this task sends to the UI.
    type Message: Send + 'static;

    /// The error type this task can return.
    ///
    /// Use `std::convert::Infallible` for tasks that never fail.
    type Error: std::error::Error + Send + 'static;

    /// Run the task.
    ///
    /// The task receives a sender for its message type and a context
    /// for cancellation. The task should exit when either:
    /// - The sender fails (channel closed, app shutting down)
    /// - The context signals cancellation
    ///
    /// Returns `Ok(())` on successful completion, or `Err(Self::Error)`
    /// if the task encounters an error.
    fn run(
        self,
        sender: TaskSender<Self::Message>,
        ctx: TaskContext,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// A type-erased boxed task future that returns a result.
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

/// Handle to a spawned blocking task.
///
/// This handle allows awaiting the result of a blocking task
/// and provides methods to check status and abort the task.
///
/// # Example
///
/// ```ignore
/// use tabitha::task::BlockingHandle;
///
/// let handle: BlockingHandle<i32> = ctx.spawn_blocking(|| {
///     // CPU-intensive work
///     compute_result()
/// });
///
/// // Later, await the result
/// match handle.await {
///     Ok(result) => println!("Result: {}", result),
///     Err(e) => println!("Task failed: {}", e),
/// }
/// ```
pub struct BlockingHandle<T> {
    /// The inner join handle from tokio.
    inner: JoinHandle<T>,
}

impl<T> BlockingHandle<T> {
    /// Create a new blocking handle.
    pub(crate) fn new(inner: JoinHandle<T>) -> Self {
        Self { inner }
    }

    /// Check if the task has finished.
    pub fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    /// Abort the task.
    pub fn abort(&self) {
        self.inner.abort();
    }

    /// Wait for the task to complete and return its result.
    pub async fn join(self) -> Result<T, tokio::task::JoinError> {
        self.inner.await
    }
}

impl<T> Future for BlockingHandle<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.inner).poll(cx)
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
    tokio::task::spawn_blocking(f)
        .await
        .map_err(BlockingTaskError::from)
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
