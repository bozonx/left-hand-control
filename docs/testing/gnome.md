# GNOME Wayland bridge and acceptance

The GNOME backend requires the bundled `left-hand-control@bozonx.dev` Shell
extension. Its metadata targets GNOME Shell 50 (Ubuntu 26.04); other Shell
versions are not claimed. KDE remains the primary accepted environment.

## Installation

Install the application and grant input permissions as described in
[the Linux installation guide](../install-linux.md). From an extracted Linux
bundle, run `./install-gnome-extension.sh`. For an installed archive, use
`~/.local/lib/dev.bozonx.left-hand-control/install-gnome-extension.sh`;
for deb/rpm, use `/usr/share/left-hand-control/install-gnome-extension.sh`.
Log out of GNOME and back in, then run:

```sh
gnome-extensions enable left-hand-control@bozonx.dev
```

Enable Ubuntu AppIndicators (or another StatusNotifier host) for the tray.
Use `LHC_POPUPS=auto` or `winit`; GNOME has no layer-shell protocol.
The application keeps its tray service alive if AppIndicators starts late or
restarts. While the tray host is unavailable it opens settings; recovery
registers the icon again without restarting the mapper or hiding settings.
Remove the extension explicitly with the same script's `uninstall` argument.
Removing/upgrading the application does not silently change GNOME's extension
preferences. Extension updates require another GNOME login.

## Bridge contract

The extension exports `/dev/bozonx/LeftHandControl/Gnome`, interface
`dev.bozonx.LeftHandControl.Gnome`, on the existing `org.gnome.Shell` session-bus
owner. The bus authenticates peers; each method asynchronously verifies the
caller's Unix uid against the Shell's uid. There is no JavaScript evaluation,
shell command execution or arbitrary method dispatch.

- `Snapshot(b titles) -> s JSON`: version 1, focused and previous external window, workspace, configured input
  sources and actual current source (including per-window selection).
- `Windows() -> s JSON`: task windows, requested on demand by the condition picker.
- `Activate(s title)`: focus a mapped window belonging to the authenticated
  caller PID only. The shell requests this off the UI thread for Wayland windows
  without an activation token.
- `Action(s)`: a fixed allowlist of layout/workspace and window actions. Missing
  windows, invalid indices, unsupported actions and locked sessions return a
  D-Bus error, which the mapper reports through its existing action-failure path.
- `Changed`: focus, title/fullscreen/geometry, current source and source-list
  changes. The layout watcher refreshes and wakes the active-window watcher.

Capturing a text target uses the watcher cache without waiting on D-Bus in the
grabbed-keyboard thread. Live destination checks run in the text worker.
Window ids combine a random extension-instance UUID and Mutter's stable window
sequence. Text delivery rechecks the live identity, rather than trusting cached
app id/title. Two identical windows, an extension reload, focus loss, a locked
session or a missing bridge cannot authorize delivery to an unknown target.
Titles are absent unless requested; source changes update the existing core cache
and `CoreEvent` bus. The active-window watcher also has its normal safety reread.

The bridge implements `switchDesktop1..20`, `windowToDesktop1..20`,
`switchLayout1..10`, `windowClose`, `windowMinimize`, `windowMaximize`,
`windowKeepAbove`, `overview`, and `appLauncher`. A numbered workspace/source must
already exist. `showDesktop` and `screenOff` remain unavailable on GNOME.
The extension does not provide a clipboard/data-control protocol or remove portal
permission requirements. In GNOME Wayland, the `libei + clipboard` and clipboard
modes request `org.freedesktop.portal.Clipboard` access on their RemoteDesktop
session before Start. Enable Clipboard Access in the consent dialog to allow
Unicode paste. Pure libei/keycode modes do not request clipboard access.
Keyboard-only and clipboard-capable sessions keep separate opaque restore tokens,
so an older keyboard grant cannot silently restore a session without clipboard
access. Upgrading to clipboard support requests the additional permission once;
a denied clipboard grant is respected.
The worker snapshots the offered MIME data (up to 128 formats / 64 MiB), checks
the live destination during transfer, and restores only while it still owns the
selection. A newer clipboard owner wins. After an input session retires, restored
formats remain served until another owner replaces them; this keeps restoration
from disappearing with the input backend. The earlier
[Ubuntu 26.04 verification](results/linux/2026-10-18-gnome-tray-recovery.txt)
stopped on Unicode with the data-control backend; the Clipboard portal addresses
that capability gap without requiring a data-control protocol. The final
[Clipboard portal verification](results/linux/2026-10-19-gnome-clipboard-portal.txt)
passed 29 VM scenarios plus capability-grant upgrade and explicit Clipboard mode;
its report records commands, artifacts and the remaining acceptance scope.

## Disposable Ubuntu VM

Create a separate Ubuntu 26.04 cloud-image disk, verify its SHA256 against
[Canonical's image checksums](https://cloud-images.ubuntu.com/releases/26.04/release/),
and provision an SSH key and passwordless sudo for `tester` using cloud-init.
Install `ubuntu-desktop-minimal linux-generic zenity python3-gi gir1.2-gtk-4.0
openssh-server desktop-file-utils wl-clipboard fonts-noto-color-emoji
gnome-shell-extension-appindicator`, then run `prepare-gnome-guest.sh tester`
**inside that disposable guest as root**. It enables GDM autologin, disables
SSH password authentication and removes the local account password; do not run it
on a workstation. Reboot, verify a GNOME Wayland session and shut down the guest.
Create `vm.json` as described in [the development guide](../development/guide.md)
with its own port, e.g. 2233, and `name: ubuntu2604-gnome`.

Build the portable package in Ubuntu 22.04 as described in
[the development guide](../development/guide.md), rather than on a newer host
whose glibc may exceed the guest version. Then run:

```sh
python3 apps/slint-shell/scripts/vm/run-gnome.py \
  --vm /path/to/ubuntu2604-gnome \
  --package target/ubuntu-22.04/packages/left-hand-control-0.3.0-linux-x86_64.tar.gz --keep
```

Pass `--examples /path/to/binaries` to also run current `editor --smoke` and
`interactions` executables in the configured guest after installing the bridge.
The runner uses a disposable overlay and stops at the first failed step. It
checks installation, missing input permissions, explicit permissions and relogin,
bridge availability, autostart, tray recovery after host disappearance and
startup without a host (with settings fallback), passthrough, remap, chord, tap/hold layer,
mapper stop/start, system layout/workspace actions, commands, manual game mode,
portal ASCII, Quick popup
focus and delivery, Unicode with text and binary MIME restoration, a mixed macro,
popup IPC, replacement by a newer clipboard owner and cancellation on mapper stop. Portal
consent keys may be configured in `vm.json` as `portal_keys`, or use
`portal_clicks: [[x, y], ...]` with screen fractions for the remote-interaction
switch followed by Share (coordinates depend on the guest resolution). Logs and screenshots
remain in the VM run directory, with a summary under `docs/testing/results/linux/`.
This smoke does not replace acceptance of Emoji selection, repeated focus return,
lock/unlock, suspend, replug, upgrade and multiple receiver applications.
