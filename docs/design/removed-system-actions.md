# Removed `sys:` actions

Reference for the built-in system actions removed in October 2026. They were
all KDE-only wrappers around KDE D-Bus calls; keeping a hand-written list per
desktop environment did not scale. What remains in `sys:` is the small set
that cannot be expressed with a key: `switchDesktopN`, `switchLayoutN` and
`windowToDesktopN`.

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
| `windowClose` | `kwin` | `Window Close` | Chord `Alt+F4` |
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
| `windowToNextDesktop` | `kwin` | `Window to Next Desktop` | `kde:kwin/Window to Next Desktop` |
| `windowToPreviousDesktop` | `kwin` | `Window to Previous Desktop` | `kde:kwin/Window to Previous Desktop` |
| `windowKeepAbove` | `kwin` | `Window Above Other Windows` | `kde:kwin/Window Above Other Windows` |
| `windowMaximizeVertical` | `kwin` | `Window Maximize Vertical` | `kde:kwin/Window Maximize Vertical` |
| `windowMaximizeHorizontal` | `kwin` | `Window Maximize Horizontal` | `kde:kwin/Window Maximize Horizontal` |
| `windowFullscreen` | `kwin` | `Window Fullscreen` | `kde:kwin/Window Fullscreen` |
| `windowOnAllDesktops` | `kwin` | `Window On All Desktops` | `kde:kwin/Window On All Desktops` |
| `moveWindow` | `kwin` | `Window Move` | `kde:kwin/Window Move` |
| `toggleNightColor` | `kwin` | `Toggle Night Color` | `kde:kwin/Toggle Night Color` |
| `screenOff` | `org_kde_powerdevil` | `Turn Off Screen` | `kde:org_kde_powerdevil/Turn Off Screen` |

## Kept or added

| Id | KDE implementation |
| --- | --- |
| `switchDesktopN` (1–10) | `org.kde.KWin /KWin org.kde.KWin.setCurrentDesktop(i32 N)` |
| `switchLayoutN` (1–10) | `org.kde.keyboard /Layouts org.kde.KeyboardLayouts.setLayout(u32 N-1)` |
| `windowToDesktopN` (1–10) | `kde:kwin/Window to Desktop N` |
| `windowClose` | `kde:kwin/Window Close` |
| `windowMinimize` | `kde:kwin/Window Minimize` |
| `windowMaximize` | `kde:kwin/Window Maximize` |
| `windowMaximizeVertical` | `kde:kwin/Window Maximize Vertical` |
| `windowKeepAbove` | `kde:kwin/Window Above Other Windows` |
| `windowToNextDesktop` | `kde:kwin/Window to Next Desktop` |
| `showDesktop` | `kde:kwin/Show Desktop` |
| `overview` | `kde:kwin/Overview` |
| `appLauncher` | `kde:plasmashell/activate application launcher` |
| `screenOff` | `kde:org_kde_powerdevil/Turn Off Screen` |

On macOS, `switchDesktopN`, `windowToDesktopN`, `windowToNextDesktop`,
`windowMaximizeVertical`, `windowKeepAbove`, `showDesktop` and `appLauncher`
have no public API; running them there is
ignored with a notification.

## Notes

- Before replacing an action with a chord, check its default in
  `kglobalaccel`. Chords only work while the user keeps the default binding.
- Standard keys rely on the desktop's media-key handling: KDE binds them
  through kglobalaccel (`XF86AudioRaiseVolume`, `XF86MonBrightnessUp`,
  `XF86ScreenSaver`, `XF86Display`, …). The mapper emits them through
  uinput like any other key.
- `DisplayOff` (`KEY_DISPLAY_OFF`) is not handled by KDE, so `screenOff`
  moved to `kde:` rather than a key.
- To list every kglobalaccel action on the running system:
  `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component allShortcutInfos`
  (component paths come from `org.kde.KGlobalAccel.allComponents` on `/kglobalaccel`).
