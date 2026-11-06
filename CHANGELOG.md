# Changelog

All notable changes to Left Hand Control are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
New entries go under `[Unreleased]`; `cargo release` turns it into a dated version section.

## [Unreleased]

### Changed

- Linux: a one-character `text:` action (such as `text:,` on a tap key) is sent as an ordinary key press when the active keyboard layout has that character and no modifier is held. It no longer waits for the text backend, so it cannot land after the next letters, be lost on a window change or a full text queue, or wait for the first portal consent. Longer text, macro text steps, characters missing from the layout and desktops that do not report the layout use the text backend as before.

- Hyprland active-window events use the runtime socket directory; support for the pre-0.40 `/tmp/hypr` socket location was removed.

- Emoji and Quick action popups use Tab/Shift+Tab to move between the grid and Close; PageUp/PageDown switch pages. Letter and number shortcuts remain available.

- Triggers: unassigned "When held" and "Double-tap action" fields are shown as a quiet "+ Assign action" button without a frame (it appears on hover and keyboard focus), and their labels are dimmed, so assigned actions stand out. E-ink and high-contrast themes keep a thin frame.
- Rules: the "Enabled" checkbox moved from the rule card into "Advanced rule options". A disabled rule is shown dimmed but stays fully editable; the options button keeps full contrast.
- Layouts page: the unsaved-changes question always offers to save; a working copy not yet in the library asks for a name first and then continues. In the "Layout changed on disk" dialog the safe "Save as new…" is now the primary button instead of "Overwrite".
- Quick actions no longer have icons.
- The Slint software renderer is no longer built: `SLINT_BACKEND` accepts `winit-skia` (the default) and `winit-skia-software` for machines without a GPU. `winit-software` drew emoji as empty cells or monochrome outlines.
- Config and layout formats were finalized before the first release (`version` stays 1; files written by earlier development builds must be recreated or edited by hand). See `docs/config-format.md`:
  - `config.json`: layout selection is grouped under `settings.layouts` (`mode`, `manual`, `order`, `auto.rules`, `auto.fallback`) instead of `layoutMode`, `activeLayoutId`, `layoutOrder`, `autoRules` and `autoDefaultLayoutId`; `inDevelopment` is renamed to `experimental`.
  - Enum values use camelCase (`eInk`, `libeiPure`), interface languages are plain language codes (`en`, `ru`), and paste shortcuts are key chords (`Shift+Insert`, `Ctrl+KeyV`, `Ctrl+Shift+KeyV`).
  - `config.json` stores only settings that differ from the defaults, so later releases can improve defaults.
  - Layout files: `description` moved into `meta`; the obsolete quick-action `icon` field is rejected; layer, macro and command names equal to their id are not written.
  - Both files: `schemaVersion` is renamed to `version`; the author's version of a layout stays in `meta.version`.
  - Action ids name the verb first: `sys:closeWindow`, `minimizeWindow`, `maximizeWindow`, `maximizeWindowVertically`, `toggleWindowKeepAbove`, `moveWindowToNextDesktop`, `moveWindowToDesktopN`, `showOverview`, `showAppLauncher`, `turnOffScreen`; `app:toggleGameMode`, `setGameModeAuto`, `setGameModeOn`, `setGameModeOff`. `sys:switchLayoutN` is `sys:switchKeyboardLayoutN`, so it is not confused with the application's layouts. Built-in macros `prevLineEnd` and `prevLineStart` are `previousLineEnd` and `previousLineStart`.
  - Layout files: `quickActionPages` and `emojiPages` are `quickMenuPages` and `emojiMenuPages` (generated page ids `quick-N` and `emoji-N`); command scripts are `sh` and `powershell` instead of `unix` and `windows`; `hold.keys` is `hold.action`; the layer list `holdModifiers` is replaced by the key form `{ "action": "Alt+Tab", "holdModifiers": true }`. Layer keys are written in keyboard order.
  - `config.json`: `layouts.auto.rules[].layoutId` is `layout`, `state.editedLayoutId` is `editedLayout`, `gameMode.processes[].onlyActiveWindow` is `foregroundOnly`, and `linux.terminalPasteShortcut` uses `"sameAsPaste"` instead of `null`.
