# left-hand-control

Key mapper for desktops which suggests a smart layout for professionals, mostly using the left hand for control functions.

Built as a desktop application on top of **Tauri 2**, **Nuxt 3**, **Nuxt UI v3**, **Vue 3** and **Tailwind CSS v4**.

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

## Install

```bash
pnpm install
```

## Run in development (desktop window)

```bash
pnpm tauri:dev
```

This starts the Nuxt dev server on `http://localhost:3000` and launches the Tauri window pointing at it.

## Run the web UI only (browser)

```bash
pnpm dev
```

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
        └── lib.rs        # exposes the `greet` command
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
- The Rust side exposes a sample `greet` command wired up in `app.vue` as an example of the JS ↔ Rust bridge.
