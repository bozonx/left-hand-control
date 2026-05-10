# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **Also read `AGENTS.md`** — it is the authoritative agent guide with deeper detail on Rust architecture, coding conventions, common pitfalls, and what not to do.

## Commands

```bash
pnpm install          # install deps + run nuxt prepare (generates types)
pnpm dev              # Nuxt dev server only (browser, no native window)
pnpm tauri:dev        # full desktop dev: Nuxt + Rust compiled + native window
pnpm tauri:build      # production native bundle → src-tauri/target/release/bundle/
pnpm lint             # ESLint check
pnpm lint:fix         # ESLint autofix
pnpm typecheck        # tsc --noEmit
pnpm test             # all tests (unit + component)
pnpm test:unit        # fast unit tests (no Nuxt runtime)
pnpm test:components  # component tests (Nuxt test environment)
pnpm test:unit:watch  # unit tests in watch mode
pnpm test:components:watch  # component tests in watch mode
```

Run a single test file:
```bash
pnpm test:unit -- path/to/file.test.ts
pnpm test:components -- path/to/Component.test.ts
```

## High-level architecture

### Two runtimes

The app has two separate runtimes that communicate over Tauri's IPC bridge:

- **Rust** (`src-tauri/src/`) — native shell, key interception engine, system actions, file I/O. Commands are defined with `#[tauri::command]` in `lib.rs` and called from TS via `invoke('command_name', { args })`.
- **TypeScript/Vue** (repo root) — Nuxt 4 SPA (`ssr: false`). All UI logic lives here. Tauri API calls must be guarded so the app degrades gracefully in `pnpm dev` (plain browser).

### Frontend state: one central singleton

`useConfig()` (`composables/useConfig.ts`) is the **entire app state**. It is a hand-rolled singleton (not Pinia) holding a `Ref<AppConfig>`. All UI components read from and write to it. A deep watcher auto-saves with a 300 ms debounce; explicit flushes are available via `flush()`.

`AppConfig` (`types/config.ts`) has two conceptual parts:
- **`LayoutPreset`** — the portable keyboard config: `layers`, `rules`, `layerKeymaps`, `macros`, `commands`, `quickActions`, `emojiPages`. This is what gets serialised to YAML user-layout files.
- **`settings: AppSettings`** — global app settings (device paths, timeouts, appearance, layout mode, …). Persisted separately to `config.json`.

### Action string format

Every bindable action is a plain string with a canonical prefix:

| Prefix | Example | Meaning |
|--------|---------|---------|
| *(none)* | `KeyA`, `Ctrl+KeyC` | Key chord / keystroke |
| `macro:` | `macro:copyLine` | User or system macro by id |
| `cmd:` | `cmd:toggleMusic` | User shell command by id |
| `sys:` | `sys:switchDesktop1` | Built-in system action |
| `app:` | `app:showQuickMenu1` | Built-in app action |
| `text:` | `text:TODO: ` | Literal text injection |
| `null` | — | Explicit swallow (suppress the key) |
| `""` / `undefined` | — | Native passthrough |

Helper functions for building/parsing these are in `types/config.ts` (`macroActionRef`, `parseCommandRef`, etc.).

### Composable patterns

Most feature logic lives in composables under `composables/`. Key ones:

- `useConfig()` — central state singleton (see above)
- `useMapper()` / `useMapperRuntime()` — mapper start/stop, device list
- `useLayers()`, `useMacros()`, `useRulesEditor()`, `useCommandEditor()` — feature-specific editors, all reading/writing `useConfig().config`
- `useLayoutLibrary()` — reading/writing user YAML layout files via Tauri fs commands
- `useUiState()` — UI-only ephemeral state (selected layer, etc.), persisted to `ui-state.json`

Composables that manage shared state use a singleton pattern (module-level `let singleton`). Test files call `resetConfigStateForTests()` to clear singletons between tests.

### i18n

All user-visible strings live in `i18n/locales/en-US.ts` (source of truth) and `i18n/locales/ru-RU.ts`. `useI18n` is Nuxt auto-imported — no explicit import needed. Every key present in EN must have a matching key in RU.

### Rust module layout

```
src-tauri/src/
├── lib.rs           # Tauri command handlers + plugin registration
├── platform/        # OS/DE detection — always dispatch through here, never raw env vars
├── mapper/          # key interception (evdev+uinput), system actions, portal text injection
└── layout/          # keyboard layout detection + watcher (KDE DBus, stubs for others)
```

The `mapper/` evdev+uinput engine is DE-agnostic. Only `mapper/system.rs` (system actions like `switchDesktopN`) dispatches per-DE via `platform::linux::detect()`.
