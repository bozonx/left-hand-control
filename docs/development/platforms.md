# Platform support and backend development

| Concern | KDE | GNOME | Sway | X11 generic | Windows | macOS |
| --- | --- | --- | --- | --- | --- | --- |
| Key interception | ✅ evdev+uinput | ✅ | ✅ | ✅ | ✅ session-wide `WH_KEYBOARD_LL` + `SendInput` (untested on hardware) | ❌ stub |
| Literal text (xdg-desktop-portal) | ✅ | ⚠ needs active-window detection | ⚠ backend/protocol dependent | ⚠ portal and focus detection required | ✅ Unicode `SendInput` (mapper and popups) | ❌ mapper; popup Unicode only |
| Layout detection | ✅ DBus `org.kde.keyboard` | 🚧 | 🚧 | 🚧 | ✅ input language of the foreground window | 🚧 |
| System actions (`switchDesktopN`, `switchLayoutN`, `windowToDesktopN`, window/shell actions) | ✅ KWin DBus, kglobalaccel | 🚧 | 🚧 | 🚧 | ✅ Win32 messages and fixed shortcuts; no public API for `switchDesktopN` / `windowToDesktopN` | ❌; ids without a macOS API are ignored with a notification |
| KDE global shortcuts (`kde:`) | ✅ kglobalaccel | — | — | — | — | — |
| Layer-shell popups (Spell) | ✅ | ❌ winit fallback | ✅ not accepted | ❌ winit | — | — |
| Game mode: GameMode daemon | ✅ D-Bus | ✅ | ✅ | ✅ | — | — |
| Game mode: fullscreen / active window | ✅ KWin script | ❌ no public API | ✅ IPC (Hyprland too); other wlroots compositors via `wlr-foreign-toplevel` | ⚠ `xprop` (+ `xdotool`, `xrandr` for borderless) | ✅ Win32 | ⚠ untested (`NSWorkspace`, `CGWindowList`) |
| Game mode: known games (each source toggled separately) | ✅ Steam, Lutris, Heroic, umu, gamescope, `.desktop` `Game` category | ✅ | ✅ | ✅ | ✅ Steam `RunningAppID`, Game Bar list (untested) | ⚠ `Info.plist` category (untested) |

**X11 is not a supported target.** Interception (evdev + uinput) does not depend on the display server, so the mapper works in an X11 session, and the existing X11 code (`xprop`/`xdotool` active-window detection, `layout/linux_x11.rs`, winit popups) stays as best effort. It is not accepted, has no e2e or VM scenarios and is not extended: GNOME has dropped the X11 session and KDE drops it in Plasma 6.8, while the remaining X11 desktops (Xfce, Cinnamon, MATE, i3) would need their own DE backends anyway. Reconsider only on concrete demand for a specific desktop, as "support DE X", not "support X11".

🚧 = skeleton module returning `Ok(None)`/`None` with a planned-implementation note; ❌ stub = explicit "not implemented" error. Both compile and are safe to ship.

**Adding a Linux DE backend:** add a variant to `platform::linux::Desktop` and `classify_desktop()`; create `layout/linux_<de>.rs` with `current()` and `start_watcher()` that report through `super::publish(&info)`; wire it into `layout/mod.rs`; add `mod <de>` with `resolve(name) -> Option<SysAction>` in `mapper/system.rs` and wire it into the dispatcher.

**Adding a system action:** a `sys:` id must have a documented API (or a fixed system shortcut) on Linux desktops and Windows; undocumented APIs are not used. Add it to `PLAIN_SYSTEM_ACTIONS` in `profile/actions.rs`, list it in `MACOS_UNSUPPORTED_SYSTEM_ACTIONS` when macOS has no public API for it, implement it in every backend of `mapper/system.rs` that can, and add its picker label (`picker-action` in `ui/i18n.slint` plus the Russian PO). Desktop-specific actions stay in `kde:` and similar backend categories.

**Windows mapper preparation:** see [implementation stages and VM acceptance matrix](windows-mapper.md).

A command stores one script per OS: `linux` runs with `sh -lc`, `windows` with PowerShell (`-NoProfile -NonInteractive -ExecutionPolicy Bypass`, no console window). A command without a script for the running OS is skipped with a notice. Draft directory checks are portable; execute-bit hints apply only on Unix. File/folder selection uses the XDG portal on Linux and the common item dialog on Windows; macOS returns an explicit unsupported error.

