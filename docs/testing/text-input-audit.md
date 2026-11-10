# Text input audit follow-up

Implementation follow-up to the static Ctrl+V / Shift+Insert / clipboard / popup
audit. This records what changed and what remains a platform or policy limitation;
it is not a claim of native acceptance on every desktop.

| Finding | Resolution / remaining limit |
| --- | --- |
| Large/slow all-MIME snapshots block paste | Removed speculative popup snapshots; preserve all formats or fail before ownership changes, with a specific recovery notification. The existing 64 MiB / 128 MIME / 2 second limits remain. Partial restoration would discard data and is not enabled. |
| Popup source re-encodes images before a selection | Popup opening no longer reads clipboard contents. `warm()` only connects/synchronizes. |
| Linux focus restoration race | Wait before the first destination check, including tool modes. The portal worker's existing later wait cannot fix a failure at the initial check. |
| GNOME idle wakes every 5 ms | Register a signal waker and park the owner until a portal signal, command or sender shutdown. Private-bus regression test covers a retired owner waking after replacement. Active transactions still poll within their bounded deadline. |
| GNOME retains RemoteDesktop session | Required by its clipboard portal while LHC serves restored formats. Remains until another owner replaces the selection. Closing immediately can erase the clipboard; no silent policy change. A replacement backend can temporarily overlap the retired owner. The [GNOME integration decision](../design/gnome-clipboard.md) records why a Shell/native shim was rejected. |
| Failed data-control ACK skips restoration | Cleanup now runs after errors in preparation/injection and attempts restoration even when the final ACK fails. A broken compositor connection still cannot guarantee preservation. |
| Unknown destination rejects every text | Unknown Linux targets use lifetime/modifier checks and the main shortcut; identity enforcement is unavailable. Known KDE/GNOME ids remain checked. |
| Embedded terminals use the host class | Documented in settings and features; recommend main Ctrl+V to avoid PRIMARY. There is no reliable inner-widget detector. Existing explicitly selected shortcuts are retained. |
| xterm/urxvt terminal defaults | Document custom CLIPBOARD bindings. No X11 extension or assumption that every terminal supports Ctrl+Shift+V. |
| Terminal flag and target use different caches | Capture terminal metadata with the KWin/GNOME destination snapshot; requests retain that shared snapshot. |
| wlroots title changes cancel typing | Metadata fallback ignores mutable title and compares app/process. This cannot distinguish two windows of the same application; native stable-id coverage outside KDE/GNOME remains limited. |
| GNOME lacks reader grace | Both transports use the same 200 ms grace policy. Neither can identify the reader. |
| GNOME temporary text appears in history | Add the same `x-kde-passwordManagerHint` MIME hint as data-control. Managers that ignore it may still record text. |
| Data-control serves cancelled requests | Check the request before ownership/injection and during each outgoing transfer write. |
| Different paste deadlines | Share prepare/arm/finish timeout constants; GNOME transaction now uses the shared paste timeout. |
| GNOME incorrectly warms data-control | Removed clipboard warm-up from popup opening entirely. |
| Clipboard request failure disables libei | Clipboard setup is optional for keyboard input; GNOME clipboard paste reports missing permission/interface directly. Failed clipboard subscription cannot close a keyboard-only session. |
| No permission reset | Add Settings → Text input → Reset text input permissions; stop keyboard backends, clear both tokens and retry on the next input. A denied clipboard grant is not persisted. |
| Modifiers change during preparation | Wait again before the paste shortcut. libei also refreshes and checks compositor depressed/latched modifiers after preparation. |
| Virtual modifiers in keycode mode | Physical-state scan includes ungrabbed devices, including the mapper's uinput keyboard. Do not inject while its modifiers are held. Native portal acceptance with a held layer remains in the checklist below. |
| Password manager loses auto-clear ownership | Reject source selections advertising the sensitive MIME hint before snapshot/ownership. Unmarked secrets cannot be identified. |
| Saved data retained unnecessarily | Remove speculative snapshot cache; clear restored data on ownership cancellation; portal snapshot copies share MIME bytes rather than duplicating them. Restored bytes must remain while LHC serves them. |
| Quit can erase clipboard / restored history duplicates | Documented protocol/lifetime limits. Original ownership cannot be transferred back by either transport. |
| Built-in macros use physical C/V/X and Ctrl+C in terminals | Resolve built-in Linux clipboard strokes at playback, follow paste settings and current/other configured layout groups, add Shift for terminal copy/cut. Cache layout groups on the watcher rather than query D-Bus from the input thread. Terminal cut/navigation/selection still depend on the application. |
| User Ctrl+KeyC/V/X on Dvorak/Colemak | These are explicit physical-key bindings and remain so. Use built-in editing macros for contextual clipboard operations. No action-string format change. |
| Native popup errors only in logs | Windows/macOS adapters now emit `CoreEvent::TextInputFailed` for missing target, focus, modifier, startup and native injection failures. |
| Different native/Linux popup lifecycle | Native popup insertion remains independent of the mapper; Linux uses the mapper queue. macOS has no mapper implementation. Unifying those lifecycles is a separate architecture change. |
| Windows UIPI silently loses input | The existing core `SendInput` helper already rejects partial/zero delivery; popup failures now reach notifications. UIPI/secure desktop restrictions are not bypassed. |
| macOS text/layout | Enigo's text path remains, with foreground/modifier checks before each character and visible errors. Requires native acceptance; no macOS mapper added. |
| X11 default clipboard capability | Document explicitly that this clipboard backend requires Wayland, with explicit xdotool selection for X11. No X11 support was extended. |
| Stale mode names and GNOME documentation | Update features, platform matrix and GNOME test guide with current mode ids, portal access, ownership, limits and focus guarantees. |
| Generic errors | Distinguish snapshot limits/slow owner, portal permission, missing text consumer and sensitive source; keep technical details diagnostic and translate all new strings in all 20 catalogs. |

