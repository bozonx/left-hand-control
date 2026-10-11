# Changelog

All notable changes to Left Hand Control are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- Slint is the only active desktop shell, backed by the UI-independent `lhc-core` crate.
- The Cargo workspace, CI and developer documentation cover the Rust application.
- Bundled layouts, layout parser fixtures and the application icon live with their owning crates.
- Retired application sources and historical comparison reports are archived outside the active build.
- Removed configuration and layout migrations and the unused raw-JSON game-mode API.
- Retired condition fields are rejected; manual activation and command permissions use explicit settings.
- System actions (`sys:`) are reduced to `switchDesktopN`, `switchLayoutN` and the new `windowToDesktopN`; the other KDE-only actions are removed in favour of standard keys, chords or `kde:` shortcuts (see `dev_docs/removed-system-actions.md`).

### Added

- `kde:<component>/<shortcut>` action and a "KDE shortcuts" picker category listing every global shortcut of the running KDE session.
