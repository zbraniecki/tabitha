//! Runtime task management for the TUI framework.
//!
//! This module provides the `TaskManager` for spawning and managing
//! background tasks at runtime, complementing the build-time task
//! registration via `AppBuilder`.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::watch;

use crate::bus::{MessageBus, TaskSender};
use crate::task::{CongestionController, Task, TaskContext, TaskHandle};

/// Error returned when spawning a task fails.
#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    /// A task with the same name is already running.
    #[error("Task with name '{0}' is already running")]
    DuplicateName(&'static str),

    /// Failed to spawn the task.
    #[error("Failed to spawn task '{0}': {1}")]
    SpawnFailed(&'static str, std::io::Error),
}

/// Manages runtime-spawned background tasks.
///
/// `TaskManager` allows spawning tasks dynamically during application
/// execution, complementing build-time tasks added via `AppBuilder`.
/// It provides methods to spawn, monitor, and abort tasks.
///
/// # Example
///
/// ```ignore
/// use tabitha::{TaskManager, TaskManagerContext, Task, TaskContext, TaskSender};
/// use std::time::Duration;
///
/// struct WorkerTask;
///
/// #[derive(Debug)]
/// struct WorkMessage(String);
///
/// impl Task for WorkerTask {
///     type Message = WorkMessage;
///
///     async fn run(self, sender: TaskSender<Self::Message>, ctx: TaskContext) {
///         // Task logic here
///     }
/// }
///
/// // In your event handler:
/// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) {
///     if let Some(mut task_ctx) = ctx.task_manager() {
///         task_ctx.spawn("worker", WorkerTask).unwrap();
///     }
/// }
/// ```
pub struct TaskManager {
    /// Active task handles by name.
    handles: HashMap<&'static str, TaskHandle>,
    /// Cancellation signal sender.
    cancel_tx: watch::Sender<bool>,
    /// Message bus for task registration.
    bus: MessageBus,
    /// Congestion controller for backpressure.
    congestion: Arc<CongestionController>,
}

impl TaskManager {
    /// Create a new task manager with a custom congestion controller.
    ///
    /// This allows configuring backpressure behavior.
    /// This is called internally by the `App` during initialization.
    pub(crate) fn with_congestion(
        cancel_tx: watch::Sender<bool>,
        bus: MessageBus,
        congestion: Arc<CongestionController>,
    ) -> Self {
        Self {
            handles: HashMap::new(),
            cancel_tx,
            bus,
            congestion,
        }
    }

    /// Spawn a new background task.
    ///
    /// The task will be registered with the message bus and started
    /// immediately. The task name must be unique among running tasks.
    ///
    /// # Errors
    ///
    /// Returns `SpawnError::DuplicateName` if a task with the same name
    /// is already running. Returns `SpawnError::SpawnFailed` if the task
    /// could not be spawned.
    ///
    /// # Example
    ///
    /// ```ignore
    /// task_manager.spawn("worker", MyTask::new()).unwrap();
    /// ```
    pub fn spawn<T: Task>(&mut self, name: &'static str, task: T) -> Result<(), SpawnError> {
        // Check for duplicate name
        if self.handles.contains_key(name) {
            return Err(SpawnError::DuplicateName(name));
        }

        // Register the channel and get a sender
        let sender: TaskSender<T::Message> = self.bus.register(name);

        // Create a cancel receiver for this task
        let cancel_rx = self.cancel_tx.subscribe();
        let ctx = TaskContext::new(cancel_rx, Arc::clone(&self.congestion));

        // Spawn the task with error handling
        let handle = tokio::spawn(async move {
            match task.run(sender, ctx).await {
                Ok(()) => {
                    tracing::info!(task_name = name, "Task completed successfully");
                }
                Err(e) => {
                    tracing::error!(task_name = name, error = %e, "Task failed with error");
                }
            }
        });

        let task_handle = TaskHandle::new(name, handle);
        self.handles.insert(name, task_handle);

        tracing::debug!(task_name = name, "Task spawned successfully");
        Ok(())
    }

    /// Check if a task with the given name is currently running.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if task_manager.is_running("worker") {
    ///     println!("Worker task is still active");
    /// }
    /// ```
    pub fn is_running(&self, name: &str) -> bool {
        self.handles.contains_key(name)
    }

    /// Abort a running task by name.
    ///
    /// Returns `true` if the task was found and aborted, `false` if
    /// no task with that name was running.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if task_manager.abort("worker") {
    ///     println!("Worker task aborted");
    /// }
    /// ```
    pub fn abort(&mut self, name: &str) -> bool {
        if let Some(handle) = self.handles.remove(name) {
            handle.abort();
            tracing::debug!(task_name = name, "Task aborted");
            true
        } else {
            false
        }
    }

    /// List all currently running task names.
    ///
    /// Returns a vector of task names for all active tasks.
    ///
    /// # Example
    ///
    /// ```ignore
    /// for name in task_manager.list_tasks() {
    ///     println!("Running task: {}", name);
    /// }
    /// ```
    pub fn list_tasks(&self) -> Vec<&'static str> {
        self.handles.keys().copied().collect()
    }

    /// Remove finished tasks from the internal tracking.
    ///
    /// This should be called periodically to clean up completed tasks
    /// and free resources.
    pub fn cleanup_finished(&mut self) {
        self.handles.retain(|name, handle| {
            if handle.is_finished() {
                tracing::debug!(task_name = name, "Cleaning up finished task");
                false
            } else {
                true
            }
        });
    }

    /// Take all task handles for shutdown.
    ///
    /// This is called internally during application shutdown to collect
    /// all running tasks for graceful termination.
    pub(crate) fn take_handles(&mut self) -> Vec<TaskHandle> {
        let handles: Vec<TaskHandle> = self.handles.drain().map(|(_, h)| h).collect();
        handles
    }

    /// Get a reference to the message bus.
    ///
    /// This allows registering new channel types if needed.
    pub fn bus(&self) -> &MessageBus {
        &self.bus
    }

    /// Get a reference to the congestion controller.
    ///
    /// This allows checking backpressure state.
    pub fn congestion(&self) -> &CongestionController {
        &self.congestion
    }

    /// Get the cancellation sender.
    #[allow(dead_code)]
    pub(crate) fn cancel_tx(&self) -> &watch::Sender<bool> {
        &self.cancel_tx
    }
}

