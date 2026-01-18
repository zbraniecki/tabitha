# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2026-01-18

### Added
- **TextBox widget**: Single-line text input widget with cursor navigation, placeholder support, password masking, and event emission
- **DataTable widget**: Interactive data table with sorting, selection, column configuration, and keyboard navigation
- **Modal management system**: Centralized modal dialog system with support for alerts, confirmations, prompts, and custom input fields
- **Theme system**: Semantic color theming with support for custom themes and automatic dimming when modals are displayed
- Modal inline input mode for compact prompt dialogs

### Changed
- Code cleanup and improvements to example outlines

### Documentation
- Updated README with concise framework overview and architecture details

## [0.0.1] - 2026-01-14

### Added
- Initial release of the tabitha TUI framework
- **Component system**: Component trait for building UI elements with event handling and rendering
- **Event system**: Terminal event handling (keyboard, mouse, resize) with convenience methods
- **Focus management**: Automatic keyboard navigation between focusable components
- **Tab system**: Built-in tab management with enable/disable support and tab bar rendering
- **Background tasks**: Async task system with typed message channels for UI communication
- **App builder**: Builder pattern for composable application setup
- **Event loop**: Event-driven architecture that only redraws on events or state changes
- **Examples**: Counter, tabs, and focus management examples demonstrating framework usage
- Support for mouse capture (configurable at build time and runtime)
- Optional tick rate for periodic updates
- Comprehensive documentation and README