- The tabs of the Settings page show icons next to their names.
- Focus rings appear only while navigating with the keyboard (Tab, arrow keys) and disappear on the next mouse click, as in web browsers. Text fields keep their focus outline, and the high-contrast and E-ink themes always show focus rings.
- Action picker: arrow keys move the selection in the item list and Enter applies it; Stop during key capture keeps the keys pressed so far; an unsupported key and a failed read of KDE shortcuts are reported.
- Game mode (in development): toggling returns to Auto when the opposite state is what detection reports, so a toggle key no longer leaves automatic detection for good; a toggle from a key binding or the CLI shows the new state on screen. The top-bar button always reads Auto/On/Off in English to keep its fixed width, and its tooltip says that popup hotkeys are off in game mode. Terminals, code editors, image viewers and music players no longer count as fullscreen games; the running-process list is read every 2 seconds instead of every second while no game runs.

### Fixed

- Linux mapper: keys of the physical keyboard outside the standard ranges (such as macro keys `KEY_MACRO1`) are passed through instead of being dropped by the virtual keyboard.
- Commands: a command that exits with an error (syntax error, missing program, permission denied) is reported with its exit status and the start of its error output instead of failing silently.
- Linux mapper: system actions still queued when the mapper stops, or queued for more than 2 seconds behind a slow one, no longer run later; a hung D-Bus call ends after 5 seconds.
- Device selection: when some input devices cannot be opened, the accessible devices are still listed together with a "No access" message, and the access check reports the denied keyboard even if another device is readable.
- Device selection: "Show all devices" and "Use the stable path" can be reached with Tab and activated with Enter or Space, with a visible focus ring.

- Triggers: dismissing the hold menu releases the modal state; stale dialogs and pickers reject edits after document changes. Advanced and condition drafts survive failed edits and background refreshes. Diagnostics follow macro and command changes, empty-trigger drafts remain removable, and suppressed taps no longer offer a misleading native hold option.

- Layouts: duplication keeps its source after Save As or a library refresh; damaged UTF-8 files can be deleted with byte-preserving rollback. Empty library files are consistently rejected. Description drafts survive background refreshes after a failed save, and normal saves recheck the disk snapshot after staging the replacement file. Metadata loaders are isolated per library page.

- Settings keep invalid advanced timing fields reachable, identify their validation failures, and let quitting retry or discard unsaved settings before deciding about layout changes. Both mapper start controls save and validate the same draft. Device discovery keeps the last successful list on failure and displays loading and errors in the picker. Optional mouse failures and reconnection are reported without stopping keyboard mapping.

- Emoji and Quick actions: cut operations are cancelled when pages are reordered, deleted or reloaded; previews save pending edits and show the edited layout without executing actions. Failed assignments keep the action picker open, catalog labels follow replacement actions while custom labels are preserved, and emoji limits count Unicode code points instead of UTF-16 units.

- Macros keep the final text step active until delivery finishes; text cancellation and failures belong to the originating run, including timeout and reconfiguration.
- The macro editor preserves incoming references while renaming an invalid draft, prevents deleting referenced macros, and lets nested actions use new draft IDs. Global delay hints update when settings change.
- Failed macro commands log the originating macro and step together with bounded stderr, without blocking input or cancellation.

- Layers page: choosing a key of the layer keyboard as an extra key, or a key that already has a row, now says so instead of reporting an invalid configuration; the rule is also enforced when saving. A layer whose stored name is invalid can have its description edited. Edits elsewhere (settings, other layers) no longer cancel an open layer dialog or picker as a conflict. A failed delete or create is reported once. Deleting the last layer selects the new last one, and "Duplicate" proposes a free name ("Name (copy) 2").

- The layout library wraps its toolbar in narrow windows, keeps layout names readable beside automatic rules, and scrolls the whole automatic-rules panel so its default remains reachable.
- Automatic layout switching preserves explicit passthrough defaults and missing rule targets. Active-layout indicators follow the mapper's accepted configuration after a failed switch; retry and unchanged-layout paths are covered without hardware. Automatic-mode controls stay available when experimental features are hidden, and rule warnings account for missing targets and game-mode Off conditions.
- KDE keyboard-layout detection clears stale state on errors, unavailable services and invalid groups, announces recovery, and reconnects after service-owner changes.

