# Platform support and backend development

| Concern | KDE | GNOME | Sway | X11 generic | Windows | macOS |
| --- | --- | --- | --- | --- | --- | --- |
| Key interception (evdev+uinput) | ✅ | ✅ | ✅ | ✅ | ❌ stub | ❌ stub |
| Literal text (xdg-desktop-portal) | ✅ | ✅ | ✅ | ⚠ if portal present | ❌ | ❌ |
| Layout detection | ✅ DBus `org.kde.keyboard` | 🚧 | 🚧 | 🚧 | 🚧 | 🚧 |
| System actions (`switchDesktopN`, `switchLayoutN`, `windowToDesktopN`) | ✅ KWin DBus | 🚧 | 🚧 | 🚧 | ❌ | ❌ |
| KDE global shortcuts (`kde:`) | ✅ kglobalaccel | — | — | — | — | — |
| Layer-shell popups (Spell) | ✅ | ❌ winit fallback | ✅ not accepted | ❌ winit | — | — |
| Game mode: GameMode daemon | ✅ D-Bus | ✅ | ✅ | ✅ | — | — |
| Game mode: fullscreen / active window | ✅ KWin script | ❌ no public API | ✅ IPC (Hyprland too); other wlroots compositors via `wlr-foreign-toplevel` | ⚠ `xprop` (+ `xdotool`, `xrandr` for borderless) | ✅ Win32 | ⚠ untested (`NSWorkspace`, `CGWindowList`) |
| Game mode: known games | ✅ Steam/Lutris processes, `.desktop` `Game` category | ✅ | ✅ | ✅ | ✅ Steam `RunningAppID`, Game Bar list (untested) | ⚠ `Info.plist` category (untested) |

🚧 = skeleton module returning `Ok(None)`/`None` with a planned-implementation note; ❌ stub = explicit "not implemented" error. Both compile and are safe to ship.

**Adding a Linux DE backend:** add a variant to `platform::linux::Desktop` and `classify_desktop()`; create `layout/linux_<de>.rs` with `current()` and `start_watcher()` that report through `super::publish(&info)`; wire it into `layout/mod.rs`; add `mod <de>` with `resolve(name) -> Option<SysAction>` in `mapper/system.rs` and wire it into the dispatcher.

**Adding Windows/macOS interception:** replace the non-Linux stubs in `mapper/runtime.rs` with `mapper/windows.rs` / `mapper/macos.rs` mirroring `mapper::linux` (`list_keyboards()`, `spawn()`, `Handle`). The engine currently uses `evdev::Key`; introduce a generic key type first. Windows: `SetWindowsHookExW(WH_KEYBOARD_LL)` + `SendInput`. macOS: `CGEventTapCreate` + `CGEventPost` (requires Accessibility permission). Test Windows in a VM as described in [Windows test environment](../testing/windows.md).

**Linux runtime requirements:** read access to `/dev/input/event*` and rw to `/dev/uinput` (group `input` + udev rule), `xdg-desktop-portal` with the DE backend for literal text, `xprop` (+ `xdotool` for title conditions) on X11, `hyprctl` / `swaymsg` on Hyprland / Sway (other wlroots compositors need `zwlr_foreign_toplevel_manager_v1`, nothing to install); KDE Wayland needs nothing extra (a KWin script reports focus over D-Bus, `kdotool` is only a fallback). Setup commands are in [Linux development guide](linux.md).