/// Context wrapper for accessing task management from event handlers.
///
/// `TaskManagerContext` provides a limited interface to the task manager
/// that can be used within event handlers via `AppContext::task_manager()`.
///
/// This type is created temporarily during event handling and provides
/// access to spawn, query, and abort tasks.
///
/// # Example
///
/// ```ignore
/// fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
///     if let Some(mut task_ctx) = ctx.task_manager() {
///         // Spawn a task
///         if let Err(e) = task_ctx.spawn("worker", WorkerTask) {
///             eprintln!("Failed to spawn worker: {}", e);
///         }
///         
///         // Check if a task is running
///         if task_ctx.is_running("worker") {
///             println!("Worker is active");
///         }
///         
///         // Abort a task
///         if task_ctx.abort("worker") {
///             println!("Worker aborted");
///         }
///         
///         // List all tasks
///         for name in task_ctx.list_tasks() {
///             println!("Task: {}", name);
///         }
///     }
///     EventResult::Unhandled
/// }
/// ```
pub struct TaskManagerContext<'a> {
    manager: &'a mut TaskManager,
}

impl<'a> TaskManagerContext<'a> {
    /// Create a new task manager context.
    ///
    /// This is called internally by `AppContext`.
    pub(crate) fn new(manager: &'a mut TaskManager) -> Self {
        Self { manager }
    }

    /// Spawn a new background task.
    ///
    /// See [`TaskManager::spawn`] for details.
    pub fn spawn<T: Task>(&mut self, name: &'static str, task: T) -> Result<(), SpawnError> {
        self.manager.spawn(name, task)
    }

    /// Check if a task with the given name is currently running.
    ///
    /// See [`TaskManager::is_running`] for details.
    pub fn is_running(&self, name: &str) -> bool {
        self.manager.is_running(name)
    }

    /// Abort a running task by name.
    ///
    /// See [`TaskManager::abort`] for details.
    pub fn abort(&mut self, name: &str) -> bool {
        self.manager.abort(name)
    }

    /// List all currently running task names.
    ///
    /// See [`TaskManager::list_tasks`] for details.
    pub fn list_tasks(&self) -> Vec<&'static str> {
        self.manager.list_tasks()
    }
}
