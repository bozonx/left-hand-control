# AGENTS.md

Guide for AI coding agents working in this repository. Read it before making changes. Day-to-day developer details (system packages, env vars, run modes, checks, e2e, releases) live in [`docs/development/guide.md`](docs/development/guide.md); this file is the short, prescriptive version.

## Project at a glance

**Left Hand Control** is a desktop keyboard mapper: it intercepts a physical keyboard, applies layers, rules, macros and commands, and emits the result through a virtual device. It also shows two quick popups (Emoji, Quick actions) and lives in the system tray.

- **The product is the Slint shell** in `apps/slint-shell/` on top of the shared Rust crate `crates/lhc-core/`. All new work goes here.
- Primary target: **Linux, KDE Plasma 6 on Wayland**. Windows has a mapper backend (session-wide low-level hooks + `SendInput`); macOS has only the shell. Neither is accepted yet.

Stack: Rust stable ≥ 1.92 (edition 2024), Cargo workspace, Slint **1.18.1** (pinned, `=1.18.1`) with winit 0.30, `ksni` tray, `evdev`/`uinput`, `zbus`, vendored `spell-framework` for layer-shell popups.

## Commands

Run everything from the repo root. Build artefacts go to the shared `target/`.

```sh
cargo run -p slint-shell                                          # popups as winit windows
LHC_POPUPS=auto cargo run -p slint-shell --features spell        # popups as layer-shell (KDE/Sway/Hyprland)
SLINT_LIVE_PREVIEW=1 cargo run -p slint-shell --features slint/live-preview  # hot reload of .slint

target/debug/left-hand-control show settings   # CLI client → running instance (show emoji|quick [page], hide,
                                         # toggle-mapper, game auto|on|off|toggle,
                                         # preferences dark|light ru|en, ping, quit;
                                         # --version and --help need no instance)
```

Windows start hidden; the app lives in the tray. Only one instance runs per `XDG_RUNTIME_DIR` (override with `LHC_SOCKET`); a second launch without arguments opens the running instance's settings.

