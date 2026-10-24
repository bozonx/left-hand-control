# Changelog

All notable changes to Left Hand Control are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
New entries go under `[Unreleased]`; `cargo release` turns it into a dated version section.

## [Unreleased]

## [0.4.0] - 2026-10-24

### Added

- Windows x64 portable ZIP packaging with SHA256 checks, release artifacts and lifecycle acceptance of the extracted executable.
- Cross-platform system actions `sys:windowClose`, `windowMinimize`, `windowMaximize`, `windowKeepAbove`, `showDesktop`, `overview`, `appLauncher` and `screenOff` (KDE implementation through kglobalaccel). On macOS, `sys:` actions without a public API there are ignored with a notification.
- The KDE shortcuts category of the action picker explains where its list comes from.

### Changed

- The bundled layout uses the new system actions instead of the matching `kde:` shortcuts.

### Fixed

- Linux popup hotkeys on the grabbed keyboard are consumed before uinput output, preventing Ctrl+Alt+F11/F12 from switching virtual consoles while the mapper runs.
- Linux mapper reconnects the selected keyboard after ENODEV, preserves the latest configuration and refuses a different device reusing the selected path.
- KWin scripts are scoped to each application's D-Bus connection, so a second instance cannot replace or unload the running instance's focus watcher.

## [0.3.0] - 2026-10-17

### Changed

- The executable is named `left-hand-control` (the Cargo package stays `slint-shell`); on Linux its windows use the app id `dev.bozonx.left-hand-control`.
- Slint is the only active desktop shell, backed by the UI-independent `lhc-core` crate.
- The Cargo workspace, CI and developer documentation cover the Rust application.
- Bundled layouts, layout parser fixtures and the application icon live with their owning crates.
- Retired application sources and historical comparison reports are archived outside the active build.
- Removed configuration and layout migrations and the unused raw-JSON game-mode API.
- Retired condition fields are rejected; manual activation and command permissions use explicit settings.
- System actions (`sys:`) are reduced to `switchDesktopN`, `switchLayoutN` and the new `windowToDesktopN`; the other KDE-only actions are removed in favour of standard keys, chords or `kde:` shortcuts (see `docs/design/removed-system-actions.md`).
- Environment variables use the `LHC_` prefix (`LHC_POPUPS`, `LHC_SOCKET`, `LHC_HOTKEYS`, …) instead of `SLINT_SHELL_`.
- Launching the application again (for example from the application menu) opens the settings window of the running instance instead of failing.

### Added

- Linux packaging metadata: application icon in all hicolor sizes and SVG, desktop entry, AppStream metainfo, a udev rule giving the active session and the `input` group access to `/dev/uinput`, and `uinput` autoload. `install-linux.sh permissions` / `remove-permissions` install and remove them explicitly.
- Layer hint (Settings → General, off by default): while a layer is held, its keys appear in a layer-shell overlay above all windows that takes neither the keyboard focus nor the pointer; never shown in game mode.
- `kde:<component>/<shortcut>` action and a "KDE shortcuts" picker category listing every global shortcut of the running KDE session.
- `--version` and `--help` command-line options.
- Linux release artifacts built by a tag-triggered workflow: `.tar.gz`, `.deb`, `.rpm`, vendored crate sources and `SHA256SUMS` with build provenance attestations.
