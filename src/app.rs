//! Application builder and main event loop.
//!
//! This module provides the `AppBuilder` for constructing applications
//! and the `App` struct that runs the main event loop.

use std::time::Duration;

use crossterm::event::EventStream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::bus::{MessageBus, TaskMessage, TaskSender};
use crate::component::MainUi;
use crate::context::{AppContext, DrawContext, TabEventContext};
use crate::event::Event;
use crate::focus::FocusManager;
use crate::tabs::{Tab, TabManager};
use crate::task::{BoxedTaskFuture, Task, TaskContext, TaskFactory, TaskHandle};
use crate::terminal::{install_panic_hook, Terminal, TerminalConfig, TerminalError};
use crate::theme::Theme;
use crate::widget::ModalManager;

/// Error type for application operations.
#[derive(Debug)]
pub enum AppError {
    /// Terminal error.
    Terminal(TerminalError),
    /// Build error.
    Build(BuildError),
    /// Runtime error.
    Runtime(RuntimeError),
    /// IO error.
    Io(std::io::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Terminal(e) => write!(f, "Terminal error: {}", e),
            AppError::Build(e) => write!(f, "Build error: {}", e),
            AppError::Runtime(e) => write!(f, "Runtime error: {}", e),
            AppError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Terminal(e) => Some(e),
            AppError::Build(e) => Some(e),
            AppError::Runtime(e) => Some(e),
            AppError::Io(e) => Some(e),
        }
    }
}

impl From<TerminalError> for AppError {
    fn from(err: TerminalError) -> Self {
        AppError::Terminal(err)
    }
}

impl From<BuildError> for AppError {
    fn from(err: BuildError) -> Self {
        AppError::Build(err)
    }
}

impl From<RuntimeError> for AppError {
    fn from(err: RuntimeError) -> Self {
        AppError::Runtime(err)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

/// Error type for building an application.
#[derive(Debug)]
pub enum BuildError {
    /// No main UI was provided.
    NoMainUi,
    /// A task with the same name was already added.
    DuplicateTask(&'static str),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoMainUi => write!(f, "No main UI provided"),
            BuildError::DuplicateTask(name) => write!(f, "Duplicate task: {}", name),
        }
    }
}

impl std::error::Error for BuildError {}

/// Error type for runtime application errors.
#[derive(Debug)]
pub enum RuntimeError {
    /// The message bus receiver was already taken.
    ReceiverAlreadyTaken,
    /// A background task failed to spawn.
    TaskSpawnFailed(&'static str, std::io::Error),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::ReceiverAlreadyTaken => write!(f, "message bus receiver already taken"),
            RuntimeError::TaskSpawnFailed(name, e) => write!(f, "failed to spawn task '{}': {}", name, e),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// A pending task to be spawned when the app runs.
struct PendingTask {
    name: &'static str,
    factory: TaskFactory,
}

/// Builder for constructing a TUI application.
///
/// Use this to configure your application before running it.
///
/// # Example
///
/// ```ignore
/// use tabitha::AppBuilder;
///
/// let app = AppBuilder::new()
///     .main_ui(MyMainUi::new())
///     .add_tab(HomeTab::new())
///     .add_tab(SettingsTab::new())
///     .add_task("ticker", TickerTask::new())
///     .mouse_capture(true)
///     .build()?;
///
/// app.run().await?;
/// ```
pub struct AppBuilder<M: MainUi> {
    main_ui: Option<M>,
    tasks: Vec<PendingTask>,
    bus: MessageBus,
    tab_manager: TabManager,
    focus_manager: FocusManager,
    modal_manager: ModalManager,
    theme: Theme,
    tick_rate: Option<Duration>,
    mouse_capture: bool,
}

impl<M: MainUi + 'static> AppBuilder<M> {
    /// Create a new application builder.
    pub fn new() -> Self {
        Self {
            main_ui: None,
            tasks: Vec::new(),
            bus: MessageBus::new(),
            tab_manager: TabManager::new(),
            focus_manager: FocusManager::new(),
            modal_manager: ModalManager::new(),
            theme: Theme::default(),
            tick_rate: None,
            mouse_capture: true,
        }
    }

    /// Set the main UI component.
    ///
    /// This is required before building the application.
    pub fn main_ui(mut self, ui: M) -> Self {
        self.main_ui = Some(ui);
        self
    }

    /// Add a tab to the application.
    ///
    /// Tabs are displayed in the order they are added. The first tab
    /// added will be the initially active tab.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let app = AppBuilder::new()
    ///     .main_ui(MyApp::new())
    ///     .add_tab(HomeTab::new())
    ///     .add_tab(SettingsTab::new())
    ///     .build()?;
    /// ```
    pub fn add_tab<T: Tab + 'static>(mut self, tab: T) -> Self {
        self.tab_manager.add(tab);
        self
    }

