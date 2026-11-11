# Removed `sys:` actions

Reference for the built-in system actions removed in October 2026. They were
all KDE-only wrappers around KDE D-Bus calls; keeping a hand-written list per
desktop environment did not scale. What remains in `sys:` is the small set
that cannot be expressed with a key: `switchDesktopN`, `switchProfileN` and
`windowToDesktopN` (today `switchToDesktopN`, `tecloid:activateProfileN` and
`moveWindowToDesktopN`).

Later, a few common window and shell actions came back as cross-platform
abstractions (see "Kept or added"): each has a documented counterpart on
Linux desktops and Windows, so `sys:` holds platform-neutral ids while
`kde:` stays the place for KDE-specific shortcuts.

Each removed action is now covered by one of:

- **Key** — a standard evdev key that every desktop handles itself.
- **Chord** — the KDE default shortcut (checked on Plasma 6 via
  `kglobalaccel allShortcutInfos`, `defaultKeys`).
- **`kde:`** — the generic action `kde:<component>/<shortcut>`, which calls
  `org.kde.kglobalaccel /component/<component>
  org.kde.kglobalaccel.Component.invokeShortcut(<shortcut>)`. It works
  whether or not the shortcut has keys assigned.

Every row also lists its old kglobalaccel call, so any action can be written
as `kde:` if its key or chord does not work.

## Removed ids