- Windows: saving settings and layouts retries briefly when another process temporarily locks a file. A failed replacement keeps the original file intact.

- Mapper configuration updates preserve the order of queued actions. Popup menus retain the mapper's accepted layout after a failed automatic switch. Stopping the mapper releases window-title collection, and event overload no longer grows an unbounded backlog or replays actions from a previous mapper session.

- Held-layer hints refresh after layout edits and language changes without restarting their delay. Layout OSDs describe the last successfully applied layout, and rapid OSD requests are delivered in order with only the latest waiting message retained.

- Saving and discarding layouts: quitting asks about unsaved changes; unrelated library edits preserve external-change conflicts; unavailable saved layouts can be reset; failed layout switches keep page drafts; failed renames remove the newly published copy.

- Settings toolbars: layout menus scroll in long libraries, full layout names are available in tooltips, automatic switching retains the editing-context warning, and failures determining the active layout are shown explicitly. Small-window geometry checks cover translated toolbars and overlapping controls.

- Keyboard navigation: tab lists, menus, device lists, action catalogs and key grids move real focus with arrow keys and leave each group with Tab. Enter acts on the focused action; Escape clears searches before closing dialogs.
- Keyboard focus is retained while editing menu cells and reordering rows, returns after dialogs and page changes, and follows mouse activation consistently. ComboBox options remain provisional until confirmed, and destructive confirmations initially focus Cancel.

- Macros: editor and runtime share expansion-limit checks; renaming carries nested references before validation, and referenced macros cannot be deleted. Step errors and unapplied drafts are visible, with a discard action. Macro cards stack their fields and steps in narrow windows so action buttons remain reachable.
- Macro playback honours trailing pauses and nested timing overrides. Command steps wait for successful completion, abort the sequence on failure, and are cancelled on shutdown, reconfiguration or timeout. Queue overflow and timeout now notify the user. Updated macro documentation and all interface translations.

- Game mode (in development): the toggle key, the CLI and the tray no longer change or save the mode while in-development features are off; a toggle reads and flips the state at once. Steam Big Picture and other launchers in fullscreen no longer turn game mode on, and Auto no longer reports itself unavailable where fullscreen detection is unsupported. The process picker no longer lists truncated duplicate names or freezes the window while it reads processes.
- Action picker: capturing a combination for a plain-key slot now takes the last key instead of producing an invalid value; capturing only modifiers gives an assignable chord.
- Action picker: opens on the category of the current value (for example Navigation for `Ctrl+ArrowUp`), an apply error disappears once the value is edited, and file-chooser errors are shown next to the command fields.
- Action picker: macros, commands, KDE shortcuts and mouse buttons can no longer be typed in by hand while they are hidden outside development mode.
- Action picker: the title of the extra-key picker says "Choose key", and long labels or large fonts no longer get clipped in the item grid and buttons.
- Bundled layout "LHC1 by Ivan K": ShiftLeft and ShiftRight use hold detection "Press", so a fast capital letter (Shift released before the letter) types the capital instead of Ctrl+C or "-" followed by a lowercase letter, and the letter appears without waiting for Shift to resolve.
- A rule with a double-tap action whose hold is the key itself (for example Shift with a tap action) treats a second press inside the double-tap window as tap-or-hold, like rules with a layer or a held keystroke: holding it, or pressing another key with hold detection "Press", holds the key (Shift+H types "H" instead of firing the double-tap and a lowercase "h"). The double-tap of such rules now fires when the second press is released.
- Layers reject stale modal edits, preserve keyboard focus when refreshing rows, and reset layer hold mode when its layer is deleted. The page shows activating triggers, explains how extra keys inherit, and displays full key codes in tooltips; empty layouts no longer offer a name editor.

- Triggers reject a switch to a chord while a double-tap action is assigned, show all blocking rule diagnostics, report layer assignment errors, and load the selected rule’s hold options. Existing conditions and long-hold actions remain visible outside development mode.

- Settings save valid fields independently of invalid timing drafts, merge updates to untouched fields, and select the key behavior tab when invalid timings prevent exit. Device choices stay open on write errors and restart the mapper only after a saved device change. Ordinary settings saves no longer scan input devices; hold detection fits narrow windows.

