Built as a desktop application on top of **Tauri 2**, **Nuxt 3**, **Nuxt UI v3**, **Vue 3** and **Tailwind CSS v4**.
Currently, only **Linux** is supported.

## Stack

- [Nuxt 3](https://nuxt.com) (SPA mode, `ssr: false`)
- [Nuxt UI v3](https://ui.nuxt.com) — components + theming
- [Tailwind CSS v4](https://tailwindcss.com) (bundled via Nuxt UI)
- [Vue 3](https://vuejs.org) + `<script setup>`
- [Tauri 2](https://tauri.app) for the native shell

## Prerequisites

- Node.js >= 20
- [pnpm](https://pnpm.io) >= 9
- Rust toolchain (`rustup`) — see [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS
- Linux-only system packages (Debian/Ubuntu example):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev \
    libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```
- Linux-only system packages (Arch/Manjaro example):
  ```bash
  sudo pacman -S webkit2gtk-4.1 base-devel curl wget file libxdo \
    openssl libappindicator-gtk3 librsvg fuse2
  ```

## Install

```bash
pnpm install
```

## Run in development (desktop window)

```bash
pnpm tauri:dev
```

This starts the Nuxt dev server on the port specified in `.env` (default `http://localhost:3000`) and launches the Tauri window pointing at it.

Development mode is supported on Linux only. The current `pnpm dev` / `pnpm tauri:dev` scripts are not intended to work on Windows.

## Run the web UI only (browser)

```bash
pnpm dev
```

## Run UI tests

```bash
pnpm test
```

Available commands:

- `pnpm test:unit` — fast unit tests for TS helpers/composables without the Nuxt runtime
- `pnpm test:components` — component tests for `.vue` files in the Nuxt test environment
- `pnpm test:unit:watch` / `pnpm test:components:watch` — watch mode during UI work

## Build a production desktop bundle

```bash
pnpm tauri:build
```

The Nuxt frontend is statically generated into `.output/public` (via `pnpm generate`) and bundled by Tauri into `src-tauri/target/release/bundle/`.

## Project layout

```
.
├── app.vue               # Root Vue component (uses Nuxt UI <UApp>)
├── app.config.ts         # Nuxt UI theme config
├── assets/css/main.css   # Tailwind + Nuxt UI entry
├── nuxt.config.ts        # Nuxt config (SPA + static + Vite tuning for Tauri)
├── package.json
└── src-tauri/            # Rust / Tauri side
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/default.json
    └── src/
        ├── main.rs
        └── lib.rs        # run() + Tauri command handlers
```

## Notes

- Before first `tauri dev` / `tauri build` you will need icons in `src-tauri/icons/`. Generate them from a square PNG (>= 1024x1024) — replace the path below with a real file:
  ```bash
  pnpm tauri icon ./my-source-icon.png
  ```
  Quick placeholder with ImageMagick:
  ```bash
  magick -size 1024x1024 xc:"#4f46e5" /tmp/app-icon.png
  pnpm tauri icon /tmp/app-icon.png
  ```
- Rust commands are registered in `src-tauri/src/lib.rs` and invoked from the frontend with `@tauri-apps/api/core`. When running `pnpm dev` in a plain browser, guard Tauri imports/calls so they do not crash outside the native shell.

## Where app state is stored

The app uses Tauri's platform-specific app directories based on the bundle identifier `dev.bozonx.left-hand-control`.

- **Linux** (primary):
  - config: `~/.config/dev.bozonx.left-hand-control/config.json`
  - UI state: `~/.config/dev.bozonx.left-hand-control/ui-state.json`
  - current working layout: `~/.local/share/dev.bozonx.left-hand-control/current-layout.yaml`
  - user layouts: `~/.local/share/dev.bozonx.left-hand-control/layouts/`

If `XDG_CONFIG_HOME` or `XDG_DATA_HOME` is set, the app uses those directories instead of `~/.config` and `~/.local/share`.

- `config.json` stores app settings and the currently selected layout id.
- `ui-state.json` stores UI-only state: active tab and the last selected keymap layer.
- `current-layout.yaml` stores the current editable layout, including macros.

- **macOS / Windows** (future support):
  - Standard Tauri app data directories.

## Key-mapper (Linux only)

The mapper reads events from a grabbed `/dev/input/eventX` device and emits remapped events via a `uinput` virtual keyboard. It runs inside the app process: start it from **Settings → Key-mapper**. Closing the window minimizes to tray — the mapper keeps running. Use the tray menu (`Выход`) to fully quit.

### Permissions setup (one-time)

1. Add yourself to the `input` group so you can read `/dev/input/event*`:
   ```bash
   sudo usermod -aG input "$USER"
   ```
   Log out and back in for it to take effect.

2. Allow access to `/dev/uinput` for the `input` group via udev:
   ```bash
   echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' \
     | sudo tee /etc/udev/rules.d/99-uinput.rules
   sudo udevadm control --reload-rules
   sudo modprobe uinput
   ```

3. Restart the app. Pick your keyboard in **Settings → Key-mapper → Клавиатура** and press **Запустить**.

If the start button returns a permissions error, verify that the user is in `input` (`id -nG | tr ' ' '\n' | grep input`) and that `ls -l /dev/uinput` shows group `input` with `rw`.

### Activation conditions

Per-rule triggers and per-layout whitelist/blacklist sets can be gated by:

- **Game Mode** state (on / off / ignore).
- **Current keyboard layout** — the rule / layout matches only when the system layout is in the allowed list.
- **Active application** — substrings (case-insensitive, OR) matched against the focused window's title and app id (`WM_CLASS` on X11, `app_id` / `class` on Wayland). Empty list means "do not check".

Active-window detection backends:

- **X11** (any DE): `xdotool` + `xprop`.
- **KDE Plasma (Wayland)**: `kdotool` (install: `paru -S kdotool`).
- **Hyprland**: `hyprctl` (ships with Hyprland).
- Other Wayland sessions (GNOME, Sway, …): not yet implemented — app-based conditions evaluate as "no match".
