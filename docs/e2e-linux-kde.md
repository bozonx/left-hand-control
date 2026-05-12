# E2E Desktop Testing

The E2E layer runs the real Tauri desktop binary through `tauri-driver` and WebdriverIO. Keep these tests on real desktop sessions or full VMs; Linux DE behavior depends on DBus, the compositor, portals, WebKitGTK and the input stack.

## Local Run

Install the one-time WebDriver binary:

```bash
cargo install tauri-driver --locked
```

Run the generic desktop smoke tests:

```bash
pnpm install --frozen-lockfile
pnpm test:e2e
```

Run the KDE Plasma Wayland smoke target:

```bash
pnpm install --frozen-lockfile
pnpm test:e2e:kde
```

Run the Windows desktop smoke target inside a Windows VM:

```powershell
pnpm install --frozen-lockfile
pnpm test:e2e:windows
```

The runner builds `src-tauri/target/debug/left-hand-control`, starts `tauri-driver` on port `4444`, and launches the app with an isolated `LHC_DEV_DIR` under `/tmp`. Override these when needed:

```bash
LHC_DEV_DIR=/tmp/lhc-e2e LHC_TAURI_DRIVER_PORT=4455 pnpm test:e2e:kde
```

If the debug binary is already built:

```bash
LHC_E2E_SKIP_BUILD=1 pnpm test:e2e:no-build
```

Supported `LHC_E2E_TARGET` values:

- `desktop`: generic app startup, routing, storage and IPC smoke.
- `kde-wayland`: generic smoke plus KDE Plasma Wayland service assertions.
- `windows`: generic smoke plus Windows platform assertions.

## Manjaro KDE Plasma Wayland VM

Use a full VM image, not a Docker container, for KDE/Wayland validation. The VM should boot into a real Plasma Wayland session for the same user that runs the tests.

Required runtime/build tools:

```bash
sudo pacman -S --needed nodejs pnpm rustup base-devel curl wget file \
  webkit2gtk-4.1 webkitgtk-6.0 libxdo openssl libappindicator-gtk3 librsvg fuse2 \
  qt6-tools glib2 xdg-desktop-portal xdg-desktop-portal-kde
rustup default stable
cargo install tauri-driver --locked
```

`tauri-driver` needs the native `WebKitWebDriver` binary on Linux. On current Arch/Manjaro it is provided by `webkitgtk-6.0` as `/usr/bin/WebKitWebDriver`. If your distribution installs it elsewhere, pass the absolute path:

```bash
LHC_E2E_NATIVE_DRIVER=/path/to/WebKitWebDriver pnpm test:e2e:kde
```

Validate the session before running E2E:

```bash
echo "$XDG_CURRENT_DESKTOP"
echo "$XDG_SESSION_TYPE"
qdbus org.kde.keyboard /Layouts org.kde.KeyboardLayouts.getLayoutsList
qdbus org.kde.KWin /KWin org.kde.KWin.supportInformation >/dev/null
```

Expected values for the first target:

```text
XDG_CURRENT_DESKTOP=KDE
XDG_SESSION_TYPE=wayland
```

## CI Shape

Keep the existing GitHub-hosted jobs for lint, unit/component tests, Rust tests and bundle builds. Add desktop E2E as self-hosted VM jobs with labels that describe the real environment:

```yaml
e2e-kde-wayland:
  runs-on: [self-hosted, linux, manjaro, kde, wayland]
  steps:
    - uses: actions/checkout@v4
    - uses: actions/setup-node@v4
      with:
        node-version: 20
    - uses: pnpm/action-setup@v4
      with:
        version: 9
    - uses: dtolnay/rust-toolchain@stable
    - run: pnpm install --frozen-lockfile
    - run: cargo install tauri-driver --locked
    - run: pnpm test:e2e:kde
```

For repeatability, prefer VM snapshots: restore a clean snapshot, run E2E, collect logs/artifacts, then discard the VM state.

## Windows VM

Use a Windows 11 VM with an interactive desktop session for the runner user.

Install:

- Node.js 20
- pnpm 9
- Rust stable with the MSVC toolchain
- Microsoft Edge WebView2 Runtime
- `tauri-driver`

One-time setup:

```powershell
rustup default stable
cargo install tauri-driver --locked
```

Run:

```powershell
pnpm install --frozen-lockfile
pnpm test:e2e:windows
```

The Windows target currently checks startup, routing, isolated debug storage, Tauri IPC and `get_platform_info` returning `windows`. It intentionally does not test key interception because the Windows mapper backend is still a stub.

## Target Matrix

Current target:

- `kde-wayland`: Manjaro KDE Plasma Wayland. Checks app startup, routing, isolated debug storage, Tauri IPC, KDE desktop detection, KDE layout detection and KDE system action services.
- `windows`: Windows 11 desktop. Checks app startup, routing, isolated debug storage, Tauri IPC and Windows platform detection.

Future targets:

- `linux-gnome-wayland`: app startup and GNOME-specific capability expectations.
- `linux-kde-x11`: KDE behavior under X11.