- `text:` actions with `$` or `€` type in Electron apps (Obsidian, VS Code) and X11 apps again: LHC no longer sends them through the special Dollar and Euro keys of the keymap, which these apps ignore. It now uses a regular key of the active layout (Shift+4 for `$` in English) or, when the layout lacks the character, the clipboard fallback.
- `text:` types `*`, `+`, `(`, `)` and `/` with the main keyboard keys (for example Shift+8) instead of keypad keys, which terminals and some apps treat as commands. The keypad is used only for characters missing from the main keys.
- A click on a button or tab while an inline editor was open no longer gets lost: the editor now commits after the click instead of on the press, so the page no longer shifts under the pointer.
- A rule whose tap is set to "Suppress" with nothing on hold now swallows the key completely; before, the original key (for example Enter) still went through.
- A key chord pressed while a tap-hold key is still undecided no longer fires ahead of it: a chord nested inside the held key now uses that key's layer, and a chord rolled after it types in physical order.
- A key with a long-hold action no longer lets the next key overtake it: pressing another key while it is held fires its tap at once (or its long hold, if already due), so for example a space stays in front of the following letter.
- Keys typed while a macro plays no longer land between its steps: they are typed right after the macro (at most 2 s later; a longer macro then lets typing through). Releasing a key pressed before the macro still passes at once.
- A press and release of the same key replayed together (after a tap-hold decision or a chord window) get a 1 ms gap, as synthetic taps already do, so apps that drop zero-length presses no longer lose letters.
- A rule with a hold (layer or held key) and a double-tap action supports tap, then press and hold: the second press activates the hold when held, and fires the double-tap only when released quickly.
- Layouts page: Activate, Edit and Delete on a layout whose file cannot be read no longer act on a previously selected layout, and such a layout can now be renamed and deleted.
- Layouts page: creating a layout no longer asks about unsaved changes twice, and "Save and continue" finishes the switch after resolving a conflict with the file on disk (overwrite, reload or save as new).
- Layouts page: renaming, describing or deleting a layout changed on disk by another program since the page showed it reports the conflict instead of overwriting the file; dialogs no longer show the error of an earlier action; a success notice is no longer replaced by a warning.

- Notifications no longer hide new problems behind older warnings or replay background successes. Popup worker failures are grouped, distinct diagnostics remain separate, repeated rule warnings stay inline, and deleting an automatic switching rule has a confirmation.
- Success and warning notifications disappear on their timeout again; only errors stay until dismissed. Hovering a notification pauses the timeout only while the window is active, and an emptied notification stack no longer leaves the next notifications paused.

- Emoji and Quick actions no longer activate cells or switch pages for Ctrl/Alt/Meta shortcuts, in both winit and Spell popups. Unhandled popup keys are no longer unconditionally consumed.
- Screen readers announce Emoji and Quick actions popup cells as buttons named by their hotkey and content, and the close button as "Close" (winit popups; Spell popups still expose no accessibility tree).
- Emoji page: the "Non-breaking space" palette item works, and leading or trailing spaces of a cell (snippets like `TODO: `) are kept instead of being trimmed.
- Quick actions page: a cell moved to another key or page is validated again, so a cell pointing at a deleted macro or command can no longer be saved by moving it. Labels and page names drop surrounding spaces.
- Emoji and Quick actions pages: deleting the only page is offered as "Clear page", keeps its position, and is disabled when the page is already empty. A save conflict offers "Discard changes"; a warning such as "the mapper still uses the previous settings" is no longer shown as an error, and repeated autosave errors no longer flood the notifications.
- Emoji and Quick actions pages: a cut cell is dimmed until pasted; Escape cancels the cut, and Delete or Backspace clears the focused cell. Screen readers announce empty cells as empty.
- Quick actions page: each assigned cell shows the icon of its action's category (shortcut, macro, command, system action…) instead of the same icon for all.

### Added