| Old id | kglobalaccel component | Shortcut name | Replacement |
| --- | --- | --- | --- |
| `taskEntryN` (1–10) | `plasmashell` | `activate task manager entry N` | Chord `Meta+DigitN` |
| `showClipboardHistory` | — (`org.kde.plasmashell /klipper org.kde.klipper.klipper.showKlipperPopupMenu`) | — | Chord `Meta+KeyV` (`plasmashell/show-on-mouse-pos`) |
| `volumeDown` | `kmix` | `decrease_volume` | Key `AudioVolumeDown` |
| `volumeUp` | `kmix` | `increase_volume` | Key `AudioVolumeUp` |
| `muteAudio` | `kmix` | `mute` | Key `AudioVolumeMute` |
| `muteMicrophone` | `kmix` | `mic_mute` | Key `MicrophoneMuteToggle` |
| `brightnessDown` | `org_kde_powerdevil` | `Decrease Screen Brightness` | Key `BrightnessDown` |
| `brightnessUp` | `org_kde_powerdevil` | `Increase Screen Brightness` | Key `BrightnessUp` |
| `decreaseKeyboardBrightness` | `org_kde_powerdevil` | `Decrease Keyboard Brightness` | Key `KeyboardBacklightDown` |
| `increaseKeyboardBrightness` | `org_kde_powerdevil` | `Increase Keyboard Brightness` | Key `KeyboardBacklightUp` |
| `toggleKeyboardBacklight` | `org_kde_powerdevil` | `Toggle Keyboard Backlight` | Key `KeyboardBacklightToggle` |
| `toggleTouchpad` | `org_kde_touchpadshortcuts_desktop` | `ToggleTouchpad` | Key `TouchpadToggle` |
| `showDisplayConfig` | `org_kde_kscreen_desktop` | `ShowOSD` | Key `DisplayToggleIntExt` or chord `Meta+KeyP` |
| `lockSession` | `ksmserver` | `Lock Session` | Key `LockScreen` or chord `Meta+KeyL` |
| `logout` | `ksmserver` | `Log Out` | Chord `Ctrl+Alt+Delete` |
| `logoutWithoutConfirmation` | `ksmserver` | `Log Out Without Confirmation` | `kde:ksmserver/Log Out Without Confirmation` |
| `closeWindow` | `kwin` | `Window Close` | Chord `Alt+F4` |
| `quickTileWindowTop` | `kwin` | `Window Quick Tile Top` | Chord `Meta+ArrowUp` |
| `quickTileWindowBottom` | `kwin` | `Window Quick Tile Bottom` | Chord `Meta+ArrowDown` |
| `quickTileWindowLeft` | `kwin` | `Window Quick Tile Left` | Chord `Meta+ArrowLeft` |
| `quickTileWindowRight` | `kwin` | `Window Quick Tile Right` | Chord `Meta+ArrowRight` |
| `maximizeWindow` | `kwin` | `Window Maximize` | Chord `Meta+PageUp` |
| `minimizeWindow` | `kwin` | `Window Minimize` | Chord `Meta+PageDown` |
| `windowToNextScreen` | `kwin` | `Window to Next Screen` | Chord `Meta+Shift+ArrowRight` |
| `windowToPreviousScreen` | `kwin` | `Window to Previous Screen` | Chord `Meta+Shift+ArrowLeft` |
| `showDesktop` | `kwin` | `Show Desktop` | Chord `Meta+KeyD` |
| `toggleOverview` | `kwin` | `Overview` | Chord `Meta+KeyW` |
| `toggleGridView` | `kwin` | `Grid View` | Chord `Meta+KeyG` |
| `togglePresentWindowsAllDesktops` | `kwin` | `ExposeAll` | Chord `Ctrl+F10` |
| `togglePresentWindowsCurrentDesktop` | `kwin` | `Expose` | Chord `Ctrl+F9` |
| `windowMenu` | `kwin` | `Window Operations Menu` | Chord `Alt+F3` |
| `killWindow` | `kwin` | `Kill Window` | Chord `Meta+Ctrl+Escape` |
| `zoomIn` | `kwin` | `view_zoom_in` | Chord `Meta+Equal` |
| `zoomOut` | `kwin` | `view_zoom_out` | Chord `Meta+Minus` |
| `zoomActualSize` | `kwin` | `view_actual_size` | Chord `Meta+Digit0` |
| `walkThroughWindowsCurrentApp` | `kwin` | `Walk Through Windows of Current Application` | Chord ``Alt+Backquote`` |
| `launchKrunner` | `org_kde_krunner_desktop` | `_launch` | Chord `Alt+Space` |
| `launchSystemMonitor` | `org_kde_plasma_systemmonitor_desktop` | `_launch` | Chord `Meta+Escape` |
| `activateApplicationLauncher` | `plasmashell` | `activate application launcher` | Chord `Alt+F1` |
| `manageActivities` | `plasmashell` | `manage activities` | Chord `Meta+KeyQ` |
| `nextActivity` | `plasmashell` | `next activity` | `kde:plasmashell/next activity` |
| `previousActivity` | `plasmashell` | `previous activity` | `kde:plasmashell/previous activity` |
| `walkThroughWindowsAlternative` | `kwin` | `Walk Through Windows Alternative` | `kde:kwin/Walk Through Windows Alternative` |
| `moveWindowToNextDesktop` | `kwin` | `Window to Next Desktop` | `kde:kwin/Window to Next Desktop` |
| `windowToPreviousDesktop` | `kwin` | `Window to Previous Desktop` | `kde:kwin/Window to Previous Desktop` |
| `toggleWindowKeepAbove` | `kwin` | `Window Above Other Windows` | `kde:kwin/Window Above Other Windows` |
| `maximizeWindowVertically` | `kwin` | `Window Maximize Vertical` | `kde:kwin/Window Maximize Vertical` |
| `windowMaximizeHorizontal` | `kwin` | `Window Maximize Horizontal` | `kde:kwin/Window Maximize Horizontal` |
| `windowFullscreen` | `kwin` | `Window Fullscreen` | `kde:kwin/Window Fullscreen` |
| `windowOnAllDesktops` | `kwin` | `Window On All Desktops` | `kde:kwin/Window On All Desktops` |
| `moveWindow` | `kwin` | `Window Move` | `kde:kwin/Window Move` |
| `toggleNightColor` | `kwin` | `Toggle Night Color` | `kde:kwin/Toggle Night Color` |
| `turnOffScreen` | `org_kde_powerdevil` | `Turn Off Screen` | `kde:org_kde_powerdevil/Turn Off Screen` |

## Kept or added

