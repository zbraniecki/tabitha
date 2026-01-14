# tabitha

An async, event-driven TUI framework built on [ratatui](https://github.com/ratatui-org/ratatui) and [tokio](https://tokio.rs/).

## Overview

Tabitha provides a clean architecture for building terminal user interfaces with minimal CPU usage. It only redraws in response to events, making it ideal for applications that need to be "quiet" and power-efficient.

## Features

- **Event-driven**: No polling – only responds to terminal events and task messages
- **Async tasks**: Background tasks communicate via typed message channels
- **Builder pattern**: Clean, composable application setup
- **Tabs support**: Built-in tab management with enable/disable support
- **Focus management**: Navigate between focusable components
- **Minimal allocations**: Designed for efficiency in hot paths
- **Runtime control**: Toggle mouse capture, navigate tabs, quit via contexts

## Installation

Add tabitha to your `Cargo.toml`:

```toml
[dependencies]
tabitha = "0.0.1"
ratatui = "0.30"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Quick Start

```rust
use tabitha::{AppBuilder, Component, MainUi, Event, AppContext, DrawContext, EventResult};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

struct MyApp;

impl Component for MyApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        frame.render_widget(Paragraph::new("Hello, tabitha!"), area);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        if event.is_quit() {
            ctx.quit();
            return EventResult::Handled;
        }
        EventResult::Unhandled
    }
}

impl MainUi for MyApp {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = AppBuilder::new()
        .main_ui(MyApp)
        .build()?;

    app.run().await?;
    Ok(())
}
```

## Core Concepts

### Components

Components are the building blocks of your TUI application. Implement the `Component` trait to create UI elements:

```rust
use tabitha::{Component, Event, AppContext, DrawContext, EventResult, KeyCode};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

struct Counter {
    count: u32,
}

impl Component for Counter {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let text = format!("Count: {}", self.count);
        frame.render_widget(Paragraph::new(text), area);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        if event.is_key(KeyCode::Up) {
            self.count = self.count.saturating_add(1);
            EventResult::Handled
        } else if event.is_key(KeyCode::Down) {
            self.count = self.count.saturating_sub(1);
            EventResult::Handled
        } else {
            EventResult::Unhandled
        }
    }
}
```

### Tabs

Register tabs with the application and use the context to draw and navigate them:

```rust
use tabitha::{Tab, AppBuilder, Component, MainUi, DrawContext, AppContext, EventResult, KeyCode};
use ratatui::{Frame, layout::Rect, widgets::Paragraph};

struct HomeTab;

impl Tab for HomeTab {
    fn id(&self) -> &str { "home" }
    fn title(&self) -> &str { "Home" }
    
    fn draw(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new("Home content"), area);
    }
}

struct MyApp;

impl Component for MyApp {
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Draw tab bar and content
        ctx.tabs().draw_tabbar(frame, tab_bar_area);
        ctx.tabs().draw_content(frame, content_area);
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
        // Navigate with Tab key
        if event.is_key(KeyCode::Tab) {
            ctx.tabs().select_next();
            return EventResult::Handled;
        }
        EventResult::Unhandled
    }
}

impl MainUi for MyApp {}

// Build with tabs
let app = AppBuilder::new()
    .main_ui(MyApp)
    .add_tab(HomeTab)
    .add_tab(SettingsTab)
    .build()?;
```

Tabs can be enabled/disabled at runtime:

```rust
// In your event handler
ctx.tabs().set_enabled("settings", false);  // Disable a tab
ctx.tabs().is_enabled("settings");           // Check if enabled
```

### Background Tasks

Create async background tasks that communicate with the UI via typed messages:

```rust
use tabitha::{Task, TaskContext, TaskSender, MainUi};
use std::time::Duration;

#[derive(Debug)]
struct TickMessage(u64);

struct TickerTask {
    interval: Duration,
}

impl Task for TickerTask {
    type Message = TickMessage;

    async fn run(self, sender: TaskSender<Self::Message>, mut ctx: TaskContext) {
        let mut count = 0u64;
        let mut interval = tokio::time::interval(self.interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    count += 1;
                    if sender.send(TickMessage(count)).await.is_err() {
                        break;
                    }
                }
                _ = ctx.cancelled() => {
                    break;
                }
            }
        }
    }
}