- System action `sys:windowMaximizeVertical` stretches the focused window to the full screen height and restores it on the next press: KDE through KWin, GNOME through the extension where Mutter exposes partial maximization, Windows through Win+Shift+Up/Down. The bundled LHC1 layout uses it instead of the KDE shortcut.
- System action `sys:windowToNextDesktop` moves the focused window to the next virtual desktop and follows it (KDE, GNOME). Like `windowToDesktopN`, it is skipped with a notice on Windows and macOS. The bundled LHC1 layout uses it instead of the KDE shortcut.
- The Layers page marks keys and extra keys that keep their modifiers held until the layer is released with an accent dot; the key tooltip explains it.
- Clipboard text input uses a separate paste shortcut in terminals (Konsole, kitty, Alacritty, GNOME Terminal and others, recognised by window class), Ctrl+Shift+V by default: Ctrl+V does not paste there, and many terminals paste the mouse selection on Shift+Insert. Settings → Mapper → Text input can set it to any shortcut or to the one used in other apps.
- Interface language choices and machine-translated catalogs for German, French, international Spanish, Italian, Portuguese, Polish, Turkish, Ukrainian, Czech, Hungarian, Romanian, Greek, Croatian, Georgian, Kazakh, Simplified and Traditional Chinese, Japanese, and Korean.
- Escape cancels a drag in progress. Reorder handles respond to Alt+Up/Down and layer tabs to Alt+Left/Right, so lists can be reordered from the keyboard.
- Dragging a layer tab or a menu page tab scrolls the tab strip at its edges.
- Emoji page: a "Kaomoji" section with Japanese-style text emoticons in the palette.

### Changed

- Improve screen reader names, selection states, page and row structure, status announcements and left/right key identification. Numeric fields expose one editable spinbox with arrow-key steps; inline editors avoid duplicate display buttons. Warnings and errors remain until dismissed, notifications show their full text and pause expiry while focused.

- The settings window fits screens with a large display scale: its minimum size is 640×400 instead of 940×700, it opens no larger than the screen (for example 928×440 at 200% on 1920×1080) and shrinks again when the scale grows. In a narrow window the header shows icons only, with the names in tooltips; the settings tab list and the layer dialog scroll instead of cutting off their buttons.

- Improve placeholder contrast and keep disabled mapper/automatic rules readable while editing. High contrast and E-ink now cover Fluent controls, popup choices, text selection and focus; theme and game-mode choices use the shared keyboard-accessible button group. The held-layer hint follows the output width and stacks hands on narrow screens without reducing text size.

- Shell/core commands use typed mapper configuration and application actions. IPC rejects commands sent to the wrong process and reports UI queue failures; popup worker communication no longer blocks the settings interface. Compound settings/layout edits roll back on failure, and core state notifications are coalesced without discarding application requests.

- Search fields share the text-field clear button, with consistent availability for disabled and read-only fields.

- Library card summaries and input device discovery load in the background. Macro typing updates the edited field immediately and validates the draft after a short pause.

- Numeric editors use the timing limits from the shared core contract; Emoji and Quick actions use the same popup surface and grid. Editor saving now has desktop-independent regression tests, including debounce and invalid drafts.

- Internal APIs distinguish system keyboard layouts, edited layouts and mapper configuration; engine output, rule execution, window conditions and process invocations use explicit names. The action picker represents its unassigned behavior with a single enum. Existing configuration files and CLI/IPC formats remain compatible.
- Confirmation dialogs focus their primary button after the shared frame initializes, so Enter confirms reliably.

- Game mode is off unless the in-development setting is on: detection does not poll, manual On is ignored, and popup hotkeys are no longer blocked by a hidden game mode.
- Game mode counts games from launchers and the Game Bar list only while their window is in front (Settings → Game mode → “Only while the game is in front”, on by default), so switching to a browser turns it off. Detection polls every second only in Auto, a stale Steam `RunningAppID` after a Steam crash no longer keeps game mode on (Windows), and fullscreen detection no longer flips to “unavailable” on an empty desktop (Windows, macOS).
- Settings pages refresh only when their dependent layout sections change; dirty indicators reuse the saved library snapshot until the library is refreshed or edited.
- Macro pause editing follows the same numeric validation as saved layouts. A successful library save completes even when updating the running mapper fails, with a separate warning.

