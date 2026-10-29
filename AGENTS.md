# AGENTS.md

Guide for AI coding agents in this repository. Read it before making changes. This is the short, prescriptive version; setup, env vars, run modes, e2e levels and releases are in [`docs/development/guide.md`](docs/development/guide.md).

## Project

**Left Hand Control** is a desktop keyboard mapper: it intercepts a physical keyboard, applies layers, rules, macros and commands, and emits the result through a virtual device. It also shows quick popups (Emoji, Quick actions) and lives in the system tray.

- **The product is the Slint shell** (`apps/slint-shell/`) on top of the shared crate `crates/lhc-core/`. All work goes here; `_old/` is an inactive archive (see [Boundaries](#boundaries)).
- Primary target: **Linux, KDE Plasma 6 on Wayland**. GNOME Wayland works through a bundled Shell extension; Windows has a mapper backend; macOS has only the shell. Only KDE is accepted. Capability matrix: [`docs/development/platforms.md`](docs/development/platforms.md).

Stack: Rust edition 2024 (MSRV 1.92 in `Cargo.toml`, CI toolchain pinned in `rust-toolchain.toml`), Cargo workspace, Slint **=1.18.1** with winit 0.30, `ksni` tray, `evdev`/`uinput`, `zbus`, vendored `spell-framework` for layer-shell popups.

## Commands

Run from the repo root; artefacts go to the shared `target/`.

```sh
cargo run -p slint-shell                                          # popups as winit windows
LHC_POPUPS=auto cargo run -p slint-shell --features spell        # layer-shell popups (KDE/Sway/Hyprland)
SLINT_LIVE_PREVIEW=1 cargo run -p slint-shell --features slint/live-preview  # hot reload of .slint
target/debug/left-hand-control --help                             # CLI client; full syntax in command.rs (USAGE)
```

Windows start hidden (the app lives in the tray). One instance per `XDG_RUNTIME_DIR` (override with `LHC_SOCKET`); a second launch without arguments opens the running instance's settings. CLI commands (`show`, `hide`, `toggle-mapper`, `game`, `preferences`, `execute <action>`, `ping`, `quit`) go to the running instance; `--version`/`--help` need none.

### Checks (CI + workspace)

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo clippy --locked -p lhc-core -p slint-shell \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets              # build without Spell
cargo deny check                                               # advisories, licences, sources
```

Single test: `cargo test --locked -p slint-shell --lib <name>` or `cargo test --locked -p lhc-core <name>`.

### Resource limits

Builds are heavy (generated Slint UI is ~28 MB of Rust; `cargo test --workspace` also links every example), and several agents may share this machine and `target/`:
- While iterating, run only affected tests: a single test, or `cargo test --locked -p <crate> --lib --tests` (skips examples).
- Run full workspace tests, clippy `--all-targets` and UI scenarios once, at the end.
- Do not start UI scenarios or e2e while another build or test run is in progress.
- Do not raise build parallelism; `CARGO_BUILD_JOBS` / `RUST_TEST_THREADS` may be limited on purpose.

### UI scenarios

Need a graphical session, use a temporary config and exit on their own:

```sh
cargo run --locked -p slint-shell --example editor -- --smoke   # key editor, language switching
cargo run --locked -p slint-shell --example interactions        # settings, save, navigation, theme, language
```

Page examples: `action-picker`, `conditions`, `library`, `macros`, `menus`, `undo-redo`, `inline-edit`, `drag-scroll`, `notifications`, `layer-keys`. Benchmarks and diagnostic stands (`bench-*`, `probes` feature, `scripts/bench`, `scripts/check`) are described in `apps/slint-shell/README.md`.

### Mapper e2e and releases

Desktop e2e and VM runs are local only; CI runs just the mapper e2e when the runner has `/dev/uinput`.

```sh
cargo test --locked -p lhc-core --test mapper_e2e -- --ignored  # real evdev → engine → uinput; needs /dev/uinput, /dev/input
apps/slint-shell/scripts/e2e-local.sh                           # all UI scenarios + mapper e2e → docs/testing/results/linux/<date>.txt
apps/slint-shell/scripts/release/release-linux.sh               # tar.gz, deb, rpm, vendor archive, SHA256SUMS (needs nfpm)
apps/slint-shell/scripts/vm/run-linux.py --vm <dir> --package <file>  # L4: package in a disposable KDE VM
apps/slint-shell/scripts/vm/run-gnome.py                        # L4: GNOME VM (see docs/testing/gnome.md)
apps/slint-shell/scripts/vm/run-windows.py --vm <dir>           # L4: Windows shell acceptance
```

Releases: `cargo release <minor|patch>` previews; `--execute` bumps the version, dates `CHANGELOG.md` and metainfo, commits, tags `v<version>` and pushes, which triggers a draft release in `.github/workflows/release.yml`. Details: guide, section 5.

## Repository map

```
crates/lhc-core/
├── assets/                   # bundled layout(s) (templates for new library layouts), GNOME Shell extension
├── tests/                    # integration tests incl. mapper_e2e
└── src/                      # UI-independent domain + platform code (no UI framework)
    ├── profile/              # config model: settings, layouts, actions, macros, menus, auto-switch; schema.rs
    ├── config_document/      # editable config.json: validation, saving, active layout
    ├── storage.rs            # StoragePaths::resolve(): dev vs release directories
    ├── events.rs             # CoreEvent bus — the only core → shell channel
    ├── mapper/               # engine/, output.rs (shared loop), linux/ (evdev+uinput), windows/ (hooks+SendInput), system.rs, runtime.rs
    ├── text_input/           # text: injection: portal (RemoteDesktop, libei), clipboard, job accounting
    ├── keyboard_layout/ gamemode/ active_window/   # system watchers feeding rule conditions
    └── platform/             # OS / DE / session detection, KDE and GNOME integrations

apps/slint-shell/
├── src/lib.rs                # process roles: settings app / Spell worker / CLI client
├── src/app/                  # settings process: popups, mapper bridge, worker supervision, layer hint
├── src/document.rs           # the single config write path
├── src/pages/                # one module per settings page, each bound to one Slint global
├── src/command.rs, ipc.rs    # typed commands and the plain-text IPC wire format
├── src/popup_model.rs        # popup data and keys, shared by winit and Spell
├── src/spell.rs              # layer-shell popup worker (feature `spell`, Linux only)
├── src/i18n.rs               # Msg ids for Locale.text(), plural rules
├── src/env.rs                # names of LHC_* environment variables
├── src/platform/linux/       # evdev hotkeys, ksni tray, Wayland activation, input return
├── src/platform/portable/    # Windows/macOS: global-hotkey, tray-icon, native input
├── build-support/            # build-time i18n checks; rust_messages.rs registers text outside Slint
├── ui/*.slint                # app.slint entry; types.slint enums; i18n.slint messages; theme.slint
├── translations/<lang>/      # PO catalogs (source strings are English); see translations/README.md
├── examples/                 # UI scenarios and benchmarks
├── packaging/                # Linux: nfpm, udev rule, desktop file, metainfo
├── scripts/                  # bench/, check/, release/, vm/ (L4), windows/, experimental/
└── vendor/                   # patched i-slint-core and spell-framework (see PATCHES.md in each)

docs/                         # development/, design/, testing/, config-format.md, features.md
```

## Architecture

- **Core vs shell.** `lhc-core` owns everything except presentation: config types and `ConfigDocument`, storage paths, platform detection, watchers, mapper lifecycle and engine, device I/O, text injection, validation. The shell displays state and calls core operations. If logic could be reused by another shell or tested without a UI, it belongs in `lhc-core`.
- **Events.** The core never calls a UI framework. It publishes `CoreEvent`s on `lhc_core::events::bus()`; the shell subscribes once in `apps/slint-shell/src/app/mapper.rs` and posts them to the Slint event loop. Do not add shell-specific callbacks to the core.
- **Config writes.** Pages change configuration only through `Document` (`src/document.rs`): `edit` stages settings/layout transactions, `edit_library` applies one core library operation with its own file rollback, and `sync_autostart` refreshes system registration. They share the runtime synchronization and view notification path. See [`docs/development/api.md`](docs/development/api.md). Never hold a `Document::read()` borrow across UI calls that may edit (`RefCell` conflicts).
- **Persistence.** Settings auto-save to `config.json`; layouts go to the library only on explicit save. Both are versioned independently (`CONFIG_SCHEMA_VERSION`, `LAYOUT_SCHEMA_VERSION` in `profile/schema.rs`) and parsed strictly (unknown fields/values are errors); see [`docs/config-format.md`](docs/config-format.md). Within a version only optional fields and enum values may be added; anything else bumps the version and adds a reader arm for the old one (procedure in the `schema.rs` header). Storage backs up an older-version file as `<file>.v<N>.bak` before overwriting. Manual config editing is unsupported.
- **Paths.** From `StoragePaths::resolve()`: `LHC_DEV_DIR` overrides in any build; otherwise debug builds use `<repo>/dev-files/<os>/`, release builds use `~/.config/dev.bozonx.left-hand-control/` and `~/.local/share/dev.bozonx.left-hand-control/`.
- **Processes.** One binary, three roles: settings process, Spell popup worker (`--spell-worker`), CLI client sending a command over IPC (Unix socket in a private `0700` dir on Linux/macOS, token-authenticated loopback TCP on Windows). The mapper holds a cross-process lock, so only one process grabs keyboards.
- **Popups.** `LHC_POPUPS=winit` (default) shows borderless in-process winit windows; `auto` uses Spell layer-shell when the compositor has `zwlr_layer_shell_v1`, else winit; `spell` requires layer-shell. Data and key handling live in `popup_model.rs` so both paths behave identically.
- **Action strings.** Bindable actions are plain strings: a key chord (`Ctrl+KeyC`), `macro:<id>`, `builtin:<id>` (built-in macros, see `mapper/builtin_macros.rs`), `cmd:<id>`, `sys:<id>` (see `PLAIN_SYSTEM_ACTIONS` and the `switchDesktopN`/`switchLayoutN`/`windowToDesktopN` families), `app:<id>`, `kde:<component>/<shortcut>`, `text:<literal>`, `pause:<ms>` (macro steps only); `null` swallows the key, empty means native passthrough. Parse and build them only through `Action` in `lhc_core::profile::actions`.

### Where does a change go?

| Change | Location |
| --- | --- |
| Config field, validation, layout file format | `crates/lhc-core/src/profile/`, `config_document/` |
| Mapper behaviour, key handling, system actions | `crates/lhc-core/src/mapper/` |
| Text injection (`text:`, popup insertion) | `crates/lhc-core/src/text_input/` |
| OS/DE detection or watcher | `crates/lhc-core/src/platform/`, `keyboard_layout/`, `gamemode/`, `active_window/` |
| GNOME Shell side | `crates/lhc-core/assets/gnome-extension/` |
| Settings page | `apps/slint-shell/src/pages/<page>.rs` + `ui/<page>.slint` |
| Popup content or keys | `apps/slint-shell/src/popup_model.rs`, `ui/popup-*.slint` |
| Tray, global hotkeys, focus return | `apps/slint-shell/src/platform/{linux,portable}/` |
| CLI/IPC command | `apps/slint-shell/src/command.rs` |
| User-visible text | `src/i18n.rs` + `ui/i18n.slint` + every `translations/*/LC_MESSAGES/slint-shell.po` |

## Conventions

**Rust**
- Platform-specific code stays behind `#[cfg(...)]` and target-specific dependencies; shared UI and core logic are OS-independent.
- Dispatch on `platform::linux::detect()`, never on raw env vars like `XDG_CURRENT_DESKTOP`.
- Return `Result<_, String>` or typed errors at module boundaries, as neighbouring code does; no `unwrap()` on runtime paths (build scripts and tests are fine).
- Use typed values across boundaries: `command::Window` / `Popup` instead of strings, `ui/types.slint` enums instead of numeric codes between Rust and Slint.
- Comments match the surrounding code: modules carry a short `//!` header; public items get `///` when the purpose is not obvious; inline comments explain *why*. No narrative comments in code you did not otherwise change.
- Keep diffs minimal and focused; no drive-by refactors.

**Slint UI**
- Each settings page is one Slint global bound by one module in `src/pages/`. Shared widgets come from `controls.slint` (base in `controls-base.slint`, inline editors in `controls-inline.slint`); colours and metrics from `theme.slint`. System, light, dark and E-ink themes, plus high-contrast and reduce-motion, must all work.
- All user-visible text goes through `@tr(...)`. Rust never formats UI text: it sends an `i18n::Msg` (`Message { id, arg, count }`) and `Locale.text()` in `ui/i18n.slint` translates it. Technical error details stay in logs.
- Adding a message means updating `src/i18n.rs`, `ui/i18n.slint` and **every** PO catalog together (Russian by hand; other catalogs may start machine-translated). The build checks completeness, contexts, plural forms, placeholders and Rust message handlers. Text outside Slint is registered in `build-support/rust_messages.rs`.
- Changing a property or callback used from Rust requires a rebuild; Live Preview only reloads pure `.slint` changes.

**Vendored crates**
- `spell-framework` is a workspace member (covered by workspace tests); `i-slint-core` is a `[patch.crates-io]` dependency excluded from the workspace.
- Every change in `vendor/i-slint-core` or `vendor/spell-framework` is recorded in that directory's `PATCHES.md`. When upgrading Slint, port or drop the patch and keep the `=1.18.1` pins (`slint`, `i-slint-core`, `slint-build`) in sync.

## Definition of done

1. `cargo fmt --check`, clippy with `-D warnings` and workspace tests pass.
2. `cargo check --locked -p slint-shell --all-targets` passes (non-Spell build).
3. UI changes: `editor -- --smoke`, `interactions` and the relevant page example pass in a graphical session.
4. New user-visible strings: all catalogs updated and the translation checks pass.
5. User-visible behaviour changes: an entry under `## [Unreleased]` in `CHANGELOG.md` (no version or date; cargo-release adds them).
6. Platform-dependent behaviour: state the environment verified (e.g. KDE Wayland) and what remains untested.

Report the exact commands you ran and their results. If something could not be run (no graphical session, no `/dev/uinput`), say so.

## Boundaries

**Always**
- Put domain and platform logic in `lhc-core`; keep the shell thin.
- Edit config through `Document` (`edit` for transactions, `edit_library` for one core library operation); resolve paths through `StoragePaths::resolve()`.
- Support the current `config.json` and layout formats plus older `schemaVersion`s via the readers in `profile/`; no fallbacks for formats predating `schemaVersion`.

**Ask first**
- Adding a dependency or bumping Slint / `spell-framework` / `i-slint-core`.
- Changing the on-disk config or layout format, the IPC wire format or CLI commands.
- Running `cargo release --execute` or pushing `v*` tags (public release).
- Extending X11 support (best effort only, see platforms.md) or touching `_old/`.

**Never**
- Read or write the real user config from debug builds or tests (use `dev-files/`, `LHC_DEV_DIR` or a temp dir).
- Hardcode absolute paths, call Slint from `lhc-core`, or add UI text outside `@tr`.
- Use `_old/` code or docs as the specification or depend on it.
- Commit build artefacts, dev config/data or dependency caches.
- Enable `slint/live-preview` in regular or release builds.

## Further reading

- [`docs/development/guide.md`](docs/development/guide.md) — setup, run modes, env vars, checks, e2e levels, release.
- [`docs/development/platforms.md`](docs/development/platforms.md) — capability matrix, adding backends.
- [`docs/features.md`](docs/features.md) — feature behaviour.
- [`apps/slint-shell/README.md`](apps/slint-shell/README.md) — popup paths, benchmarks, diagnostic stands, Windows/macOS notes.
- [`docs/testing/`](docs/testing/README.md) — manual acceptance, GNOME, Windows VM.
- `apps/slint-shell/vendor/*/PATCHES.md` — what vendored crates change and why.
