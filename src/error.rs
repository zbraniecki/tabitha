use std::fmt;

/// Error returned when sending a message fails.
#[derive(Debug)]
pub struct SendError<T>(pub T);

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "channel closed")
    }
}

impl<T: fmt::Debug> std::error::Error for SendError<T> {}

/// Error returned when try_send fails.
#[derive(Debug)]
pub enum TrySendError<T> {
    /// The channel is full.
    Full(T),
    /// The channel is closed.
    Closed(T),
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrySendError::Full(_) => write!(f, "channel full"),
            TrySendError::Closed(_) => write!(f, "channel closed"),
        }
    }
}

impl<T: fmt::Debug> std::error::Error for TrySendError<T> {}

/// Error type for task execution.
#[derive(Debug)]
pub enum TaskError {
    /// Task was cancelled.
    Cancelled,
    /// Task failed with an error.
    Failed(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::Cancelled => write!(f, "task cancelled"),
            TaskError::Failed(err) => write!(f, "task failed: {}", err),
        }
    }
}

impl std::error::Error for TaskError {}

/// Error type for task spawning.
#[derive(Debug)]
pub enum SpawnError {
    /// Task with this name already exists.
    DuplicateTask(&'static str),
    /// Failed to spawn task.
    SpawnFailed(String),
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnError::DuplicateTask(name) => write!(f, "duplicate task: {}", name),
            SpawnError::SpawnFailed(err) => write!(f, "spawn failed: {}", err),
        }
    }
}

impl std::error::Error for SpawnError {}

/// Error type for blocking task pool configuration.
#[derive(Debug)]
pub enum BlockingPoolError {
    /// Invalid pool size.
    InvalidSize(usize),
    /// Failed to create thread pool.
    PoolCreationFailed(String),
}

impl fmt::Display for BlockingPoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockingPoolError::InvalidSize(size) => write!(f, "invalid pool size: {}", size),
            BlockingPoolError::PoolCreationFailed(err) => {
                write!(f, "pool creation failed: {}", err)
            }
        }
    }
}

impl std::error::Error for BlockingPoolError {}
