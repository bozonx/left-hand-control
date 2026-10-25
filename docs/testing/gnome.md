# GNOME Wayland bridge and acceptance

The GNOME backend requires the bundled `left-hand-control@bozonx.dev` Shell
extension. Its metadata targets GNOME Shell 50 (Ubuntu 26.04); other Shell
versions are not claimed. KDE remains the primary accepted environment.

## Объём проверки и запуск

Проверяемая desktop-среда — **Ubuntu 26.04, GNOME Shell 50, Wayland**,
с расширением `left-hand-control@bozonx.dev`, AppIndicators и portal.
Это отдельная smoke-матрица, а не полная KDE L4-приёмка.
[Прогон 2026-10-19](results/linux/2026-10-19-vm-ubuntu2604-gnome-2.txt)
прошёл 29 шагов без ошибок; [проверка clipboard/portal](results/linux/2026-10-19-gnome-clipboard-portal.txt)
уточняет покрытие clipboard grants и оставшиеся ограничения.
Свидетельства относятся к указанным исходникам, не автоматически к текущему HEAD.

### Предварительные условия

Linux-хост с KVM/QEMU, Python, SSH/SCP; отдельная Ubuntu VM с GNOME Wayland,
автологином и тестовым пользователем. Расширение нужно включить после установки
и нового входа в GNOME; для трея нужен AppIndicators. Подготовка VM и установка
расширения описаны ниже в [Installation](#installation) и
[Disposable Ubuntu VM](#disposable-ubuntu-vm).
GNOME не поддерживает layer-shell: использовать `LHC_POPUPS=auto` либо `winit`.
Пакет собирать по инструкции совместимой Linux-сборки в руководстве разработчика.

### Как запустить

Сначала [общие Linux-проверки](common.md), затем:

```sh
python3 apps/slint-shell/scripts/vm/run-gnome.py --vm /path/to/ubuntu2604-gnome --package /path/to/left-hand-control-VERSION-linux-x86_64.tar.gz
```

Для текущих UI-примеров добавить `--examples /path/to/current-binaries` с бинарниками
`editor` и `interactions`, собранными из проверяемых исходников. Без этого параметра
UI-примеры не выполняются. `--keep` сохраняет overlay. Параметр `--show` этот runner
не предоставляет. Не заменять GNOME-команду на `run-linux.py`, рассчитанный на Plasma.

### Что проверяется

Установка, отсутствие grab без прав, выдача прав и перелогин, GNOME bridge,
автозапуск, трей и восстановление после исчезновения/появления tray host;
passthrough/remap/chord/tap-hold layer, stop/restart, системные раскладки/рабочие
столы, команда и ручной game mode; portal ASCII и Unicode, Quick popup с возвратом
фокуса, смешанный макрос, popup open/hide, восстановление text/binary MIME,
новый владелец clipboard и отмена на mapper stop.
Runner останавливается на первой ошибке: оставшиеся шаги не пройдены.
Полный перечень — [run-gnome.py](../../apps/slint-shell/scripts/vm/run-gnome.py).

### Что не проверяется полностью

Emoji selection, многократный возврат фокуса, другие приложения-приёмники,
lock/unlock, suspend, replug, физическое железо и полное обновление deb/rpm между
версиями не закрыты этим smoke. Точечная проверка обновления архива не заменяет
полный package upgrade. Не заявлена матрица других версий GNOME/дистрибутивов.
`showDesktop` и `screenOff` на GNOME недоступны; это граница backend, а не PASS теста.

### Результаты

Сводки создаются в `docs/testing/results/linux/`, логи, journal и снимки —
`<vm>/runs/<timestamp>/logs`. Версию расширения, portal consent, clipboard grants,
popup backend и `--examples` записывать явно. Правила хранения —
[results/README.md](results/README.md).

## Подготовка и контракт GNOME bridge

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
`windowMaximizeVertical`, `windowKeepAbove`, `overview`, and `appLauncher`. A numbered
workspace/source must already exist. `windowMaximizeVertical` needs Mutter's public
`get/set_maximize_flags`; without them the extension reports it unsupported. `showDesktop` and `screenOff` remain unavailable on GNOME.
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
