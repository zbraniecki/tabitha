//! Application builder and main event loop.
//!
//! This module provides the `AppBuilder` for constructing applications
//! and the `App` struct that runs the main event loop.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;

use crate::animation::AnimationController;
use crate::bus::{MessageBus, MessageBusReceiver, TaskMessage, TaskSender};
use crate::component::{Component, MainUi};
use crate::context::traits::CanQuit;
use crate::context::{AppContext, DrawContext};
use crate::event::{AppEvent, Event};
use crate::focus::FocusManager;
use crate::tabs::TabManager;
use crate::task::{
    BoxedTaskFuture, CongestionController, Task, TaskContext, TaskFactory, TaskHandle,
};
use crate::task_manager::TaskManager;
use crate::terminal::{install_panic_hook, Terminal, TerminalConfig, TerminalError};
use crate::theme::Theme;
use crate::widget::log_viewer::LogLine;
use crate::widget::{DevOverlayManager, FrameTrigger, ModalManager};

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
    log_rx: Option<mpsc::UnboundedReceiver<LogLine>>,
    /// Selection configuration (only available with selection feature).
    #[cfg(feature = "selection")]
    selection_config: crate::selection::SelectionConfig,
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
            log_rx: None,
            #[cfg(feature = "selection")]
            selection_config: crate::selection::SelectionConfig::default(),
        }
    }

    /// Set the log receiver for the log viewer.
    ///
    /// This allows the developer overlay log viewer to receive log messages
    /// from a tracing layer. The log viewer can be toggled via the
    /// dev_overlays context in your event handler.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::{AppBuilder, DevConsoleLayer};
    /// use tokio::sync::mpsc;
    ///
    /// let (tx, rx) = mpsc::unbounded_channel();
    /// let layer = DevConsoleLayer::new(tx);
    ///
    /// let app = AppBuilder::new()
    ///     .main_ui(MyApp::new())
    ///     .with_log_receiver(Some(rx))
    ///     .build()?;
    /// ```
    pub fn with_log_receiver(mut self, rx: Option<mpsc::UnboundedReceiver<LogLine>>) -> Self {
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

    /// Set the selection configuration (requires `selection` feature).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use tabitha::{AppBuilder, SelectionConfig};
    ///
    /// let app = AppBuilder::new()
    ///     .main_ui(MyApp::new())
    ///     .selection_config(SelectionConfig::without_auto_copy())
    ///     .build()?;
    /// ```
    #[cfg(feature = "selection")]
    pub fn selection_config(mut self, config: crate::selection::SelectionConfig) -> Self {
        self.selection_config = config;
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
            dev_overlay_manager: DevOverlayManager::new(self.log_rx),
            last_frame_trigger: None,
            #[cfg(feature = "selection")]
            selection_manager: crate::selection::SelectionManager::with_config(
                self.selection_config,
            ),
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
    dev_overlay_manager: DevOverlayManager,
    /// Track what triggered the last frame for the debug panel
    last_frame_trigger: Option<FrameTrigger>,
    /// Selection manager for mouse-based text selection (requires `selection` feature).
    #[cfg(feature = "selection")]
    selection_manager: crate::selection::SelectionManager,
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
        self.last_frame_trigger = Some(FrameTrigger::Other("initial".to_string()));
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
                    if matches!(event, Event::Key(key_event) if key_event.kind == KeyEventKind::Press) {
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
    /// Uses dynamic tick scheduling: when no animations or periodic updates
    /// are active, the loop blocks on `event_rx.recv().await` with zero CPU.
    /// When animations are running, it uses `tokio::select!` between tick
    /// timeout and events at the configured tick rate.
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

        let mut ticking = false;
        let mut last_tick = Instant::now();

        // Call on_mount for main_ui before starting the event loop
        {
            let mut lifecycle_ctx = crate::context::LifecycleContext::new(&mut self.focus_manager);
            self.main_ui.on_mount(&mut lifecycle_ctx);
        }

        // Do initial tick to register animations and determine if ticking is needed
        if self.tick_rate.is_some() {
            let (needs_redraw, keep_ticking) =
                self.do_tick(Duration::ZERO, terminal, task_manager, &mut should_quit)?;
            ticking = keep_ticking;
            if needs_redraw {
                self.draw(terminal)?;
            }
        }

        loop {
            // Clean up finished runtime tasks periodically
            task_manager.cleanup_finished();

            let mut needs_redraw = false;

            // Wait for event (and optionally tick timeout)
            let event = if ticking {
                let wait = self
                    .tick_rate
                    .map(|d| d.saturating_sub(last_tick.elapsed()))
                    .unwrap_or(Duration::from_millis(16));
                if wait.is_zero() {
                    event_rx.try_recv().ok()
                } else {
                    tokio::select! {
                        biased;
                        _ = tokio::time::sleep(wait) => None,
                        event = event_rx.recv() => event,
                    }
                }
            } else {
                // IDLE MODE: block until event — zero CPU
                event_rx.recv().await
            };

            // Process event
            if let Some(event) = event {
                needs_redraw |= self.process_app_event(
                    event,
                    terminal,
                    task_manager,
                    build_time_handles,
                    &mut should_quit,
                )?;
            } else if !ticking {
                // Channel closed while idle
                tracing::trace!("main_loop: event channel closed");
                break;
            }

            // Tick (always, after event or timeout) if tick_rate is configured
            if self.tick_rate.is_some() {
                let now = Instant::now();
                let elapsed = now.duration_since(last_tick);
                last_tick = now;

                let (tick_redraw, keep_ticking) =
                    self.do_tick(elapsed, terminal, task_manager, &mut should_quit)?;
                needs_redraw |= tick_redraw;

                // Transition idle <-> tick mode
                let was_ticking = ticking;
                ticking = keep_ticking;
                if ticking && !was_ticking {
                    last_tick = Instant::now(); // Fresh start for tick timing
                }
            }

            // Poll log receiver and feed logs to dev overlay
            let logs_polled = self.dev_overlay_manager.poll_logs();
            needs_redraw |= logs_polled;

            // Check if we should quit
            if should_quit {
                break;
            }

            // Redraw if needed
            if needs_redraw {
                #[cfg(feature = "clipboard")]
                let had_selection = self.selection_manager.has_selection();

                self.draw(terminal)?;

                // If auto-copy cleared the selection during draw, redraw
                // immediately to remove the selection overlay from the screen.
                #[cfg(feature = "clipboard")]
                if had_selection && !self.selection_manager.has_selection() {
                    self.draw(terminal)?;
                }
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
        // Track what triggered this frame for the debug panel
        let trigger = match &event {
            AppEvent::Terminal(event) => DevOverlayManager::event_to_trigger(event),
            AppEvent::TaskMessage(task_message) => {
                FrameTrigger::TaskMessage(task_message.task_name.to_string())
            }
            AppEvent::Tick => FrameTrigger::Tick,
        };

        // Store the trigger for later use in draw()
        self.last_frame_trigger = Some(trigger);

        match event {
            AppEvent::Terminal(event) => {
                self.dispatch_terminal_event(&event, terminal, task_manager, should_quit)
            }
            AppEvent::TaskMessage(task_message) => {
                let before_handle = Instant::now();
                let mut ctx = AppContext::with_task_manager_and_overlays(
                    terminal,
                    &mut self.tab_manager,
                    &mut self.focus_manager,
                    &mut self.modal_manager,
                    task_manager,
                    &mut self.animation_controller,
                    &mut self.dev_overlay_manager,
                );
                let redraw = self.main_ui.handle_task_message(
                    task_message.task_name,
                    task_message.payload,
                    &mut ctx,
                );
                *should_quit = ctx.should_quit();
                let handle_time = before_handle.elapsed();
                if handle_time.as_millis() > 0 {
                    tracing::debug!(
                        task_name = task_message.task_name,
                        ?handle_time,
                        "task message handled slowly"
                    );
                }
                Ok(redraw)
            }
            AppEvent::Tick => {
                // Ticks are now handled by do_tick() in the main loop.
                // This arm is kept for completeness but should not be reached.
                Ok(false)
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
        // Multi-phase event dispatch:
        //
        // Phase -1: Dev overlays (log viewer) handles events when visible
        // The log viewer consumes all keyboard input for filtering/navigation
        if self.dev_overlay_manager.is_log_viewer_visible() {
            let result = self.dev_overlay_manager.handle_event(event);
            if result.is_handled() {
                return Ok(true);
            }
        }
        //
        // Phase -0.5: Mouse selection (feature-gated, mouse-capture-gated)
        // Selection handles mouse events and Ctrl+C when selection is active
        #[cfg(feature = "selection")]
        {
            if terminal.mouse_capture_enabled() {
                // Handle mouse events for selection

                use crossterm::event::KeyEventKind;
                if let Event::Mouse(mouse_event) = event {
                    if self.selection_manager.handle_mouse_event(mouse_event) {
                        // Selection consumed the event
                        return Ok(true);
                    }
                }

                // Handle Ctrl+C when selection is active (copy instead of quit)
                if let Event::Key(key) = event && key.kind == KeyEventKind::Press {
                    if key.code == crate::event::KeyCode::Char('c')
                        && key.modifiers.contains(crate::event::KeyModifiers::CONTROL)
                        && self.selection_manager.has_selection()
                    {
                        // Copy to clipboard and clear selection
                        #[cfg(feature = "clipboard")]
                        {
                            if let Some(text) = self.selection_manager.selected_text() {
                                if let Err(e) = crate::selection::clipboard::copy_to_clipboard(text)
                                {
                                    tracing::warn!("Failed to copy to clipboard: {}", e);
                                }
                            }
                        }
                        self.selection_manager.clear_selection();
                        return Ok(true);
                    }
                }

                // Clear selection on any other key event
                if matches!(event, Event::Key(_)) && self.selection_manager.has_selection() {
                    self.selection_manager.clear_selection();
                }
            }
        }

        //
        // Phase 0: Modal handles the event first (if open)
        let modal_consumed = self.modal_manager.handle_event(event);

        // Phase 1: MainUi handles the event
        let main_result = {
            let mut ctx = AppContext::with_task_manager_and_overlays(
                terminal,
                &mut self.tab_manager,
                &mut self.focus_manager,
                &mut self.modal_manager,
                task_manager,
                &mut self.animation_controller,
                &mut self.dev_overlay_manager,
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

        // Only redraw if something actually handled the event
        Ok(modal_consumed || main_result.is_handled() || *should_quit)
    }

    /// Execute a single tick cycle.
    ///
    /// Returns `(needs_redraw, needs_more_ticks)`.
    fn do_tick(
        &mut self,
        elapsed: Duration,
        terminal: &mut Terminal,
        task_manager: &mut TaskManager,
        should_quit: &mut bool,
    ) -> Result<(bool, bool), AppError> {
        self.last_frame_trigger = Some(FrameTrigger::Tick);

        let animations_changed = self.animation_controller.tick(elapsed);

        let mut ctx = AppContext::with_task_manager_and_overlays(
            terminal,
            &mut self.tab_manager,
            &mut self.focus_manager,
            &mut self.modal_manager,
            task_manager,
            &mut self.animation_controller,
            &mut self.dev_overlay_manager,
        );
        let ui_changed = self.main_ui.tick(&mut ctx);
        *should_quit |= ctx.should_quit();

        let needs_redraw = animations_changed || ui_changed || *should_quit;
        let needs_more_ticks = self.animation_controller.needs_ticking() || ui_changed;

        Ok((needs_redraw, needs_more_ticks))
    }

    /// Draw the UI.
    fn draw(&mut self, terminal: &mut Terminal) -> Result<(), AppError> {
        use std::time::Instant;

        let draw_start = Instant::now();

        // Clear selection regions at the START of draw so regions registered during
        // this frame survive until the next frame's event handling.
        #[cfg(feature = "selection")]
        self.selection_manager.clear_regions();

        // Get the trigger for this frame, or use a default
        let trigger = self
            .last_frame_trigger
            .take()
            .unwrap_or_else(|| FrameTrigger::Other("draw".to_string()));

        // Use dimmed theme for main UI when modal is open
        let main_theme = if self.modal_manager.is_open() {
            self.theme.dimmed()
        } else {
            self.theme.clone()
        };

        // Create draw context with selection support
        #[cfg(feature = "selection")]
        let draw_ctx = DrawContext::with_selection(
            &self.tab_manager,
            &self.focus_manager,
            &main_theme,
            &self.selection_manager.registry,
        );

        #[cfg(not(feature = "selection"))]
        let draw_ctx = DrawContext::new(&self.tab_manager, &self.focus_manager, &main_theme);

        let before_terminal_draw = Instant::now();
        let mut main_ui_time = Duration::ZERO;
        let mut modal_time = Duration::ZERO;
        let mut overlay_time = Duration::ZERO;
        #[cfg(feature = "selection")]
        let mut extracted_text: Option<String> = None;

        terminal.draw(|frame| {
            let area = frame.area();

            let t1 = Instant::now();
            // Draw main UI first (with potentially dimmed theme)
            self.main_ui.draw(frame, area, &draw_ctx);
            main_ui_time = t1.elapsed();

            let t2 = Instant::now();
            // Draw modal on top (if open) - modal uses full theme colors
            self.modal_manager.draw(frame, area, &self.theme);
            modal_time = t2.elapsed();

            let t3 = Instant::now();
            // Draw developer overlays on top of everything
            self.dev_overlay_manager.draw(frame, area, &self.theme);
            overlay_time = t3.elapsed();

            // Selection highlight overlay: swap fg/bg on selected cells in buffer
            // Also extract selected text from the buffer for clipboard operations
            #[cfg(feature = "selection")]
            {
                crate::selection::rendering::draw_selection_overlay(frame, &self.selection_manager);

                // Extract text from buffer for the active selection.
                // Copy selection and region data first to avoid borrow conflicts with set_selected_text.
                let extract_info = self.selection_manager.selection().and_then(|sel| {
                    let selection = *sel;
                    let region_id = self.selection_manager.active_region()?.clone();
                    let region_ref = self.selection_manager.region_by_id(&region_id)?;
                    let region_clone = crate::selection::SelectionRegion::new(
                        region_ref.id.clone(),
                        region_ref.rect,
                        region_ref.z_order,
                    );
                    drop(region_ref);
                    Some((selection, region_clone))
                });

                if let Some((selection, region)) = extract_info {
                    let text = crate::selection::rendering::extract_selected_text(
                        frame.buffer_mut(),
                        &region,
                        &selection,
                    );
                    if !text.is_empty() {
                        extracted_text = Some(text);
                    } else {
                        tracing::debug!("selection: extracted text is empty");
                    }
                } else {
                    tracing::trace!("selection: no extract_info (no selection/region)");
                }
            }
        })?;

        // Set extracted text on selection manager after draw closure to avoid borrow conflicts
        #[cfg(feature = "selection")]
        if let Some(text) = extracted_text {
            self.selection_manager.set_selected_text(text);
        }
        let terminal_draw_time = before_terminal_draw.elapsed();

        // Record frame timing
        let total_render_time = draw_start.elapsed();

        // Log if terminal.draw took significant time
        if terminal_draw_time.as_micros() > 500 {
            let trigger_str = format!("{:?}", trigger);
            tracing::debug!(
                trigger = %trigger_str,
                ?terminal_draw_time,
                ?main_ui_time,
                ?modal_time,
                ?overlay_time,
                ?total_render_time,
                "slow terminal.draw breakdown"
            );
        }

        self.dev_overlay_manager
            .record_frame(total_render_time, trigger);

        Ok(())
    }
}
