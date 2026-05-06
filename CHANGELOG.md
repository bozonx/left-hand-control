# Changelog

All notable changes to Left Hand Control are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Structured logging (`utils/logger.ts`) with timestamps and log levels across all composables
- Toast notifications for user-visible failures in layout delete and reset operations
- Coverage reporting for unit and component test suites
- `pnpm audit` and `cargo audit` steps in CI for dependency vulnerability scanning
- Dependabot configuration for automated dependency update PRs
- Device path validation in `start_mapper` Rust command (must be under `/dev/input/`)
- `eslint` lint step in CI pipeline

## [0.2.0] - 2025-01-01

### Added
- Emoji selector overlay with configurable hotkeys and multiple pages
- Quick menu overlay with keyboard shortcuts
- Game mode detection and layout auto-switching
- Layout library with import/export support
- Macro editor with system macros support
- Command runner with trust/approval model
- Auto-layout switching based on active window, system layout, and game mode

## [0.1.0] - 2024-09-01

### Added
- Initial release with keyboard remapping via evdev on Linux
- Basic keymap editor with layer support
- Settings UI built with Nuxt 4 and Tauri 2
