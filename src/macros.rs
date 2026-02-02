//! Macros for the Tabitha TUI framework.
//!
//! This module provides utility macros for common patterns in TUI development.

/// Type-safe task message dispatch macro.
///
/// This macro provides a type-safe way to dispatch task messages to their
/// corresponding handlers. It matches on the task name and downcasts the
/// message to the appropriate type.
///
/// # Syntax
///
/// ```ignore
/// match_task_message!(message, task_name => {
///     "task_name" => MessageType as binding => handler_expr,
///     // ... more arms
/// })
/// ```
///
/// # Parameters
///
/// - `$msg`: The message expression (a `TaskMessage`)
/// - `$task_name`: The task name to match against (typically a string)
/// - Each arm consists of:
///   - `"task_name"` - The literal task name string
///   - `MessageType` - The expected message type for this task
///   - `as binding` - The variable name to bind the downcast result to
///   - `=> handler_expr` - The expression to execute (should return `bool` indicating if handled)
///
/// # Returns
///
/// Returns `true` if a matching handler was found and executed, `false` otherwise.
///
/// # Example
///
/// ```ignore
/// use tabitha::match_task_message;
/// use tabitha::bus::TaskMessage;
///
/// struct MyComponent {
///     count: u64,
///     data: Vec<u8>,
/// }
///
/// #[derive(Debug)]
/// struct TickerMessage(u64);
///
/// #[derive(Debug)]
/// struct LoaderMessage { data: Vec<u8> }
///
/// impl MyComponent {
///     fn handle_message(&mut self, message: &TaskMessage, task_name: &str) -> bool {
///         match_task_message!(message, task_name => {
///             "ticker" => TickerMessage as msg => {
///                 self.count = msg.0;
///                 true  // handled
///             },
///             "loader" => LoaderMessage as msg => {
///                 self.data = msg.data.clone();
///                 true
///             },
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! match_task_message {
    ($msg:expr, $task_name:expr => {
        $($name:literal => $type:ty as $binding:ident => $handler:expr),* $(,)?
    }) => {
        match $task_name {
            $(
                $name => {
                    if let Some($binding) = $msg.downcast_ref::<$type>() {
                        $handler
                    } else {
                        false
                    }
                }
            )*
            _ => false,
        }
    };
}

/// Re-export the macro at crate root for convenience.
pub use match_task_message;