- The settings window looks the same on every page: dropdown menus share one style with a check mark for the current choice and arrow-key highlighting, value fields and pickers use one outline and background, every focusable control shows the same focus ring, and page spacing, tooltips, card titles and empty-list hints are consistent.
- Built-in macros have consistent, accurate names and IDs: for example “Move line down” is now “Empty line above” (`builtin:emptyLineAbove`), “Paste at line bottom” is “Paste on new line below”, “Move up 5 times” is “Move 5 lines up” (`builtin:move5LinesUp`), and “buffer” is “clipboard”. Old `builtin:` IDs are not migrated: bindings that use them show as unknown macros and need to be reassigned.

- All dialogs share one frame: the same width rules, padding, rounded corners, bold title and close button, centred in the window; Escape and a click outside close each of them. The layer view switch on the Layers page uses the shared button group.

- Settings → Key behavior shows “Defaults” as a section heading above its card, with “Tap”, “Hold” and “Macros” as equal groups inside.

- Move paste shortcut explanations into information icons beside their settings labels.
- Errors are reported once, as a toast. The line under the top bar shows only lasting problems such as an unreadable configuration. A failed mapper start no longer also reports “Mapper stopped because of an error”, and the mapper card shows the known cause (for example a disconnected keyboard) instead of the generic failure text.
- Mapper start errors are shorter and more specific: the toast names the cause (disconnected keyboard, no access to the keyboard or /dev/uinput) instead of the generic failure, and the long device-access hint is gone. Choosing another device while the mapper is stopped clears the previous failure. Wrapped cause lines no longer overflow the mapper card in a narrow window.

- Standardize the Triggers header, use full-width settings cards, simplify game mode and device settings, and move quick-action clearing beside the cell heading.

- Application lists in automatic layout rules, trigger rules and game mode share the same heading, add button and list frame.

- Action capture shows held keys and waits until every captured key is released, preserving the full combination regardless of release order.

- Rule headers truncate advanced option summaries before condition labels when the window narrows.

- Keyboard and mouse devices are chosen in a dialog with three tabs: “Keyboards” (or “Mice”) lists the devices detected as such by name, with a note below the list that detection is heuristic; “All devices” lists every input device with its shortened path, virtual devices last, and has a search field; “Enter path” opens with the current path, shows under the field whether the typed path is a connected device and which one, and offers the stable `by-id` path when an `eventN` or other changing link was entered. The footer holds “Refresh”, “Reset”, which clears the choice, and “Close”; on “Enter path” it holds “Reset”, “Cancel” and “Apply”, which is available once the path differs from the current one, and Enter in the field applies too. Names come from the `by-id` link when a device reports none. A saved device that is not connected stays listed and is marked “Not connected”. On Windows, which intercepts every keyboard and mouse of the session, the dialog is replaced by “Intercept all keyboards” and “Intercept buttons of all mice” checkboxes.

- Gate long-hold actions, emoji and quick-action popups, user macros, mouse assignment controls and KDE action selection behind In development features.

- The emoji library offers four language-independent text snippets: "TODO: ", "FIXME: ", "Continue." and "Thanks!".

- When the popup process keeps failing to start, its error is shown once instead of on every retry, and “Popup menus could not recover” appears once when retries stop; the repeats go to the log.

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
- The FemtoVG renderer is no longer built: `SLINT_BACKEND` accepts `winit-skia` (the default, unchanged) and `winit-software`.
- Layer-shell popups no longer write hourly log files to `$XDG_RUNTIME_DIR/spell/` or poll a file there every two seconds; their messages go to the application log (`RUST_LOG=spell_framework=info` to see them). Building no longer needs the libdbus and libpam development packages.

### Fixed

- Keyboard navigation stays inside modal dialogs and returns to their initiator. Device and layer menus activate the focused control, segmented choices retain arrow-key selection, hidden buttons show their focus, and InfoTip help opens with Enter/Space. F1 shows the full focused value and help; menu cells and page reordering, action categories, application rows and scrolling also support the keyboard.

- Interface language selection respects the LANGUAGE preference list and C/POSIX locales. Missing runtime translations display English text; translation checks allow reordered arguments. Settings paths and detection statuses use complete translated messages. Correct Spanish key/action terminology and remove obsolete capture instructions accidentally appended to translations.

