//! Tracing layer that sends logs to the developer console.

use std::fmt;
use tokio::sync::mpsc;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

use crate::widget::LogLine;

/// A tracing layer that sends log events to a channel.
///
/// This layer captures tracing events and sends them to a channel,
/// which can then be consumed by the DevConsole widget.
pub struct DevConsoleLayer {
    sender: mpsc::UnboundedSender<LogLine>,
}

impl DevConsoleLayer {
    /// Create a new DevConsoleLayer with the given sender.
    pub fn new(sender: mpsc::UnboundedSender<LogLine>) -> Self {
        Self { sender }
    }
}

impl<S> Layer<S> for DevConsoleLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let level = *metadata.level();
        let target = metadata.target();

        // Format the event message
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        // If no message was found, build one from all fields
        let message = if visitor.message.is_empty() {
            visitor.other_fields.join(", ")
        } else {
            // Append other fields if present
            if !visitor.other_fields.is_empty() {
                format!("{} {}", visitor.message, visitor.other_fields.join(", "))
            } else {
                visitor.message
            }
        };

        // Create a log line
        let log_line = LogLine::new(level, message).with_target(target);

        // Send to the channel (ignore errors if receiver is dropped)
        let _ = self.sender.send(log_line);
    }
}

/// Visitor that extracts the message from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
    other_fields: Vec<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.other_fields
                .push(format!("{}={}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            // Format without quotes for cleaner output
            self.message = format!("{:?}", value);
            // Remove surrounding quotes if present
            if self.message.starts_with('"')
                && self.message.ends_with('"')
                && self.message.len() > 1
            {
                self.message = self.message[1..self.message.len() - 1].to_string();
            }
        } else {
            self.other_fields
                .push(format!("{}={:?}", field.name(), value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.other_fields
                .push(format!("{}={}", field.name(), value));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.other_fields
                .push(format!("{}={}", field.name(), value));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.other_fields
                .push(format!("{}={}", field.name(), value));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.other_fields
                .push(format!("{}={}", field.name(), value));
        }
    }
}