    /// Add a background task.
    ///
    /// The task will be spawned when the application runs and will
    /// receive a typed sender for its message type.
    pub fn add_task<T: Task>(mut self, name: &'static str, task: T) -> Self {
        // Register the channel and get a sender
        let sender: TaskSender<T::Message> = self.bus.register(name);

        // Create a factory that will spawn the task with its sender
        let factory: TaskFactory = Box::new(move |ctx: TaskContext| {
            Box::pin(async move {
                task.run(sender, ctx).await;
            }) as BoxedTaskFuture
        });

        self.tasks.push(PendingTask { name, factory });
        self
    }

    /// Set an optional tick rate for periodic updates.
    ///
    /// If set, the main UI's `tick()` method will be called at this interval.
    /// Leave unset for pure event-driven operation (recommended for "quiet" apps).
    pub fn tick_rate(mut self, rate: Duration) -> Self {
        self.tick_rate = Some(rate);
        self
    }

    /// Enable or disable mouse capture.
    ///
    /// When enabled (default), mouse events will be captured and delivered
    /// to your event handlers. When disabled, mouse events are not captured.
    ///
    /// Mouse capture can also be toggled at runtime via `AppContext::set_mouse_capture()`.
    pub fn mouse_capture(mut self, enabled: bool) -> Self {
        self.mouse_capture = enabled;
        self
    }

    /// Set the application theme.
    ///
    /// The theme provides semantic color roles that components can use
    /// for consistent styling. If not set, the default dark theme is used.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::{AppBuilder, Theme};
    ///
    /// let app = AppBuilder::new()
    ///     .main_ui(MyApp::new())
    ///     .with_theme(Theme::default())
    ///     .build()?;
    /// ```
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Build the application.
    ///
    /// Returns an error if no main UI was provided.
    pub fn build(self) -> Result<App<M>, BuildError> {
        let main_ui = self.main_ui.ok_or(BuildError::NoMainUi)?;

        Ok(App {
            main_ui,
            tasks: self.tasks,
            bus: self.bus,
            tab_manager: self.tab_manager,
            focus_manager: self.focus_manager,
            modal_manager: self.modal_manager,
            theme: self.theme,
            tick_rate: self.tick_rate,
            terminal_config: TerminalConfig {
                mouse_capture: self.mouse_capture,
            },
        })
    }

    /// Register a focusable element.
    ///
    /// Elements are focused in registration order.
    /// Use this to pre-register focusable elements before the app runs.
    pub fn register_focus(mut self, id: &str) -> Self {
        self.focus_manager.register(id);
        self
    }