- Activating the edited layout no longer panics. Inline editors retain their draft after a failed save, and explicit saves finish inline edits before flushing page drafts. Unreadable library files show an error instead of appearing as empty layouts.

- Keyboard layout notifications include XKB variant and group-index changes even when the layout code and label stay the same. Layout signal watchers stop promptly while waiting to reconnect.
- Command cleanup checks action fields only, so metadata resembling a `cmd:` reference no longer retains unused commands. Clipboard paste acknowledgement ignores reads before arming and non-text transfers.

- Failed library saves preserve the working copy and its commands; suggested copy names fit file-name limits, including UTF-8 names.
- Concurrent starts cannot remove another running instance's IPC socket while recovering a stale socket.

- Opening a library layout keeps the previous working copy and its identity when saving settings fails.
- Incomplete IPC requests have a total deadline and no longer hold up application shutdown while a client slowly sends data.
- Long Unicode layout names are checked against file name byte limits, and atomic saves use short temporary names.

- Mapper startup applies edits made while the device was starting, even when the active layout is unchanged. The mapper card derives its running, busy and error state from one lifecycle snapshot.
- Commands chosen for a key or rule are applied together with their assignment; commands in macro and menu drafts stay pending until the draft is saved. Failed assignments and discarded drafts do not change existing commands.
- Quit keeps the settings window open when a page draft cannot be saved, and reentrant page refreshes are retried instead of being dropped.

- Preserve invalid macro and menu drafts across page updates, merge unrelated menu edits, and flush pending edits before saving or replacing a layout. Ignore file chooser results from closed or changed command editors.
- Load command assignments in the background only when their settings panel is opened, and cache them until relevant configuration changes.

- Mapper restart waits for the previous worker to stop. Text jobs are cancelled when their mapper session exits, including on panic.
- While a Linux keyboard is disconnected, mapper control keeps only the latest configuration and rejects new actions; pending actions are cancelled instead of running after reconnection. Control queues are bounded.
- Slow or panicking core event subscribers no longer interrupt keyboard processing; subscriptions are released when their owner exits.
- Watcher subprocess timeouts close inherited pipes and terminate the process group, with bounded output collection.
- A directory-sync failure after a settings or layout file is published no longer rolls back in-memory settings against an already updated file; the durability failure is logged.

- Closing a layout preserves global settings navigation. Dialogs have header close buttons and Escape dismissal while the application is active.

- Text fields of the application accept Shift+Insert (paste), Ctrl+Insert (copy) and Shift+Delete (cut), so text input with the Shift+Insert paste shortcut works in them too.
- Virtual input devices, such as the ydotoold device, are no longer suggested as keyboards or mice.
- Restored `text:` input into the settings typing test by tracking text destinations separately from rule conditions and identifying popup windows explicitly. Focus cancellation now reports a destination error instead of suggesting a mapper restart.
- Repeated presses of a layer key that keeps its modifiers held (Tab for Alt+Tab) no longer press the held modifier again or wait for the modifier delay, so fast window switching keeps up.
- The “Command started” notification is shown again when a command action runs.
- Pressing a page-bound popup hotkey (`app:showQuickMenuN`, `app:showEmojiMenuN`) again while the popup is open turns through all pages like Tab instead of switching only between the first two.
- Unplugging the optional mouse no longer stops keyboard mapping; the mouse reconnects independently with balanced button releases.
- A tap that waits for a decision or for a possible double tap keeps the modifier that was held when the key was pressed: Ctrl+A on a key with a double-tap action, or Shift+A on a tap-hold key, released after the modifier, no longer types a plain “a”.
- Tap, hold, long-hold and double-tap timers no longer expire while the key's own release or second press is still held back behind another undecided key or a possible chord: a layer key released in time stays a tap, a long-hold key released in time does not run its long action, and a quick second press stays a double tap.
- A chord with a tap-hold key fires only when its keys are pressed within the combo window; a key pressed later while the tap-hold key is held goes to its layer or hold instead.
- Windows: keys released on the secure desktop (Ctrl+Alt+Del, lock screen, UAC) no longer stay held afterwards, so a layer held before it no longer remains active and the next press of such a key is not lost. Auto-repeat keeps its order among key presses and releases.
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
