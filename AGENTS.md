# AGENTS.md

Guide for AI coding agents working in this repository. Read it before making changes. Day-to-day developer details (system packages, env vars, run modes, troubleshooting) live in [`docs/slint-dev-linux.md`](docs/slint-dev-linux.md); this file is the short, prescriptive version.

## Project at a glance

**Left Hand Control** is a desktop keyboard mapper: it intercepts a physical keyboard, applies layers, rules, macros and commands, and emits the result through a virtual device. It also shows two quick popups (Emoji, Quick actions) and lives in the system tray.

- **The product is the Slint shell** in `apps/slint-shell/` on top of the shared Rust crate `crates/lhc-core/`. All new work goes here.
- **The Tauri 2 + Nuxt 4 app is legacy** (`src-tauri/` and the Nuxt project at the repo root). It is being phased out. Do not develop, refactor or fix it unless the user explicitly asks — see [Legacy Tauri/Nuxt](#legacy-taurinuxt).
- Primary target: **Linux, KDE Plasma 6 on Wayland**. The Slint UI also compiles for Windows and macOS with minimal portable adapters; those platforms are not accepted yet.

Stack: Rust stable ≥ 1.85 (edition 2024), Cargo workspace, Slint **1.17.1** (pinned, `=1.17.1`) with winit 0.30, `ksni` tray, `evdev`/`uinput`, `zbus`, vendored `spell-framework` for layer-shell popups.

## Commands

Run everything from the repo root. Build artefacts go to the shared `target/`.

```sh
cargo run -p slint-shell                                          # popups as winit windows
SLINT_SHELL_POPUPS=auto cargo run -p slint-shell --features spell # popups as layer-shell (KDE/Sway/Hyprland)
SLINT_LIVE_PREVIEW=1 cargo run -p slint-shell --features slint/live-preview  # hot reload of .slint

target/debug/slint-shell show settings   # CLI client → running instance (show emoji|quick [page], hide,
                                         # toggle-mapper, preferences dark|light ru|en, ping, quit)
```

Windows start hidden; the app lives in the tray. Only one instance runs per `XDG_RUNTIME_DIR` (override with `SLINT_SHELL_SOCKET`).

Checks (same as CI plus the workspace-wide ones):

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo clippy --locked -p lhc-core -p slint-shell -p left-hand-control \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets              # build without Spell
```

Single test: `cargo test --locked -p slint-shell --lib <name>` or `cargo test --locked -p lhc-core <name>`.

UI scenarios (need a graphical session, use a temporary config, exit on their own):

```sh
cargo run --locked -p slint-shell --example editor -- --smoke   # key editor, RU/EN switching
cargo run --locked -p slint-shell --example interactions        # settings, save, navigation, theme, language
```

Page-specific examples: `action-picker`, `library`, `macros`, `menus`, `undo-redo`, `inline-edit`, `drag-scroll`, `notifications`. Benchmarks and diagnostic stands (`bench-*`, `probes` feature, `apps/slint-shell/scripts/`) are described in `apps/slint-shell/README.md`.

## Repository map

```
crates/lhc-core/src/          # UI-independent domain + platform code (no Slint, no Tauri)
├── profile/                  # config model: settings, layouts, actions, macros, menus, auto-switch
├── config_document.rs        # editable config.json: validation, external-change guard, reload
├── storage.rs                # StoragePaths::resolve(): dev vs release directories
├── events.rs                 # CoreEvent bus — the only core → shell channel
├── mapper/                   # engine/, linux/ (evdev+uinput), portal.rs, system.rs, runtime.rs
├── layout/ gamemode/ active_window/   # system watchers feeding rule conditions
└── platform/                 # OS / DE / session detection

apps/slint-shell/
├── src/lib.rs                # process roles: settings app / Spell worker / CLI client
├── src/app/                  # settings process: state, popups, mapper bridge, worker supervision
├── src/document.rs           # the single config write path
├── src/pages/                # one module per settings page, each bound to one Slint global
├── src/command.rs, ipc.rs    # typed commands and the plain-text IPC wire format
├── src/popup_model.rs        # popup data and keys, shared by winit and Spell
├── src/spell.rs              # layer-shell popup worker (feature `spell`, Linux only)
├── src/i18n.rs               # Msg ids for Locale.text()
├── src/platform/linux/       # evdev hotkeys, ksni tray, Wayland activation, input return
├── src/platform/portable/    # Windows/macOS: global-hotkey, tray-icon, native input
├── ui/*.slint                # app.slint entry; types.slint enums; i18n.slint messages; theme.slint
├── translations/ru/          # Russian PO catalog (source strings are English)
├── examples/                 # UI scenarios and benchmarks
└── vendor/                   # patched i-slint-core and spell-framework (see PATCHES.md in each)

docs/                         # slint-dev-linux.md, windows-testing.md, e2e-linux-kde.md
```

## Architecture

**Core vs shell.** `lhc-core` owns everything that is not presentation: config types and `ConfigDocument`, storage paths, platform detection, watchers, mapper lifecycle and engine, device I/O, portal text injection, validation. The Slint shell only displays state and calls core operations. If logic could be reused by another shell or tested without a UI, it belongs in `lhc-core`.

**Events.** The core never calls into a UI framework. It publishes `CoreEvent`s on `lhc_core::events::bus()`; the shell subscribes once at startup in `apps/slint-shell/src/app/mapper.rs` and posts them to the Slint event loop. Do not add shell-specific callbacks into the core.

**Config writes.** Pages change the configuration only through `Document::edit` (`src/document.rs`). It applies the edit, saves, pushes the result to a running mapper and notifies the other views via `Document::subscribe`. Never hold a `Document::read()` borrow across UI calls that may edit — it causes `RefCell` conflicts.

**Persistence.** Settings auto-save to `config.json`; layouts are written to the library only on an explicit save. External changes (e.g. from the legacy Tauri app) are detected and reloaded; a save over unread external changes is rejected. Paths come from `StoragePaths::resolve()`: debug builds use `<repo>/.dev-files/` (or `LHC_DEV_DIR`), release builds use `~/.config/dev.bozonx.left-hand-control/` and `~/.local/share/dev.bozonx.left-hand-control/`.

**Processes.** One binary, three roles: the settings process, the Spell popup worker (`--spell-worker`) and a CLI client that sends a command over IPC (Unix socket in a private `0700` dir on Linux/macOS, token-signed loopback TCP on Windows). The mapper holds a cross-process lock, so only one process can grab keyboards.

**Popups.** `SLINT_SHELL_POPUPS=winit` (default) shows borderless winit windows in-process; `auto` uses Spell layer-shell when the compositor supports `zwlr_layer_shell_v1` and falls back to winit otherwise; `spell` requires layer-shell. Popup data and key handling live in `popup_model.rs` so both paths behave identically.

**Action strings.** Bindable actions are plain strings: a key chord (`Ctrl+KeyC`), `macro:<id>`, `cmd:<id>`, `sys:<id>`, `app:<id>`, `text:<literal>`, `pause:<ms>` (macro steps only); `null` swallows the key, empty means native passthrough. Parse and build them through the `Action` type in `lhc_core::profile::actions`, not ad-hoc string handling.

### Where does a change go?

| Change | Location |
| --- | --- |
| Config field, validation, layout file format | `crates/lhc-core/src/profile/`, `config_document.rs` |
| Mapper behaviour, key handling, system actions | `crates/lhc-core/src/mapper/` |
| New OS/DE detection or watcher | `crates/lhc-core/src/platform/`, `layout/`, `gamemode/`, `active_window/` |
| Settings page logic | `apps/slint-shell/src/pages/<page>.rs` + `ui/<page>.slint` |
| Popup content or keys | `apps/slint-shell/src/popup_model.rs`, `ui/popup-*.slint` |
| Tray, global hotkeys, focus return | `apps/slint-shell/src/platform/{linux,portable}/` |
| New CLI/IPC command | `apps/slint-shell/src/command.rs` |
| User-visible text | `src/i18n.rs` + `ui/i18n.slint` + `translations/ru/LC_MESSAGES/slint-shell.po` |

## Conventions

**Rust**
- Platform-specific code stays behind `#[cfg(...)]` and target-specific dependencies; the shared UI and core logic are OS-independent.
- Dispatch on `platform::linux::detect()`, never on raw environment variables like `XDG_CURRENT_DESKTOP`.
- Return `Result<_, String>` / typed errors at module boundaries as neighbouring code does; no `unwrap()` on runtime paths (build scripts and tests are fine).
- Use typed values across boundaries: `command::Window` / `Popup` instead of strings, enums from `ui/types.slint` instead of numeric codes between Rust and Slint.
- Comments: match the surrounding code. Modules carry a short `//!` header; public items get `///` docs when the purpose is not obvious; inline comments explain *why*, not *what*. Do not add narrative comments to code you did not otherwise change.
- Keep diffs minimal and focused; no drive-by refactors.

**Slint UI**
- Each settings page is one Slint global bound by one module in `src/pages/`. Shared widgets live in `controls.slint`, colours and metrics in `theme.slint` (system, light, dark and E-ink themes must all work).
- All user-visible text goes through `@tr(...)`. Rust never formats UI text: it sends an `i18n::Msg` (`Message { id, arg, count }`) and `Locale.text()` in `ui/i18n.slint` translates it.
- Adding a message means updating `src/i18n.rs`, `ui/i18n.slint` and the Russian PO file together. The test `every_slint_string_has_a_russian_translation` enforces coverage; Russian plural forms must be filled.
- Changing a property or callback used from Rust requires a rebuild; Live Preview only reloads pure `.slint` changes.

**Vendored crates**
- `vendor/i-slint-core` (via `[patch.crates-io]`, text-field undo/redo and shortcut fixes) and `vendor/spell-framework` are patched copies. Any change there must be recorded in that directory's `PATCHES.md`. When upgrading Slint, port or drop the patch and keep the `=1.17.1` pins (`slint`, `i-slint-core`, `slint-build`) in sync.

## Platform support

| Concern | KDE | GNOME | Sway | X11 generic | Windows | macOS |
| --- | --- | --- | --- | --- | --- | --- |
| Key interception (evdev+uinput) | ✅ | ✅ | ✅ | ✅ | ❌ stub | ❌ stub |
| Literal text (xdg-desktop-portal) | ✅ | ✅ | ✅ | ⚠ if portal present | ❌ | ❌ |
| Layout detection | ✅ DBus `org.kde.keyboard` | 🚧 | 🚧 | 🚧 | 🚧 | 🚧 |
| System actions (`switchDesktopN`) | ✅ KWin DBus | 🚧 | 🚧 | 🚧 | ❌ | ❌ |
| Layer-shell popups (Spell) | ✅ | ❌ winit fallback | ✅ not accepted | ❌ winit | — | — |

🚧 = skeleton module returning `Ok(None)`/`None` with a planned-implementation note; ❌ stub = explicit "not implemented" error. Both compile and are safe to ship.

**Adding a Linux DE backend:** add a variant to `platform::linux::Desktop` and `classify_desktop()`; create `layout/linux_<de>.rs` with `current()` and `start_watcher()` that report through `super::publish(&info)`; wire it into `layout/mod.rs`; add `mod <de>` with `resolve(name) -> Option<SysAction>` in `mapper/system.rs` and wire it into the dispatcher.

**Adding Windows/macOS interception:** replace the non-Linux stubs in `mapper/runtime.rs` with `mapper/windows.rs` / `mapper/macos.rs` mirroring `mapper::linux` (`list_keyboards()`, `spawn()`, `Handle`). The engine currently uses `evdev::Key`; introduce a generic key type first. Windows: `SetWindowsHookExW(WH_KEYBOARD_LL)` + `SendInput`. macOS: `CGEventTapCreate` + `CGEventPost` (requires Accessibility permission). Test Windows in a VM as described in `docs/windows-testing.md`.

**Linux runtime requirements:** read access to `/dev/input/event*` and rw to `/dev/uinput` (group `input` + udev rule), `xdg-desktop-portal` with the DE backend for literal text, `kdotool` for active-window conditions on KDE Wayland. Setup commands are in `docs/slint-dev-linux.md`.

## Definition of done

1. `cargo fmt --check`, clippy with `-D warnings` and the workspace tests pass (commands above).
2. `cargo check --locked -p slint-shell --all-targets` passes (non-Spell build).
3. For UI changes: `editor -- --smoke` and `interactions` pass in a graphical session, and the relevant page example if one exists.
4. For new user-visible strings: Russian translation added and the translation test passes.
5. For core changes that affect the public API: the legacy shell still compiles (it is part of the workspace, so the clippy/test commands above cover it).
6. For platform-dependent behaviour: state which environment you verified (e.g. KDE Wayland) and what remains untested.

Report the exact commands you ran and their results. If something could not be run (no graphical session, no `/dev/uinput` access), say so.

## Boundaries

**Always**
- Put domain and platform logic in `lhc-core`; keep the shell thin.
- Edit config through `Document::edit`; resolve paths through `StoragePaths::resolve()`.
- Keep `config.json` and layout YAML backward-compatible: users' existing files must still load.

**Ask first**
- Adding a dependency or bumping Slint / `spell-framework` / `i-slint-core`.
- Changing the on-disk config or layout format, the IPC wire format or the CLI commands.
- Touching anything under `src-tauri/` or the Nuxt sources.

**Never**
- Read or write the real user config from debug builds or tests (use `.dev-files/`, `LHC_DEV_DIR` or a temp dir).
- Hardcode absolute paths, call into Slint from `lhc-core`, or add UI text outside `@tr`.
- Commit `target/`, `.dev-files/`, `.nuxt/`, `.output/` or `node_modules/`.
- Enable `slint/live-preview` in regular or release builds.

## Legacy Tauri/Nuxt

`src-tauri/` (crate `left-hand-control`) and the Nuxt 4 SPA at the repo root (`pages/`, `components/`, `composables/`, `utils/`, `i18n/`, `package.json`, `pnpm` tooling) are frozen. They still build and share `config.json` and the dev directory with the Slint shell, and `src-tauri` is still a workspace member over `lhc-core`.

- Do not add features, refactor or port fixes there unless explicitly asked; the Slint shell and `lhc-core` are the reference.
- Do not use the Tauri app or its TypeScript as a specification when it disagrees with the Slint shell.
- If a core change breaks compilation of `src-tauri`, make the minimal adaptation needed to keep the workspace green and mention it.
- `scripts/check-system-macros-sync.mjs` checks that `utils/systemMacros.ts` matches `crates/lhc-core/src/mapper/system_macros.rs`; if you change system macros in the core, either update the TS list or tell the user the check will fail.
- `CLAUDE.md` still describes the Tauri/Nuxt frontend in detail; when it disagrees with this file, this file wins.

## Common pitfalls

- **`slint-shell is already running`** — another instance owns the socket: `target/debug/slint-shell quit` or set `SLINT_SHELL_SOCKET`.
- **First build is slow or fails in Skia** — Skia is built from source; install clang, cmake, ninja and Python.
- **`No readable Ctrl+Alt+F11/F12 evdev devices`** — no access to `/dev/input`; fix permissions or run with `SLINT_SHELL_HOTKEYS=off`.
- **Monochrome or missing emoji** — install Noto Color Emoji; in winit windows only `SLINT_BACKEND=winit-skia` draws colour emoji.
- **`compositor does not advertise zwlr_layer_shell_v1`** — GNOME has no layer-shell; use `SLINT_SHELL_POPUPS=auto` or `winit`.
- **Save rejected: "configuration changed by another application"** — the file changed externally; the document is already reloaded, repeat the edit.
- **`RefCell` already borrowed panic in a page** — a `Document::read()` borrow is alive while a callback edits; drop it before calling into the UI.
- **CLI replies `queued`** — the command is queued on the UI thread; the window is not necessarily ready for input yet.

## Further reading

- [`docs/slint-dev-linux.md`](docs/slint-dev-linux.md) — setup, run modes, env vars, feature behaviour, troubleshooting.
- [`apps/slint-shell/README.md`](apps/slint-shell/README.md) — popup invocation paths, benchmarks, diagnostic stands, Windows/macOS notes.
- [`docs/windows-testing.md`](docs/windows-testing.md) — Windows VM test environment.
- `apps/slint-shell/vendor/*/PATCHES.md` — what the vendored crates change and why.