    /// Set initial focus to a specific element.
    ///
    /// The element must be registered (either via `register_focus` or
    /// will be registered at runtime).
    pub fn initial_focus(mut self, id: &str) -> Self {
        self.focus_manager.register(id);
        self.focus_manager.set_focus(id);
        self
    }
}

impl<M: MainUi + 'static> Default for AppBuilder<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// A configured TUI application ready to run.
pub struct App<M: MainUi> {
    main_ui: M,
    tasks: Vec<PendingTask>,
    bus: MessageBus,
    tab_manager: TabManager,
    focus_manager: FocusManager,
    modal_manager: ModalManager,
    theme: Theme,
    tick_rate: Option<Duration>,
    terminal_config: TerminalConfig,
}

impl<M: MainUi + 'static> App<M> {
    /// Run the application.
    ///
    /// This sets up the terminal, spawns background tasks, and runs
    /// the main event loop until the application quits.
    pub async fn run(mut self) -> Result<(), AppError> {
        let _span = tracing::trace_span!("app_run").entered();

        // Install panic hook for terminal restoration
        install_panic_hook();

        // Set up terminal with configuration
        let mut terminal = Terminal::with_config(self.terminal_config.clone())?;
        tracing::trace!("terminal initialized");

        // Set up cancellation for tasks
        let (cancel_tx, cancel_rx) = watch::channel(false);

        // Take the unified message receiver
        let mut message_rx = self.bus.take_receiver()
            .ok_or(RuntimeError::ReceiverAlreadyTaken)?;

        // Spawn all tasks
        let mut task_handles: Vec<TaskHandle> = Vec::with_capacity(self.tasks.len());
        for pending in self.tasks.drain(..) {
            tracing::trace!(task_name = pending.name, "spawning task");
            let ctx = TaskContext::new(cancel_rx.clone());
            let future = (pending.factory)(ctx);
            let handle = tokio::spawn(future);
            task_handles.push(TaskHandle::new(pending.name, handle));
        }
        tracing::trace!(task_count = task_handles.len(), "all tasks spawned");

        // Run the event loop
        let result = self.run_event_loop(&mut terminal, &mut message_rx).await;

        // Signal all tasks to stop
        tracing::trace!("sending cancellation signal");
        let _ = cancel_tx.send(true);

        // Wait for tasks to finish (with timeout)
        let shutdown_timeout = Duration::from_secs(2);
        for handle in task_handles {
            let task_name = handle.name;
            match tokio::time::timeout(shutdown_timeout, handle.join()).await {
                Ok(Ok(())) => tracing::trace!(task_name, "task completed"),
                Ok(Err(_)) => tracing::warn!(task_name, "task panicked"),
                Err(_) => tracing::debug!(task_name, "task shutdown timeout"),
            }
        }

        // Restore terminal
        terminal.restore()?;
        tracing::trace!("terminal restored");

        result
    }