// Handle messages in your MainUi
impl MainUi for MyApp {
    fn handle_task_message(
        &mut self,
        task_name: &str,
        message: Box<dyn std::any::Any + Send>,
        ctx: &mut AppContext,
    ) -> bool {
        if task_name == "ticker" {
            if let Some(TickMessage(count)) = message.downcast_ref::<TickMessage>() {
                // Handle the message
                return true; // Request redraw
            }
        }
        false
    }
}

// Register the task
let app = AppBuilder::new()
    .main_ui(MyApp::new())
    .add_task("ticker", TickerTask { interval: Duration::from_secs(1) })
    .build()?;
```

### Focus Management

Components can participate in focus navigation:

```rust
impl Component for MyWidget {
    fn focus_id(&self) -> Option<&str> {
        Some("my_widget")  // Makes this component focusable
    }

    fn is_focusable(&self) -> bool {
        true  // Can be toggled dynamically
    }

    fn on_focus(&mut self) {
        // Called when focus is gained
    }

    fn on_blur(&mut self) {
        // Called when focus is lost
    }
}

// Navigate focus in event handlers
if event.is_key(KeyCode::Tab) {
    ctx.focus().focus_next();
    return EventResult::Handled;
}
```

### Event Handling

Events are handled through the `handle_event` method with an `EventResult` return type:

- `EventResult::Handled` – Event consumed, stop propagation
- `EventResult::Unhandled` – Bubble to parent component
- `EventResult::StopPropagation` – Stop propagation without handling

Convenience methods on `Event`:

```rust
event.is_quit()                                    // Ctrl+C or Ctrl+Q
event.is_key(KeyCode::Enter)                       // Specific key
event.is_key_with_modifiers(KeyCode::Char('s'), KeyModifiers::CONTROL)
event.is_mouse_click()                             // Any mouse click
event.mouse_position()                             // Get (x, y) if mouse event
```

## Configuration

### Mouse Capture

Mouse capture can be configured at build time or toggled at runtime:

```rust
// At build time
let app = AppBuilder::new()
    .main_ui(MyApp::new())
    .mouse_capture(false)  // Disable mouse capture
    .build()?;

// At runtime
ctx.set_mouse_capture(true);
let enabled = ctx.mouse_capture_enabled();
```

### Tick Rate

For applications that need periodic updates:

```rust
let app = AppBuilder::new()
    .main_ui(MyApp::new())
    .tick_rate(Duration::from_millis(250))  // Optional periodic tick
    .build()?;
```

## Examples

Run the examples to see tabitha in action:

```bash
# Counter with background task
cargo run --example counter

# Tab navigation
cargo run --example tabs

# Focus management with tables
cargo run --example focus_tables
```

## Optional Features

- `blocking-tasks` – Enable helpers for spawning blocking operations on a dedicated thread pool

```toml
[dependencies]
tabitha = { version = "0.0.1", features = ["blocking-tasks"] }
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        AppBuilder                           │
│  - main_ui(component)                                       │
│  - add_tab(tab)                                             │
│  - add_task(name, task)                                     │
│  - mouse_capture(bool)                                      │
│  - tick_rate(duration)                                      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                           App                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   MainUi    │  │ TabManager  │  │    FocusManager     │  │
│  │ (Component) │  │   (Tabs)    │  │ (Focus navigation)  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│                                                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Event Loop                           ││
│  │  - Terminal events (keyboard, mouse, resize)            ││
│  │  - Task messages (via MessageBus)                       ││
│  │  - Optional tick timer                                  ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Background Tasks                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │  Task 1  │  │  Task 2  │  │  Task N  │  ...              │
│  │ (async)  │  │ (async)  │  │ (async)  │                   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                   │
│       │             │             │                         │
│       └─────────────┼─────────────┘                         │
│                     │ TaskSender<T>                         │
│                     ▼                                       │
│             ┌───────────────┐                               │
│             │  MessageBus   │ ──► MainUi.handle_task_message│
│             └───────────────┘                               │
└─────────────────────────────────────────────────────────────┘
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
