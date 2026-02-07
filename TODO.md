# TODO

## Async Tick Coordinator

Design an event loop / async manager that coordinates ticks across all async tasks.

### Core Concept

A central coordinator that all async tasks register with, specifying their timing requirements. The coordinator calculates an optimal wakeup schedule that satisfies all registered tasks while minimizing total wakeups.

### Registration API

Tasks register with parameters like:
- **interval**: desired tick frequency (e.g. "every 5 seconds")
- **tolerance window**: acceptable jitter before/after the ideal tick (e.g. +/- 500ms)
- **failure callback**: invoked if the coordinator cannot meet the timing requirement

The coordinator uses the tolerance windows to batch and coalesce wakeups — finding the least-frequent schedule that fits within every registered task's acceptable window.

### Animation Manager

The animation manager would itself be an async task registered with the coordinator. It requests ticks at whatever framerate animations need, and the coordinator folds those wakeups into the global schedule.

### User-Imposed Limits

A user can artificially cap tick frequency (e.g. "no more than one tick per second"). The coordinator respects this ceiling when computing the schedule, and tasks whose tolerance windows can't be satisfied get their failure callbacks invoked.

## Input Event Coalescing

Sync input events (key, mouse, touch) with the tick coordinator so that:

- Input debouncing is tied to the event loop frequency rather than ad-hoc timers
- Rapid key repeats, mouse moves, and touch events are coalesced within tick windows
- The coordinator can batch input processing with other scheduled work in the same wakeup

## TextBox Improvements

### Single-Line and Multi-Line Modes

TextBox should support both single-line and multi-line modes:
- **Single-line mode**: Enter submits, ignores newlines
- **Multi-line mode**: Shift+Enter creates new line, Enter submits (or configurable)
- Mode should be settable at construction or togglable at runtime

### Text Wrapping Support

Implement text wrapping for long lines:
- Soft wraps (visual only, no actual newline in content)
- Configurable wrap width or fit-to-container
- Proper cursor navigation across wrapped lines
- Selection should work correctly across visual line breaks

## Collapsible Sections

### Togglable Line Unfolding

Lines can unfold into sections with configurable presentation:
- **With border**: unfolded section has its own border/box
- **Without border**: unfolded content flows inline
- Click or keyboard shortcut to toggle collapse/expand
- Visual indicator (▶/▼) for collapsed/expanded state
- Nested folding support

## Tree Widget

Implement a tree widget with togglable items:
- Hierarchical structure with expand/collapse per node
- Keyboard navigation (arrows, Enter to toggle)
- Mouse click to toggle expansion
- Visual indicators for expandable nodes
- Lazy loading support for large trees
- Selection and focus states