    /// The main event loop.
    async fn run_event_loop(
        &mut self,
        terminal: &mut Terminal,
        message_rx: &mut mpsc::Receiver<TaskMessage>,
    ) -> Result<(), AppError> {
        // Create the event stream for terminal events
        let mut event_stream = EventStream::new();

        // Optional tick interval
        let mut tick_interval = self.tick_rate.map(tokio::time::interval);

        // Initial draw
        self.draw(terminal)?;

        // Track if we should quit
        let mut should_quit = false;

        loop {
            // Wait for an event
            let (needs_redraw, event_to_dispatch) = if let Some(ref mut interval) = tick_interval {
                tokio::select! {
                    biased;

                    // Terminal events (keyboard, mouse, resize)
                    event = event_stream.next() => {
                        match event {
                            Some(Ok(crossterm_event)) => {
                                let event = Event::from(crossterm_event);
                                (true, Some(event))
                            }
                            Some(Err(e)) => return Err(AppError::Io(e)),
                            None => break, // Stream ended
                        }
                    }

                    // Messages from background tasks
                    msg = message_rx.recv() => {
                        match msg {
                            Some(task_message) => {
                                let mut ctx = AppContext::new(
                                    terminal,
                                    &mut self.tab_manager,
                                    &mut self.focus_manager,
                                    &mut self.modal_manager,
                                );
                                let redraw = self.main_ui.handle_task_message(
                                    task_message.task_name,
                                    task_message.payload,
                                    &mut ctx,
                                );
                                should_quit = ctx.should_quit();
                                (redraw, None)
                            }
                            None => break, // All senders dropped
                        }
                    }

                    // Tick timer
                    _ = interval.tick() => {
                        let mut ctx = AppContext::new(
                            terminal,
                            &mut self.tab_manager,
                            &mut self.focus_manager,
                            &mut self.modal_manager,
                        );
                        self.main_ui.tick(&mut ctx);
                        should_quit = ctx.should_quit();
                        (true, None)
                    }
                }
            } else {
                // No tick timer - pure event-driven
                tokio::select! {
                    biased;

                    // Terminal events (keyboard, mouse, resize)
                    event = event_stream.next() => {
                        match event {
                            Some(Ok(crossterm_event)) => {
                                let event = Event::from(crossterm_event);
                                (true, Some(event))
                            }
                            Some(Err(e)) => return Err(AppError::Io(e)),
                            None => break, // Stream ended
                        }
                    }

                    // Messages from background tasks
                    msg = message_rx.recv() => {
                        match msg {
                            Some(task_message) => {
                                let mut ctx = AppContext::new(
                                    terminal,
                                    &mut self.tab_manager,
                                    &mut self.focus_manager,
                                    &mut self.modal_manager,
                                );
                                let redraw = self.main_ui.handle_task_message(
                                    task_message.task_name,
                                    task_message.payload,
                                    &mut ctx,
                                );
                                should_quit = ctx.should_quit();
                                (redraw, None)
                            }
                            None => {
                                // All senders dropped - if no tasks, this is expected
                                // Keep running as long as there are terminal events
                                tracing::trace!("all task senders dropped");
                                (false, None)
                            }
                        }
                    }
                }
            };

            // Dispatch event if we have one
            if let Some(event) = event_to_dispatch {
                // Three-phase event dispatch:
                //
                // Phase 0: Modal handles the event first (if open)
                // Modal events are handled by ModalManager before main UI
                let modal_consumed = self.modal_manager.handle_event(&event);

                // Phase 1: MainUi handles the event (can handle quit, tab switching, etc.)
                // Also allows MainUi to check modal results
                let main_result = {
                    let mut ctx = AppContext::new(
                        terminal,
                        &mut self.tab_manager,
                        &mut self.focus_manager,
                        &mut self.modal_manager,
                    );
                    let result = if modal_consumed {
                        // Modal consumed the event, but still call MainUi
                        // so it can check for modal results
                        self.main_ui.handle_event(&event, &mut ctx);
                        crate::focus::EventResult::StopPropagation
                    } else {
                        self.main_ui.handle_event(&event, &mut ctx)
                    };
                    should_quit = ctx.should_quit();
                    result
                };

                // Phase 2: If MainUi didn't handle it, delegate to active tab
                // Uses TabEventContext which doesn't include TabManager, avoiding borrow conflicts
                if main_result.should_propagate() && !should_quit {
                    let mut tab_ctx = TabEventContext::new(terminal, &mut self.focus_manager);
                    self.tab_manager.handle_event(&event, &mut tab_ctx);
                    should_quit = should_quit || tab_ctx.should_quit();
                }
            }

            // Check if we should quit
            if should_quit {
                break;
            }

            // Redraw if needed
            if needs_redraw {
                self.draw(terminal)?;
            }
        }

        Ok(())
    }

    /// Draw the UI.
    fn draw(&mut self, terminal: &mut Terminal) -> Result<(), AppError> {
        // Use dimmed theme for main UI when modal is open
        let main_theme = if self.modal_manager.is_open() {
            self.theme.dimmed()
        } else {
            self.theme.clone()
        };

        let draw_ctx = DrawContext::new(&self.tab_manager, &self.focus_manager, &main_theme);
        terminal.draw(|frame| {
            let area = frame.area();
            // Draw main UI first (with potentially dimmed theme)
            self.main_ui.draw(frame, area, &draw_ctx);
            // Draw modal on top (if open) - modal uses full theme colors
            self.modal_manager.draw(frame, area, &self.theme);
        })?;
        Ok(())
    }
}