## Verification

The implementation passed workspace tests, Clippy, formatting, the non-Spell
build, dependency policy checks, the editor/interactions/small-window UI scenarios,
the isolated KWin clipboard test and all ten mapper e2e scenarios. A disposable
GNOME 50.1 smoke also passed real portal Unicode insertion and byte-for-byte text
and custom binary MIME restoration. This does not replace the broader matrix below.

## Acceptance still needed

Use temporary config/data and disposable receivers, never a real password or the
user's clipboard for an automated test.

- KDE Wayland: Spectacle/Gwenview 4K image in multiple MIME encodings and a slow
  LibreOffice offer; verify a failed snapshot leaves the full original offer.
- KWin ACK fault injection with a live connection, plus a real disconnect: verify
  cleanup attempts and the reported limitation instead of claiming guaranteed
  recovery after connection loss.
- GNOME VM: Clipboard interface absent; consent denied then reset and granted;
  libei Latin text works without Clipboard permission; emoji reports the specific
  access error; no timer-driven idle wakeups; replacing a restored selection closes
  its retired session. Observe any remote-control indicator during overlap.
- KDE/GNOME: delay popup focus return, switch to a different window during a text
  transfer, hold a mapper Ctrl layer, press a physical shortcut during snapshot,
  and paste after cancellation. No incorrect modified paste shortcut should escape.
- Embedded Kate/Dolphin/IDE terminals with deliberately different PRIMARY and
  CLIPBOARD contents, explicit normal/terminal shortcuts, legacy terminal bindings.
- Dvorak/Colemak and a non-Latin active group with a Dvorak Latin group: built-in
  copy/paste/cut versus explicitly physical user chords. Terminal cut/selection
  needs application support.
- Sensitive-marked source auto-clear, new copies during cleanup, LHC quit, history
  manager behavior and unmarked secret limits.
- Windows: regular/elevated receiver, rejected SendInput, popup focus change and
  visible failure notification. macOS: permission denial, native Unicode with
  several layouts, focus changes and notifications.

Executed commands and results are recorded in
[the implementation verification report](results/linux/2026-11-10-text-input-audit.txt).