**Windows backend:** `mapper::windows` installs `WH_KEYBOARD_LL` (and `WH_MOUSE_LL` for buttons when a mouse is selected) on its own message-loop thread. The callback only translates the event and queues it; the shared engine runs on the mapper thread, and output goes through `SendInput` with scan codes, so the target window's layout applies. Keys use Linux key codes everywhere (`mapper::key_code`); `mapper::scancode` maps set-1 scan codes, extended keys and virtual-key-only keys (Pause, Num Lock, media). Injected input is never processed, which excludes the mapper's own output and other tools. Windows does not auto-repeat injected keys, so the hook forwards repeats and the engine re-presses the held virtual key. Limitations: there is no per-device interception without a driver, so the mapper takes every keyboard of the session (`windows:keyboard`); input to elevated windows and the secure desktop (UAC, Ctrl+Alt+Del, lock screen) bypasses the hooks, and UIPI blocks injection into elevated windows, which is reported as an action failure. A named mutex keeps one mapper per session.

**Adding macOS interception:** replace the stub in `mapper/runtime.rs` with `mapper/macos.rs` mirroring `mapper::windows` (`list_keyboards()`, `spawn()`, `Handle`, an `EventSink` and a `LoopDriver` for `mapper::output`): `CGEventTapCreate` + `CGEventPost` (requires Accessibility permission). Test Windows in a VM as described in [Windows test environment](../testing/windows.md).

**Linux runtime requirements:** read access to `/dev/input/event*` and rw to `/dev/uinput` (group `input` + udev rule), `xdg-desktop-portal` with the DE backend for literal text, `xprop` (+ `xdotool` for title conditions) on X11, `hyprctl` / `swaymsg` on Hyprland / Sway (other wlroots compositors need `zwlr_foreign_toplevel_manager_v1`, nothing to install); KDE Wayland needs nothing extra (a KWin script reports focus over D-Bus, `kdotool` is only a fallback). Setup commands are in the [developer guide](guide.md#1-окружение).


Text input cancels stale jobs and checks the destination before emitting input. KDE uses KWin window ids; other backends can only compare their available active-window metadata, so identical windows may not be distinguishable. Clipboard paste requires `ext_data_control_manager_v1` or `zwlr_data_control_manager_v1` and Shift+Insert support in the destination. All offered MIME formats are preserved within bounded size/time limits; failure to snapshot leaves the clipboard unchanged. Transfer completion confirms that bytes were requested, not which client requested them. Windows popup text uses Unicode `SendInput` without the clipboard; macOS uses Enigo after foreground/window checks. Neither portable path implements keyboard interception or mapper macros. Windows/macOS adapters still require native acceptance testing.

## Mapper runtime guarantees

Live configuration updates retain held virtual keys and layer triggers until their
physical release; new presses use the updated bindings. Identical runtime configs
are ignored, so saving unrelated preferences does not interrupt a macro. Changed
configs cancel the active macro, stale text jobs and side effects waiting for popup
focus. Shutdown discards buffered popup input. Disabling commands also
revokes commands from decisions that were already pending.

The Linux keyboard backend rejects interfaces that also advertise pointer axes or
non-keyboard buttons, before grabbing them. Select a separate keyboard interface;
this backend cannot relay pointer input. Keyboard and optional mouse must resolve
to different devices. Their events use a common monotonic clock and are merged by
capture time. Native mouse clicks still cannot be suppressed or reliably delayed
until a virtual modifier reaches the compositor.

Macro expansion is limited to 1000 entries (including nested references) and ten
nesting levels, before execution. Portable file validation accepts known system
actions of every platform, so a layout made on KDE also starts on GNOME or Windows.
An action this OS or desktop cannot run (`kde:` outside KDE, `sys:switchDesktopN`
on Windows, a command without a script for this OS) is skipped when triggered and
reported with an "unavailable" notice; a macro containing such a step is skipped
as a whole rather than run halfway.
System-action failures reach the shell through core events. Popup actions bypass
the text worker, so a portal permission dialog cannot block popup dispatch.

macOS shows the missing interception capability and disables mapper device
controls. On Windows the device pickers offer the whole session ("All keyboards",
"All mice").