Several ids above came back later as cross-platform `sys:` actions; their
rows above are history. A `sys:` id needs a documented API on at least two
platforms.

| Id | KDE | GNOME (extension) | Windows |
| --- | --- | --- | --- |
| `switchToDesktopN` (1–10) | `kde:kwin/Switch to Desktop N` | `workspace.activate` | — |
| `switchToKeyboardLayoutN` (1–10) | `org.kde.keyboard /Profiles setProfile(u32 N-1)` | input source `activate` | `WM_INPUTLANGCHANGEREQUEST` |
| `moveWindowToDesktopN` (1–10) | `kde:kwin/Window to Desktop N` | `change_workspace` | — |
| `closeWindow` | `kde:kwin/Window Close` | `delete` | `SC_CLOSE` |
| `minimizeWindow` | `kde:kwin/Window Minimize` | `minimize` | `SC_MINIMIZE` |
| `maximizeWindow` | `kde:kwin/Window Maximize` | `maximize` / `unmaximize` | `SC_MAXIMIZE` / `SC_RESTORE` |
| `maximizeWindowVertically` | `kde:kwin/Window Maximize Vertical` | maximize flags (some Mutter versions) | Win+Shift+Up/Down |
| `toggleWindowKeepAbove` | `kde:kwin/Window Above Other Windows` | `make_above` | `HWND_TOPMOST` |
| `moveWindowToNextDesktop` | `kde:kwin/Window to Next Desktop` | `change_workspace` | — |
| `moveWindowToPreviousDesktop` | `kde:kwin/Window to Previous Desktop` | `change_workspace` | — |
| `moveWindowToNextScreen` | `kde:kwin/Window to Next Screen` | `move_to_monitor` | Win+Shift+Right |
| `moveWindowToPreviousScreen` | `kde:kwin/Window to Previous Screen` | `move_to_monitor` | Win+Shift+Left |
| `switchToNextDesktop` | `kde:kwin/Switch to Next Desktop` | `workspace.activate` | Ctrl+Win+Right |
| `switchToPreviousDesktop` | `kde:kwin/Switch to Previous Desktop` | `workspace.activate` | Ctrl+Win+Left |
| `showDesktop` | `kde:kwin/Show Desktop` | — | `IShellDispatch4::ToggleDesktop` |
| `showOverview` | `kde:kwin/Overview` | `overview.toggle` | Win+Tab |
| `showAppLauncher` | `kde:plasmashell/activate application launcher` | `overview.showApps` | Ctrl+Esc |
| `turnOffScreen` | `kde:org_kde_powerdevil/Turn Off Screen` | `DisplayConfig.PowerSaveMode = 3` | `SC_MONITORPOWER` after 0.5 s |
| `lockSession` | `org.freedesktop.ScreenSaver.Lock` | `screenShield.lock` | `LockWorkStation` |

The `kde:` list in the action picker hides the KDE shortcuts these ids
already run, and the KWin `Switch to Desktop N` / `Window to Desktop N`
shortcuts above 10.

On macOS, `switchToDesktopN`, `moveWindowToDesktopN`, the next/previous desktop and
next/previous screen actions, `maximizeWindowVertically`, `toggleWindowKeepAbove`,
`showDesktop`, `showAppLauncher` and `lockSession` have no implementation; running
them there is ignored with a notification.

## Notes

- Before replacing an action with a chord, check its default in
  `kglobalaccel`. Chords only work while the user keeps the default binding.
- Standard keys rely on the desktop's media-key handling: KDE binds them
  through kglobalaccel (`XF86AudioRaiseVolume`, `XF86MonBrightnessUp`,
  `XF86ScreenSaver`, `XF86Display`, …). The mapper emits them through
  uinput like any other key.
- `DisplayOff` (`KEY_DISPLAY_OFF`) is not handled by KDE, so `turnOffScreen`
  moved to `kde:` rather than a key.
- To list every kglobalaccel action on the running system:
  `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos`
  (component paths come from `org.kde.KGlobalAccel.allComponents` on `/kglobalaccel`).