Checks (same as CI plus the workspace-wide ones):

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo clippy --locked -p lhc-core -p slint-shell \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets              # build without Spell
cargo deny check                                               # advisories, licences, sources (deny.toml)
```

Single test: `cargo test --locked -p slint-shell --lib <name>` or `cargo test --locked -p lhc-core <name>`.

**Memory.** A full build is heavy (the generated Slint UI is ~28 MB of Rust; `cargo test --workspace` also builds and links every example), and several agents often share this machine and its `target/`. Therefore:
- While iterating, run only the affected tests: a single test as above, or `cargo test --locked -p <crate> --lib --tests` (skips building examples).
- Run the full workspace tests, clippy `--all-targets` and UI scenarios once, at the end of the task, not after every edit.
- Do not start UI scenarios or e2e while another build or test run is in progress; cargo serialises builds in the shared `target/`, but test and example processes run concurrently.
- Do not raise build parallelism; `CARGO_BUILD_JOBS` / `RUST_TEST_THREADS` may be limited by the environment on purpose.

UI scenarios (need a graphical session, use a temporary config, exit on their own):

```sh
cargo run --locked -p slint-shell --example editor -- --smoke   # key editor, RU/EN switching
cargo run --locked -p slint-shell --example interactions        # settings, save, navigation, theme, language
```

Page-specific examples: `action-picker`, `library`, `macros`, `menus`, `undo-redo`, `inline-edit`, `drag-scroll`, `notifications`. Benchmarks and diagnostic stands (`bench-*`, `probes` feature, `apps/slint-shell/scripts/`) are described in `apps/slint-shell/README.md`.

Mapper, e2e and release checks (desktop e2e and VM runs are local only; CI runs just the mapper e2e, when the runner has `/dev/uinput`):

```sh
cargo test --locked -p lhc-core --test mapper_e2e -- --ignored  # real evdev → engine → uinput path; needs /dev/uinput and /dev/input
apps/slint-shell/scripts/e2e-local.sh                           # all UI scenarios + mapper e2e; writes docs/testing/results/linux/<date>.txt
apps/slint-shell/scripts/release/release-linux.sh               # tar.gz, deb, rpm, vendor archive, SHA256SUMS (needs nfpm)
apps/slint-shell/scripts/vm/run-linux.py --vm <dir> --package <file>  # L4: package in a disposable KDE VM (mapper, text:, popups, lock, suspend, replug, upgrade)
apps/slint-shell/scripts/vm/run-windows.py --vm <dir>                  # L4: Windows shell acceptance in a disposable VM
```

Releases: `cargo release <minor|patch>` previews and `--execute` bumps the version, dates the `CHANGELOG.md` section and metainfo entry, commits, tags `v<version>` and pushes; the tag makes `.github/workflows/release.yml` build a draft release. See `docs/development/guide.md` (section 5).

## Repository map

```
crates/lhc-core/assets/       # bundled layouts (<name>.json, templates for new library layouts)
crates/lhc-core/src/          # UI-independent domain + platform code (no UI framework)
├── profile/                  # config model: settings, layouts, actions, macros, menus, auto-switch
├── config_document/          # editable config.json: validation, saving, active layout
├── storage.rs                # StoragePaths::resolve(): dev vs release directories
├── events.rs                 # CoreEvent bus — the only core → shell channel
├── mapper/                   # engine/, output.rs (shared loop), linux/ (evdev+uinput), windows/ (hooks+SendInput), system.rs, runtime.rs
├── text_input/               # text: injection: portal/ (RemoteDesktop, libei), clipboard, job accounting
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
├── assets/                   # application icon
├── ui/*.slint                # app.slint entry; types.slint enums; i18n.slint messages; theme.slint
├── translations/ru/          # Russian PO catalog (source strings are English)
├── examples/                 # UI scenarios and benchmarks
├── scripts/                  # check/, release/, windows/, bench/, vm/ (L4), experimental/
└── vendor/                   # patched i-slint-core and spell-framework (see PATCHES.md in each)

docs/                         # development/, design/, testing/; README.md index
```

## Architecture

**Core vs shell.** `lhc-core` owns everything that is not presentation: config types and `ConfigDocument`, storage paths, platform detection, watchers, mapper lifecycle and engine, device I/O, portal text injection, validation. The Slint shell only displays state and calls core operations. If logic could be reused by another shell or tested without a UI, it belongs in `lhc-core`.

**Events.** The core never calls into a UI framework. It publishes `CoreEvent`s on `lhc_core::events::bus()`; the shell subscribes once at startup in `apps/slint-shell/src/app/mapper.rs` and posts them to the Slint event loop. Do not add shell-specific callbacks into the core.

**Config writes.** Pages change the configuration only through `Document::edit` (`src/document.rs`). It applies the edit, saves, pushes the result to a running mapper and notifies the other views via `Document::subscribe`. Never hold a `Document::read()` borrow across UI calls that may edit — it causes `RefCell` conflicts.

**Persistence.** Settings auto-save to `config.json`; layouts are written to the library only on an explicit save. Both formats are versioned independently (`CONFIG_SCHEMA_VERSION`, `LAYOUT_SCHEMA_VERSION` in `profile/schema.rs`, both 1) and parsed strictly (unknown fields and values are errors); see [`docs/config-format.md`](docs/config-format.md). Within a version only optional fields and enum values may be added; anything else bumps that version and adds a reader arm for the old one (procedure in the `schema.rs` header); storage backs up an older-version file as `<file>.v<N>.bak` before overwriting it. Configuration is read at startup and edited only through the application; manual editing is unsupported and at the user's own risk. Paths come from `StoragePaths::resolve()`: an explicit `LHC_DEV_DIR` overrides paths in debug and release builds; otherwise debug builds use `<repo>/dev-files/<os>/`, and release builds use `~/.config/dev.bozonx.left-hand-control/` and `~/.local/share/dev.bozonx.left-hand-control/`.

**Processes.** One binary, three roles: the settings process, the Spell popup worker (`--spell-worker`) and a CLI client that sends a command over IPC (Unix socket in a private `0700` dir on Linux/macOS, token-authenticated loopback TCP on Windows). The mapper holds a cross-process lock, so only one process can grab keyboards.

**Popups.** `LHC_POPUPS=winit` (default) shows borderless winit windows in-process; `auto` uses Spell layer-shell when the compositor supports `zwlr_layer_shell_v1` and falls back to winit otherwise; `spell` requires layer-shell. Popup data and key handling live in `popup_model.rs` so both paths behave identically.

**Action strings.** Bindable actions are plain strings: a key chord (`Ctrl+KeyC`), `macro:<id>`, `cmd:<id>`, `sys:<id>` (`switchDesktopN`, `switchLayoutN`, `windowToDesktopN` and cross-platform window/shell actions such as `windowClose`, `showDesktop`, `screenOff`; see `PLAIN_SYSTEM_ACTIONS`), `app:<id>`, `kde:<component>/<shortcut>` (any KDE global shortcut via kglobalaccel), `text:<literal>`, `pause:<ms>` (macro steps only); `null` swallows the key, empty means native passthrough. Parse and build them through the `Action` type in `lhc_core::profile::actions`, not ad-hoc string handling.

### Where does a change go?

| Change | Location |
| --- | --- |
| Config field, validation, layout file format | `crates/lhc-core/src/profile/`, `config_document/` |
| Mapper behaviour, key handling, system actions | `crates/lhc-core/src/mapper/` |
| Text injection (`text:`, popup insertion) | `crates/lhc-core/src/text_input/` |
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
- Each settings page is one Slint global bound by one module in `src/pages/`. Shared widgets are exported by `controls.slint` (base widgets in `controls-base.slint`, inline editors in `controls-inline.slint`), colours and metrics in `theme.slint` (system, light, dark and E-ink themes must all work).
- All user-visible text goes through `@tr(...)`. Rust never formats UI text: it sends an `i18n::Msg` (`Message { id, arg, count }`) and `Locale.text()` in `ui/i18n.slint` translates it.
- Adding a message means updating `src/i18n.rs`, `ui/i18n.slint` and the Russian PO file together. The build checks catalog completeness, contexts, plural forms, placeholders and Rust message handlers. Text outside Slint is registered in `build-support/rust_messages.rs`; its translations are generated from the PO file without runtime fallbacks. Technical error details stay in logs.
- Changing a property or callback used from Rust requires a rebuild; Live Preview only reloads pure `.slint` changes.

**Vendored crates**
- `spell-framework` is an explicit workspace member: workspace tests cover it. `i-slint-core` is a patched dependency, excluded from workspace membership.
- `vendor/i-slint-core` (via `[patch.crates-io]`, text-field undo/redo and shortcut fixes) and `vendor/spell-framework` are patched copies. Any change there must be recorded in that directory's `PATCHES.md`. When upgrading Slint, port or drop the patch and keep the `=1.18.1` pins (`slint`, `i-slint-core`, `slint-build`) in sync.

## Platform support

The primary accepted environment is KDE Plasma 6 on Wayland. Windows interception
(`mapper/windows/`) is implemented but not accepted; macOS interception is not
implemented. X11 sessions are not a supported target: the existing X11 code is best effort,
without acceptance or e2e, and is not extended. Actions a platform cannot run are
skipped with a notice.
See [platform support and backend development](docs/development/platforms.md)
for the capability matrix and backend extension instructions.

## Definition of done

1. `cargo fmt --check`, clippy with `-D warnings` and the workspace tests pass (commands above).
2. `cargo check --locked -p slint-shell --all-targets` passes (non-Spell build).
3. For UI changes: `editor -- --smoke` and `interactions` pass in a graphical session, and the relevant page example if one exists.
4. For new user-visible strings: Russian translation added and the translation test passes.
5. For user-visible behaviour changes: an entry under `## [Unreleased]` in `CHANGELOG.md` (never a version or date; cargo-release adds them).
6. Core public API changes must compile with the Slint shell; workspace checks cover both crates.
7. For platform-dependent behaviour: state which environment you verified (e.g. KDE Wayland) and what remains untested.

Report the exact commands you ran and their results. If something could not be run (no graphical session, no `/dev/uinput` access), say so.

## Boundaries

**Always**
- Put domain and platform logic in `lhc-core`; keep the shell thin.
- Edit config through `Document::edit`; resolve paths through `StoragePaths::resolve()`.
- Support the current `config.json` and layout JSON formats, plus older `schemaVersion`s through the readers in `profile/`; do not add fallbacks for formats that predate `schemaVersion`.

**Ask first**
- Adding a dependency or bumping Slint / `spell-framework` / `i-slint-core`.
- Changing the on-disk config or layout format, the IPC wire format or the CLI commands.
- Running `cargo release --execute` or pushing `v*` tags (starts a public release).
- Modifying the historical archive under `_old/`.

**Never**
- Read or write the real user config from debug builds or tests (use `dev-files/`, `LHC_DEV_DIR` or a temp dir).
- Hardcode absolute paths, call into Slint from `lhc-core`, or add UI text outside `@tr`.
- Commit build artifacts, development config/data, or dependency caches.
- Enable `slint/live-preview` in regular or release builds.

## Historical archive

`_old/` is an inactive archive, excluded from the Cargo workspace and CI.
Do not use its code or instructions as the product specification, add dependencies
on it, or try to keep it building. Work only in `lhc-core` and `apps/slint-shell`.

## Further reading

- [`docs/development/guide.md`](docs/development/guide.md) — setup, run modes, env vars, checks, e2e levels (host and VMs), release.
- [`docs/features.md`](docs/features.md) — how the application's features behave.
- [`apps/slint-shell/README.md`](apps/slint-shell/README.md) — popup invocation paths, benchmarks, diagnostic stands, Windows/macOS notes.
- [`docs/testing/windows.md`](docs/testing/windows.md) — Windows VM installation; [`docs/testing/manual-acceptance.md`](docs/testing/manual-acceptance.md) — manual acceptance checklist.
- `apps/slint-shell/vendor/*/PATCHES.md` — what the vendored crates change and why.
