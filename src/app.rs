//! Application builder and main event loop.
//!
//! This module provides the `AppBuilder` for constructing applications
//! and the `App` struct that runs the main event loop.

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::EventStream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::animation::AnimationController;
use crate::bus::{MessageBus, MessageBusReceiver, TaskMessage, TaskSender};
use crate::component::{Component, MainUi};
use crate::context::traits::CanQuit;
use crate::context::{AppContext, DrawContext};
use crate::event::{AppEvent, Event, KeyCode};
use crate::focus::FocusManager;
use crate::tabs::TabManager;
use crate::task::{
    BoxedTaskFuture, CongestionController, Task, TaskContext, TaskFactory, TaskHandle,
};
use crate::task_manager::TaskManager;
use crate::terminal::{install_panic_hook, Terminal, TerminalConfig, TerminalError};
use crate::theme::Theme;
use crate::widget::{DevConsole, ModalManager};

/// Default buffer size for the event channel.
const EVENT_CHANNEL_SIZE: usize = 256;

/// Default timeout for task shutdown in seconds.
const SHUTDOWN_TIMEOUT_SECONDS: u64 = 2;

/// Error type for application operations.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Terminal error.
    #[error("Terminal error: {0}")]
    Terminal(#[from] TerminalError),
    /// Build error.
    #[error("Build error: {0}")]
    Build(#[from] BuildError),
    /// Runtime error.
    #[error("Runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Error type for building an application.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    /// No main UI was provided.
    #[error("No main UI provided")]
    NoMainUi,
    /// A task with the same name was already added.
    #[error("Duplicate task: {0}")]
    DuplicateTask(&'static str),
}

/// Error type for runtime application errors.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    /// The message bus receiver was already taken.
    #[error("message bus receiver already taken")]
    ReceiverAlreadyTaken,
    /// A background task failed to spawn.
    #[error("failed to spawn task '{0}': {1}")]
    TaskSpawnFailed(&'static str, std::io::Error),
    /// The event channel was closed.
    #[error("event channel closed")]
    EventChannelClosed,
    /// An event loop task panicked.
    #[error("event loop task '{0}' panicked")]
    TaskPanicked(&'static str),
}

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
    bus_receiver: MessageBusReceiver,
    tab_manager: TabManager,
    focus_manager: FocusManager,
    modal_manager: ModalManager,
    theme: Theme,
    tick_rate: Option<Duration>,
    mouse_capture: bool,
    dev_console_enabled: bool,
    log_rx: Option<mpsc::UnboundedReceiver<crate::widget::LogLine>>,
}

impl<M: MainUi + 'static> AppBuilder<M> {
    /// Create a new application builder.
    pub fn new() -> Self {
        let (bus, bus_receiver) = MessageBusReceiver::new();
        Self {
            main_ui: None,
            tasks: Vec::new(),
            bus,
            bus_receiver,
            tab_manager: TabManager::new(),
            focus_manager: FocusManager::new(),
            modal_manager: ModalManager::new(),
            theme: Theme::default(),
            tick_rate: None,
            mouse_capture: true,
            dev_console_enabled: false,
            log_rx: None,
        }
    }

    /// Enable the developer console.
    ///
    /// When enabled, press `~` (backtick) to toggle the developer console
    /// overlay which displays log messages and debug information.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let app = AppBuilder::new()
    ///     .main_ui(MyApp::new())
    ///     .enable_dev_console(true)
    ///     .build()?;
    /// ```
    pub fn enable_dev_console(mut self, enabled: bool) -> Self {
        self.dev_console_enabled = enabled;
        self
    }

    /// Set the log receiver for the developer console.
    ///
    /// This is typically set by `TabithaArgs::init_tracing()` when `--dev` is used.
    pub fn with_log_receiver(
        mut self,
        rx: Option<mpsc::UnboundedReceiver<crate::widget::LogLine>>,
    ) -> Self {
        self.log_rx = rx;
        self
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
    ///     .add_tab("home", "Home", HomeTab::new())
    ///     .add_tab("settings", "Settings", SettingsTab::new())
    ///     .build()?;
    /// ```
    pub fn add_tab(
        mut self,
        id: &'static str,
        title: impl Into<String>,
        component: impl Component + 'static,
    ) -> Self {
        use crate::tabs::TabInfo;
        let info = TabInfo::new(id, title);
        self.tab_manager.add(info, component);
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
                match task.run(sender, ctx).await {
                    Ok(()) => {
                        tracing::info!(task_name = name, "Build-time task completed successfully");
                    }
                    Err(e) => {
                        tracing::error!(task_name = name, error = %e, "Build-time task failed with error");
                    }
                }
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

        let dev_console = if self.dev_console_enabled {
            DevConsole::new()
        } else {
            // Create a hidden console that won't be rendered
            let mut console = DevConsole::new();
            console.hide();
            console
        };

        Ok(App {
            main_ui,
            tasks: self.tasks,
            bus: self.bus,
            bus_receiver: Some(self.bus_receiver),
            tab_manager: self.tab_manager,
            focus_manager: self.focus_manager,
            modal_manager: self.modal_manager,
            animation_controller: AnimationController::new(),
            theme: self.theme,
            tick_rate: self.tick_rate,
            terminal_config: TerminalConfig {
                mouse_capture: self.mouse_capture,
            },
            dev_console,
            dev_console_enabled: self.dev_console_enabled,
            log_rx: self.log_rx,
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
    bus_receiver: Option<MessageBusReceiver>,
    tab_manager: TabManager,
    focus_manager: FocusManager,
    modal_manager: ModalManager,
    animation_controller: AnimationController,
    theme: Theme,
    tick_rate: Option<Duration>,
    terminal_config: TerminalConfig,
    dev_console: DevConsole,
    dev_console_enabled: bool,
    log_rx: Option<mpsc::UnboundedReceiver<crate::widget::LogLine>>,
}

impl<M: MainUi + 'static> App<M> {
    /// Run the application.
    ///
    /// This sets up the terminal, spawns background tasks, and runs
    /// the main event loop until the application quits.
    ///
    /// Uses structured concurrency with fail-fast semantics: if any
    /// event source task fails, the entire application stops.
    pub async fn run(mut self) -> Result<(), AppError> {
        let _span = tracing::trace_span!("app_run").entered();

        // Install panic hook for terminal restoration
        install_panic_hook();

        // Set up terminal with configuration
        let mut terminal = Terminal::with_config(self.terminal_config.clone())?;
        tracing::trace!("terminal initialized");

        // Set up cancellation for tasks
        let (cancel_tx, cancel_rx) = watch::channel(false);

        // Create shared congestion controller for backpressure management
        let congestion = Arc::new(CongestionController::default());

        // Create the task manager for runtime task spawning
        let mut task_manager = TaskManager::with_congestion(
            cancel_tx.clone(),
            self.bus.clone(),
            Arc::clone(&congestion),
        );

        // Take the unified message receiver
        let message_rx = self
            .bus_receiver
            .take()
            .ok_or(RuntimeError::ReceiverAlreadyTaken)?
            .take();

        // Spawn all build-time background tasks
        let mut task_handles: Vec<TaskHandle> = Vec::with_capacity(self.tasks.len());
        for pending in self.tasks.drain(..) {
            tracing::trace!(task_name = pending.name, "spawning build-time task");
            let ctx = TaskContext::new(cancel_rx.clone(), Arc::clone(&congestion));
            let future = (pending.factory)(ctx);
            let handle = tokio::spawn(future);
            task_handles.push(TaskHandle::new(pending.name, handle));
        }
        tracing::trace!(
            task_count = task_handles.len(),
            "all build-time tasks spawned"
        );

        // Create unified event channel for structured concurrency
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_SIZE);

        // Initial draw before starting event loop
        self.draw(&mut terminal)?;

        // Run event sources concurrently with fail-fast semantics
        let result = self
            .run_with_structured_concurrency(
                &mut terminal,
                event_tx,
                event_rx,
                message_rx,
                &mut task_manager,
                &mut task_handles,
            )
            .await;

        // Signal all tasks to stop
        tracing::trace!("sending cancellation signal");
        let _ = cancel_tx.send(true);

        // Collect runtime task handles and combine with build-time handles
        let mut all_handles: Vec<TaskHandle> = task_manager.take_handles();
        all_handles.extend(task_handles);

        // Wait for all tasks to finish (with shared timeout deadline to avoid accumulation)
        let shutdown_deadline =
            tokio::time::Instant::now() + Duration::from_secs(SHUTDOWN_TIMEOUT_SECONDS);
        for handle in all_handles {
            let task_name = handle.name;
            match tokio::time::timeout_at(shutdown_deadline, handle.join()).await {
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

    /// Run the application with structured concurrency.
    ///
    /// Spawns separate tasks for:
    /// - Input processing (terminal events)
    /// - Task message coordination
    ///
    /// The main loop runs in the current task for &mut self access.
    /// Uses `tokio::select!` for fail-fast behavior - if any task fails,
    /// the select returns immediately with the error.
    async fn run_with_structured_concurrency(
        &mut self,
        terminal: &mut Terminal,
        event_tx: mpsc::Sender<AppEvent>,
        event_rx: mpsc::Receiver<AppEvent>,
        message_rx: mpsc::Receiver<TaskMessage>,
        task_manager: &mut TaskManager,
        build_time_handles: &mut Vec<TaskHandle>,
    ) -> Result<(), AppError> {
        // Clone event_tx for each event source task
        let input_tx = event_tx.clone();
        let task_tx = event_tx.clone();

        // Spawn event source tasks
        let mut input_handle = tokio::spawn(Self::input_processor(input_tx));

        let mut task_handle = tokio::spawn(Self::task_coordinator(task_tx, message_rx));

        // Run main loop concurrently with event sources using pin! for select
        let main_loop_fut =
            std::pin::pin!(self.main_loop(terminal, event_rx, task_manager, build_time_handles));

        // Fail-fast: if any task completes (success or error), abort the others
        let result = tokio::select! {
            biased;

            // Event source tasks - propagate errors immediately (fail-fast)
            result = &mut input_handle => {
                // Task completed - abort the other
                task_handle.abort();
                // Propagate any join error, then the task result
                result.map_err(|_| AppError::Runtime(RuntimeError::TaskPanicked("input_processor")))?
            }
            result = &mut task_handle => {
                // Task completed - abort the other
                input_handle.abort();
                result.map_err(|_| AppError::Runtime(RuntimeError::TaskPanicked("task_coordinator")))?
            }

            // Main loop task
            result = main_loop_fut => {
                // Main loop completed - abort event sources
                input_handle.abort();
                task_handle.abort();
                result
            }
        };

        // Wait for aborted tasks to finish (they should exit quickly)
        let _ = tokio::join!(input_handle, task_handle);

        result
    }

    /// Input processor task.
    ///
    /// Reads terminal events and sends them as AppEvent::Terminal.
    /// Also handles tick events when tick_rate is set.
    ///
    /// Implements rate limiting: key events are throttled to prevent saturating
    /// the event channel and blocking animation ticks when holding keys.
    async fn input_processor(event_tx: mpsc::Sender<AppEvent>) -> Result<(), AppError> {
        use tokio::time::Instant;

        let mut event_stream = EventStream::new();

        // Rate limiting state
        const MIN_KEY_INTERVAL_MS: u64 = 16; // ~60fps max for key events
        let mut last_key_time: Option<Instant> = None;

        loop {
            match event_stream.next().await {
                Some(Ok(crossterm_event)) => {
                    let event = Event::from(crossterm_event);
                    tracing::trace!(?event, "input_processor: received terminal event");

                    // For key events, apply rate limiting
                    if matches!(event, Event::Key(_)) {
                        let now = Instant::now();

                        // Check if enough time has passed since last key event
                        if let Some(last_time) = last_key_time {
                            let elapsed = now.duration_since(last_time);
                            if elapsed < Duration::from_millis(MIN_KEY_INTERVAL_MS) {
                                // Rate limit: drop this key event
                                tracing::trace!(
                                    elapsed_ms = elapsed.as_millis(),
                                    "input_processor: rate limiting key event"
                                );
                                continue;
                            }
                        }

                        // Update last key time and send the event
                        last_key_time = Some(now);
                        event_tx
                            .send(AppEvent::Terminal(event))
                            .await
                            .map_err(|_| AppError::Runtime(RuntimeError::EventChannelClosed))?;
                    } else {
                        // Non-key events (resize, mouse, focus) sent immediately
                        event_tx
                            .send(AppEvent::Terminal(event))
                            .await
                            .map_err(|_| AppError::Runtime(RuntimeError::EventChannelClosed))?;
                    }
                }
                Some(Err(e)) => return Err(AppError::Io(e)),
                None => {
                    tracing::trace!("input_processor: event stream ended");
                    return Ok(());
                }
            }
        }
    }

    /// Task coordinator task.
    ///
    /// Receives messages from background tasks and forwards them as AppEvent::TaskMessage.
    async fn task_coordinator(
        event_tx: mpsc::Sender<AppEvent>,
        mut message_rx: mpsc::Receiver<TaskMessage>,
    ) -> Result<(), AppError> {
        loop {
            match message_rx.recv().await {
                Some(task_message) => {
                    tracing::trace!(
                        task_name = task_message.task_name,
                        "task_coordinator: received task message"
                    );
                    event_tx
                        .send(AppEvent::TaskMessage(task_message))
                        .await
                        .map_err(|_| AppError::Runtime(RuntimeError::EventChannelClosed))?;
                }
                None => {
                    tracing::trace!("task_coordinator: all senders dropped");
                    return Ok(());
                }
            }
        }
    }

    /// Main event loop.
    ///
    /// Processes unified AppEvents from all sources.
    ///
    /// Animation ticks are tracked independently using elapsed time, ensuring
    /// they fire at the correct rate regardless of event processing.
    async fn main_loop(
        &mut self,
        terminal: &mut Terminal,
        event_rx: mpsc::Receiver<AppEvent>,
        task_manager: &mut TaskManager,
        build_time_handles: &mut Vec<TaskHandle>,
    ) -> Result<(), AppError> {
        use tokio::time::Instant;

        let mut event_rx = event_rx;
        let mut should_quit = false;

        // Animation timing - tracked manually to ensure ticks happen at the right rate
        let tick_duration = self.tick_rate;
        let mut last_tick = Instant::now();

        // Call on_mount for main_ui before starting the event loop
        {
            let mut lifecycle_ctx = crate::context::LifecycleContext::new(&mut self.focus_manager);
            self.main_ui.on_mount(&mut lifecycle_ctx);
        }

        loop {
            // Clean up finished runtime tasks periodically
            task_manager.cleanup_finished();

            let mut needs_redraw = false;

            // Calculate how long until next tick (or wait indefinitely if no animations)
            let wait_duration = tick_duration.map(|d| {
                let elapsed = last_tick.elapsed();
                if elapsed >= d {
                    Duration::ZERO
                } else {
                    d - elapsed
                }
            });

            // Wait for either an event or the next tick time
            let event = if let Some(wait) = wait_duration {
                if wait.is_zero() {
                    // Tick is due now, don't wait - just check for events non-blocking
                    event_rx.try_recv().ok()
                } else {
                    tokio::select! {
                        biased;

                        // Timeout for next animation tick
                        _ = tokio::time::sleep(wait) => None,

                        // Event from channel
                        event = event_rx.recv() => event,
                    }
                }
            } else {
                // No animations - just wait for events
                event_rx.recv().await
            };

            // Process the event if we got one
            if let Some(event) = event {
                needs_redraw |= self.process_app_event(
                    event,
                    terminal,
                    task_manager,
                    build_time_handles,
                    &mut should_quit,
                )?;
            } else if wait_duration.is_none() {
                // Channel closed and no animations
                tracing::trace!("main_loop: event channel closed");
                break;
            }

            // Always check if animation tick is due (after processing event)
            // This ensures ticks happen even when events are queued
            if let Some(d) = tick_duration {
                if last_tick.elapsed() >= d {
                    tracing::trace!("main_loop: animation tick");
                    last_tick = Instant::now();
                    needs_redraw |= self.process_app_event(
                        AppEvent::Tick,
                        terminal,
                        task_manager,
                        build_time_handles,
                        &mut should_quit,
                    )?;
                }
            }

            // Poll log receiver and feed logs to dev console
            if let Some(ref mut log_rx) = self.log_rx {
                while let Ok(log_line) = log_rx.try_recv() {
                    self.dev_console.push(log_line);
                    needs_redraw = true;
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

    /// Process a single AppEvent.
    ///
    /// Returns true if a redraw is needed.
    fn process_app_event(
        &mut self,
        event: AppEvent,
        terminal: &mut Terminal,
        task_manager: &mut TaskManager,
        _build_time_handles: &mut Vec<TaskHandle>,
        should_quit: &mut bool,
    ) -> Result<bool, AppError> {
        match event {
            AppEvent::Terminal(event) => {
                self.dispatch_terminal_event(&event, terminal, task_manager, should_quit)
            }
            AppEvent::TaskMessage(task_message) => {
                let mut ctx = AppContext::with_task_manager(
                    terminal,
                    &mut self.tab_manager,
                    &mut self.focus_manager,
                    &mut self.modal_manager,
                    task_manager,
                    &mut self.animation_controller,
                );
                let redraw = self.main_ui.handle_task_message(
                    task_message.task_name,
                    task_message.payload,
                    &mut ctx,
                );
                *should_quit = ctx.should_quit();
                Ok(redraw)
            }
            AppEvent::Tick => {
                // Tick animations first
                let tick_duration = self.tick_rate.unwrap_or(Duration::from_millis(16));
                let _needs_redraw = self.animation_controller.tick(tick_duration);

                let mut ctx = AppContext::with_task_manager(
                    terminal,
                    &mut self.tab_manager,
                    &mut self.focus_manager,
                    &mut self.modal_manager,
                    task_manager,
                    &mut self.animation_controller,
                );
                self.main_ui.tick(&mut ctx);
                *should_quit = ctx.should_quit();
                Ok(true)
            }
        }
    }

    /// Dispatch a terminal event through the component hierarchy.
    fn dispatch_terminal_event(
        &mut self,
        event: &Event,
        terminal: &mut Terminal,
        task_manager: &mut TaskManager,
        should_quit: &mut bool,
    ) -> Result<bool, AppError> {
        // Check for dev console toggle first (` key)
        if self.dev_console_enabled
            && matches!(event, Event::Key(key) if key.code == KeyCode::Char('`'))
        {
            self.dev_console.toggle();
            return Ok(true);
        }

        // Three-phase event dispatch:
        //
        // Phase 0: Modal handles the event first (if open)
        let modal_consumed = self.modal_manager.handle_event(event);

        // Phase 1: MainUi handles the event
        let _main_result = {
            let mut ctx = AppContext::with_task_manager(
                terminal,
                &mut self.tab_manager,
                &mut self.focus_manager,
                &mut self.modal_manager,
                task_manager,
                &mut self.animation_controller,
            );
            let result = if modal_consumed {
                self.main_ui.handle_event(event, &mut ctx);
                crate::focus::EventResult::StopPropagation
            } else {
                self.main_ui.handle_event(event, &mut ctx)
            };
            *should_quit = ctx.should_quit();
            result
        };

        // Phase 2: If MainUi didn't handle it, events are not propagated further
        // Tabs now receive events through the MainUi or through TabContent widget
        // which calls ctx.tabs().forward_event()

        Ok(true)
    }

    /// Draw the UI.
    fn draw(&mut self, terminal: &mut Terminal) -> Result<(), AppError> {
        tracing::trace!("drawing frame");

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
            // Draw developer console on top of everything (if visible)
            self.dev_console.draw(frame, area, &self.theme);
        })?;
        Ok(())
    }
}
