//! # tabitha
//!
//! An async, event-driven TUI framework built on ratatui and tokio.
//!
//! This framework provides a clean architecture for building terminal user interfaces
//! with minimal CPU usage. It only redraws in response to events, making it ideal
//! for applications that need to be "quiet" and power-efficient.
//!
//! ## Features
//!
//! - **Event-driven**: No polling, only responds to terminal events and task messages
//! - **Async tasks**: Background tasks communicate via typed message channels
//! - **Builder pattern**: Clean, composable application setup
//! - **Tabs support**: Built-in tab management with enable/disable support
//! - **Minimal allocations**: Designed for efficiency in hot paths
//! - **Runtime control**: Toggle mouse capture, navigate tabs, quit via contexts
//!
//! ## Quick Start
//!
//! ```ignore
//! use tabitha::{AppBuilder, Component, MainUi, Event, AppContext, DrawContext, EventResult};
//! use ratatui::{Frame, layout::Rect, widgets::Paragraph};
//!
//! struct MyApp;
//!
//! impl Component for MyApp {
//!     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
//!         frame.render_widget(Paragraph::new("Hello!"), area);
//!     }
//!
//!     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
//!         if event.is_quit() {
//!             ctx.quit();
//!             return EventResult::Handled;
//!         }
//!         EventResult::Unhandled
//!     }
//! }
//!
//! impl MainUi for MyApp {}
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let app = AppBuilder::new()
//!         .main_ui(MyApp)
//!         .build()?;
//!
//!     app.run().await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Tabs
//!
//! Register tabs with the application and use the context to draw and navigate them:
//!
//! ```ignore
//! use tabitha::{Tab, AppBuilder, Component, MainUi, DrawContext, AppContext, EventResult, KeyCode};
//!
//! struct HomeTab;
//!
//! impl Tab for HomeTab {
//!     fn id(&self) -> &str { "home" }
//!     fn title(&self) -> &str { "Home" }
//!     fn draw(&self, frame: &mut Frame, area: Rect) {
//!         frame.render_widget(Paragraph::new("Home content"), area);
//!     }
//! }
//!
//! struct MyApp;
//!
//! impl Component for MyApp {
//!     fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
//!         // Draw tab bar and content
//!         ctx.tabs().draw_tabbar(frame, tab_bar_area);
//!         ctx.tabs().draw_content(frame, content_area);
//!     }
//!
//!     fn handle_event(&mut self, event: &Event, ctx: &mut AppContext) -> EventResult {
//!         // Navigate with Tab key
//!         if event.is_key(KeyCode::Tab) {
//!             ctx.tabs().select_next();
//!             return EventResult::Handled;
//!         }
//!         EventResult::Unhandled
//!     }
//! }
//!
//! let app = AppBuilder::new()
//!     .main_ui(MyApp)
//!     .add_tab(HomeTab)
//!     .add_tab(SettingsTab)
//!     .build()?;
//! ```

pub mod app;
pub mod bus;
pub mod component;
pub mod context;
pub mod dev_console_layer;
pub mod event;
pub mod focus;
pub mod macros;
pub mod tabs;
pub mod task;
pub mod task_manager;
pub mod terminal;
pub mod theme;
pub mod widget;

// Re-export main types at crate root for convenience
pub use app::{App, AppBuilder, AppError, BuildError};
pub use bus::{MessageBus, SendError, TaskMessage, TaskSender, TrySendError};
pub use component::{BoxedComponent, Component, ComponentExt, MainUi};
pub use context::{
    traits::{CanQuit, CanSpawnBlocking, HasFocus, HasModal, HasTabs, HasTaskManager, HasTerminal},
    AppContext, DrawContext, FocusDrawContext, FocusEventContext, LifecycleContext,
    ModalEventContext, TabsDrawContext, TabsEventContext,
};
pub use dev_console_layer::DevConsoleLayer;
pub use event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
pub use focus::{EventResult, FocusManager};
pub use tabs::{TabInfo, TabManager, TabMut, TabRef};
pub use task::{Task, TaskContext, TaskHandle};
pub use task_manager::{SpawnError, TaskManager, TaskManagerContext};
pub use terminal::{install_panic_hook, Terminal, TerminalConfig, TerminalError};
pub use theme::Theme;

// The match_task_message macro is automatically exported at crate root by #[macro_export]

// Conditionally re-export blocking task helpers
#[cfg(feature = "blocking-tasks")]
pub use task::{spawn_blocking, spawn_blocking_or_err, BlockingHandle, BlockingTaskError};

// Re-export widget types
pub use widget::{
    BoxedControl, Column, ColumnAlign, ColumnType, ColumnWidth, Control, ControlEvent, ControlExt,
    CursorBlinkConfig, DataTable, DataTableConfig, DataTableEvent, DevConsole, LogLine, Modal,
    ModalButton, ModalConfig, ModalInput, ModalManager, ModalResult, SelectionMode, SimpleRow,
    SortDirection, SortState, TabBar, TabContent, TableRow, TextBox, TextBoxBuilder, TextBoxConfig,
    TextBoxEvent,
};

// Re-export ratatui types that users commonly need
pub use ratatui::{layout::Rect, Frame};
