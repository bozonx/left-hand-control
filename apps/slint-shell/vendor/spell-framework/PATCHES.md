# Local patches

Source: https://github.com/VimYoung/Spell/tree/b37604eb46edfec5861720915db87e646b645bd1/spell-framework
Version: 1.0.6 (unreleased Git revision with Slint 1.18 support). Original license is included in LICENSE.

This copy is linked into the Slint shell (`--features spell`); the project is
licensed GPL-3.0-or-later, compatible with this crate's license.

- Commit unmap immediately and redraw after remapping without waiting for a stopped frame callback.
- Bound pending frame callbacks, avoid hidden-surface commits, and recreate compositor-closed layers after output removal.
- Deliver focus, committed rendered frame and layer closure through per-window callbacks.
- Deliver keyboard repeats and synchronize modifiers, including modifiers held before focus; release pressed keys on focus loss.
- Resolve output objects on each window's own Wayland connection (replaces the upstream global `AVAILABLE_MONITORS` lookup in `SpellWin::create_window`).
- Calculate physical sizes from original logical sizes on every scale change; preserve logical layer geometry.

- `on_call` dispatches without waiting; the application blocks on the event-loop file descriptors itself instead of spinning every millisecond.

- Trimmed to the layer-shell windows the application uses: removed `vault` (MPRIS, desktop entries, notification server), the session lock (`SpellLock`, PAM), the `cast_spell`/`generate_widgets` macros and their docs, and the dependencies only they needed (`mpris`/libdbus, `nonstick`/libpam, `tokio`, `zbus`, `rust-fuzzy-search`, `paste`) plus the unused `event-listener` and `futures-util`.
- Removed the built-in logging setup (`tracing-subscriber`, `tracing-appender`): no global subscriber, no hourly log files and CLI socket in `$XDG_RUNTIME_DIR/spell/`, no 2-second polling of `spell_cli`. `tracing` uses its `log` feature, so events reach the application's logger. Clipboard reads no longer log the pasted text.
- Slint crates are pinned to `=1.18.1` like the application.

- Build integration: `[lib] doctest = false`, because upstream doctests do not build from this vendored copy inside the application workspace.
- Build integration: `[package.metadata.release] release = false`, so the application's cargo-release run does not bump or tag this crate.

Frame events mean rendering and buffer commit, not compositor presentation.

- Normalize text editing shortcuts with the patched Slint core, preserving Latin layouts and using physical keys for non-Latin layouts; preserve AltGr input and normalized press/release/repeat events.

- Optional compositor-sized horizontal layers: request width zero with opposite anchors and resize the Slint buffer/viewport on configure, including later output width changes. Used by the responsive held-layer hint.

- Physical popup shortcut callbacks are skipped while Ctrl, Alt or Logo is held; modified input continues through Slint with its modifiers.
