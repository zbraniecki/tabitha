//! Message bus for inter-task communication.
//!
//! This module provides a typed channel registry for communication
//! between background tasks and the main UI.

use std::any::Any;
use std::collections::HashMap;
use std::num::NonZeroUsize;

use tokio::sync::mpsc;

/// Default channel buffer size for task messages.
pub const DEFAULT_CHANNEL_SIZE: NonZeroUsize = match NonZeroUsize::new(32) {
    Some(n) => n,
    None => unreachable!(),
};

/// A type-erased message that can be sent through the bus.
pub struct TaskMessage {
    /// The name of the task that sent this message.
    pub task_name: &'static str,
    /// The message payload.
    pub payload: Box<dyn Any + Send>,
}

impl TaskMessage {
    /// Create a new task message.
    pub fn new<T: Any + Send + 'static>(task_name: &'static str, message: T) -> Self {
        Self {
            task_name,
            payload: Box::new(message),
        }
    }

    /// Try to downcast the message to a specific type.
    pub fn downcast<T: Any + Send + 'static>(self) -> Result<T, Self> {
        match self.payload.downcast::<T>() {
            Ok(msg) => Ok(*msg),
            Err(payload) => Err(Self {
                task_name: self.task_name,
                payload,
            }),
        }
    }

    /// Try to get a reference to the message as a specific type.
    pub fn downcast_ref<T: Any + Send + 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref()
    }
}

/// Error returned when sending a message fails.
#[derive(Debug)]
pub struct SendError<T>(pub T);

impl<T> std::fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "channel closed")
    }
}

impl<T: std::fmt::Debug> std::error::Error for SendError<T> {}

/// Message bus for typed inter-task communication.
///
/// The message bus allows background tasks to send typed messages
/// to the main UI. Each task registers its message type when added
/// to the application.
///
/// # Example
///
/// ```ignore
/// use tabitha::bus::MessageBus;
///
/// let mut bus = MessageBus::new();
///
/// // Register a channel for a task
/// let tx = bus.register::<String>("my_task");
///
/// // Send a message (in an async context)
/// tx.send("Hello".to_string()).await.unwrap();
/// ```
pub struct MessageBus {
    /// Registered task names for validation.
    registered_tasks: HashMap<&'static str, ()>,
    /// Unified channel for receiving messages from all tasks.
    unified_tx: mpsc::Sender<TaskMessage>,
    unified_rx: Option<mpsc::Receiver<TaskMessage>>,
}

impl MessageBus {
    /// Create a new empty message bus.
    pub fn new() -> Self {
        let (unified_tx, unified_rx) = mpsc::channel(DEFAULT_CHANNEL_SIZE.get() * 4);
        Self {
            registered_tasks: HashMap::new(),
            unified_tx,
            unified_rx: Some(unified_rx),
        }
    }

    /// Register a new channel for a task.
    ///
    /// Returns a sender that the task can use to send messages.
    /// Messages sent through this sender will be forwarded to the
    /// unified receiver with the task name attached.
    pub fn register<T: Any + Send + 'static>(&mut self, task_name: &'static str) -> TaskSender<T> {
        self.registered_tasks.insert(task_name, ());

        TaskSender {
            task_name,
            unified_tx: self.unified_tx.clone(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a sender for a task (without registering).
    ///
    /// This is useful when you need additional senders for an already
    /// registered task.
    pub fn sender<T: Any + Send + 'static>(
        &self,
        task_name: &'static str,
    ) -> Option<TaskSender<T>> {
        if self.registered_tasks.contains_key(task_name) {
            Some(TaskSender {
                task_name,
                unified_tx: self.unified_tx.clone(),
                _marker: std::marker::PhantomData,
            })
        } else {
            None
        }
    }

    /// Take the unified receiver.
    ///
    /// This can only be called once. The receiver is used by the main
    /// event loop to receive messages from all tasks.
    pub fn take_receiver(&mut self) -> Option<mpsc::Receiver<TaskMessage>> {
        self.unified_rx.take()
    }

    /// Check if a task is registered.
    pub fn has_task(&self, task_name: &str) -> bool {
        self.registered_tasks.contains_key(task_name)
    }

    /// Get the number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.registered_tasks.len()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// A typed sender wrapper that forwards messages to the unified channel.
pub struct TaskSender<T> {
    task_name: &'static str,
    unified_tx: mpsc::Sender<TaskMessage>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Clone for TaskSender<T> {
    fn clone(&self) -> Self {
        Self {
            task_name: self.task_name,
            unified_tx: self.unified_tx.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T: Any + Send + 'static> TaskSender<T> {
    /// Send a message.
    ///
    /// This wraps the message and forwards it to the unified channel
    /// with the task name attached.
    pub async fn send(&self, message: T) -> Result<(), SendError<T>> {
        let task_message = TaskMessage::new(self.task_name, message);
        self.unified_tx.send(task_message).await.map_err(|e| {
            // Extract the original message from TaskMessage
            let payload = e.0.payload;
            let msg = payload
                .downcast::<T>()
                .expect("type mismatch in TaskSender");
            SendError(*msg)
        })
    }

    /// Try to send a message without blocking.
    pub fn try_send(&self, message: T) -> Result<(), TrySendError<T>> {
        let task_message = TaskMessage::new(self.task_name, message);
        self.unified_tx.try_send(task_message).map_err(|e| match e {
            mpsc::error::TrySendError::Full(tm) => {
                let msg = tm.payload.downcast::<T>().expect("type mismatch");
                TrySendError::Full(*msg)
            }
            mpsc::error::TrySendError::Closed(tm) => {
                let msg = tm.payload.downcast::<T>().expect("type mismatch");
                TrySendError::Closed(*msg)
            }
        })
    }

    /// Get the task name associated with this sender.
    pub fn task_name(&self) -> &'static str {
        self.task_name
    }
}

/// Error returned when try_send fails.
#[derive(Debug)]
pub enum TrySendError<T> {
    /// The channel is full.
    Full(T),
    /// The channel is closed.
    Closed(T),
}

impl<T> std::fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrySendError::Full(_) => write!(f, "channel full"),
            TrySendError::Closed(_) => write!(f, "channel closed"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}
