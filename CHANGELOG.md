# Changelog

All notable changes to Left Hand Control are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
New entries go under `[Unreleased]`; `cargo release` turns it into a dated version section.

## [Unreleased]

### Added

- System action `sys:windowMaximizeVertical` stretches the focused window to the full screen height and restores it on the next press: KDE through KWin, GNOME through the extension where Mutter exposes partial maximization, Windows through Win+Shift+Up/Down. The bundled LHC1 layout uses it instead of the KDE shortcut.
- The Layers page marks keys and extra keys that keep their modifiers held until the layer is released with an accent dot; the key tooltip explains it.
- Interface language choices and machine-translated catalogs for German, French, international Spanish, Italian, Portuguese, Polish, Turkish, Ukrainian, Czech, Hungarian, Romanian, Greek, Croatian, Georgian, Kazakh, Simplified and Traditional Chinese, Japanese, and Korean.

### Changed

- Application lists in automatic layout rules, trigger rules and game mode share the same heading, add button and list frame.

- Action capture shows held keys and waits until every captured key is released, preserving the full combination regardless of release order.

- Rule headers truncate advanced option summaries before condition labels when the window narrows.

- The device picker shows each device on two lines: its name (read from the `by-id` link when the device reports none) and a shortened path. It is wider, highlights the selected device, folds non-matching and virtual devices behind “Show N more devices”, and keeps a saved device that is not connected as a “Not connected” entry instead of switching to manual input.

- Gate long-hold actions, emoji and quick-action popups, user macros, mouse assignment controls and KDE action selection behind In development features.

- The emoji library offers four language-independent text snippets: "TODO: ", "FIXME: ", "Continue." and "Thanks!".

- The automatic layout rule dialog fits its content without empty gaps, shows the condition hint under “When”, and adds applications with icon-only “+” buttons; empty application lists no longer cut off wrapped text.

- Removed the large Settings heading and redundant headings on the Main, Key behavior and Accessibility settings pages.

- Updated Slint to 1.18.1 and the bundled Spell layer-shell library to its Slint 1.18 revision; the local text editing, shortcut and scrolling fixes were ported.
- Selecting a quick action cell only selects it; the action picker opens from the action field in the side panel.
- Dragging a cell onto another one in the emoji and quick action menu grids swaps the two cells instead of shifting every cell in between.

- Built-in layouts use their metadata title for display and suggested copy names instead of the asset filename (LHC1 rather than LHC1_by_Ivan_K).

- Separate built-in macro actions (`builtin:<id>`) from user macros (`macro:<id>`), allowing identical IDs. Schema version remains 1; existing built-in references need updating without automatic migration.

- Commands, quick actions, typing test, top-bar keyboard language, automatic layout switching, E-ink theme, layout name notifications and held-layer hints are shown only with “In development features” enabled.

- Both JSON formats accept and preserve an optional string `$schema`. Layouts also preserve portable `meta.name`, `meta.author` and `meta.version`, independently of the local filename.
- Rule and macro timing overrides now reject explicit `null`; omit the field to inherit settings. Schema versions remain 1.

- An action with an unknown `word:` prefix (for example, from a newer version) is kept on save and reported as an unknown action type instead of an invalid key.
- `config.json` and layout files now have independent schema versions (both still 1). A file with an older schema version is backed up as `<file>.v<N>.bak` before the application first overwrites it. Editing these files by hand is unsupported and at your own risk.

- Game mode controls are available only after enabling the hidden “In development features” checkbox below the general settings paths.

- Keystrokes outside macros wait at most 50 ms between modifiers and key, so a long macro modifier delay no longer freezes the keyboard; macros still use the full setting.
- `app:showQuickMenuN` and `app:showEmojiMenuN` open a page by its position with no upper limit (the action picker offers pages 1–10). Moving or deleting a page no longer rewrites these actions, so reordering pages changes what a key opens, like reordering the library does for `app:activateLayoutN`.
- The library order moved from `state.layoutOrder` to `settings.layoutOrder` in `config.json`. A config that still has `state.layoutOrder` is rejected: move the field into `settings` by hand.
- The mapper accepts the same numbered system actions as the action picker (1–10), and `pause:` steps follow the macro editor's 1–10000 ms range.

### Fixed

- Restored `text:` input into the settings typing test by tracking text destinations separately from rule conditions and identifying popup windows explicitly. Focus cancellation now reports a destination error instead of suggesting a mapper restart.
- Repeated presses of a layer key that keeps its modifiers held (Tab for Alt+Tab) no longer press the held modifier again or wait for the modifier delay, so fast window switching keeps up.
- The “Command started” notification is shown again when a command action runs.
- Unplugging the optional mouse no longer stops keyboard mapping; the mouse reconnects independently with balanced button releases.
- Keyboards selected through `/dev/input/by-id` reconnect after changing USB ports, and the shell shows when the mapper is waiting for its keyboard.

- A layer key bound to an empty macro or to an action that does nothing on this system no longer types the physical key.
- Keys bound to a command that cannot run (commands disabled, no script for this OS) now show the "action unavailable" notice instead of silently doing nothing; an `app:` step that cannot run makes its macro unavailable like a `sys:` step.
- While a popup is opening, releases of keys pressed before it pass through, so the popup no longer opens with a modifier still held.
- The action picker on Windows no longer marks commands and system actions as unavailable, and marks commands without a script for the current OS.
- A deleted or damaged layout file that was open for editing no longer makes the whole configuration unavailable; the editor starts from a fresh layout and the file is left untouched.
- Renaming a library layout by changing only letter case works on Windows and macOS.

## [0.4.0] - 2026-10-24

### Added

- Windows x64 portable ZIP packaging with SHA256 checks, release artifacts and lifecycle acceptance of the extracted executable.
- Cross-platform system actions `sys:windowClose`, `windowMinimize`, `windowMaximize`, `windowKeepAbove`, `showDesktop`, `overview`, `appLauncher` and `screenOff` (KDE implementation through kglobalaccel). On macOS, `sys:` actions without a public API there are ignored with a notification.
- The KDE shortcuts category of the action picker explains where its list comes from.

### Changed

- The bundled layout uses the new system actions instead of the matching `kde:` shortcuts.

### Fixed

- Linux popup hotkeys on the grabbed keyboard are consumed before uinput output, preventing Ctrl+Alt+F11/F12 from switching virtual consoles while the mapper runs.
- Linux mapper reconnects the selected keyboard after ENODEV, preserves the latest configuration and refuses a different device reusing the selected path.
- KWin scripts are scoped to each application's D-Bus connection, so a second instance cannot replace or unload the running instance's focus watcher.
