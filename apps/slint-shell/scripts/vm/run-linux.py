#!/usr/bin/env python3
"""Linux VM e2e (L4): install a release package into a disposable overlay of a
prepared KDE Plasma VM and check permissions, tray, autostart, the mapper with
real keyboard events from QEMU (layers, macros, commands, system actions, game
mode, lock screen, suspend, replugging, stuck keys), `text:` through the portal,
popups, upgrades and removal.

Usage: run-linux.py --vm /mnt/disk2/vm/lhc-fedora-kde --package <file>
       [--previous <older release>] [--show] [--keep]
See docs/development/guide.md, "E2E в виртуальных машинах".
"""

import argparse
import hashlib
import json
import platform
from pathlib import Path
import shlex
import subprocess
import sys
import time

from text_corpus import TEXT_CORPUS, TEXT_UNICODE

from vmlib import Report, StepFailed, Vm, commit, environment_ok, receiver_text, result_path, wait_for

# scp paths are relative to the guest home; shell commands use ~.
GUEST = "~/lhc-e2e"
GUEST_PY = f"python3 {GUEST}/linux-guest.py"
CONFIG = "~/.config/dev.bozonx.left-hand-control/config.json"


def validate_acceptance(args, suspend_exclusion=None):
    if not args.acceptance:
        return
    if args.stress_seconds < 180:
        raise StepFailed("acceptance requires --stress-seconds >= 180")
    if not args.previous and not args.first_release:
        raise StepFailed("acceptance requires --previous or explicit --first-release")
    if args.previous and args.first_release:
        raise StepFailed("--first-release excludes --previous")
    if suspend_exclusion:
        raise StepFailed("acceptance requires suspend; use an S3-capable VM")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vm", required=True, help="VM directory with vm.json")
    parser.add_argument("--package", required=True, type=Path,
                        help=".deb, .rpm or .tar.gz built by release-linux.sh")
    parser.add_argument("--previous", type=Path,
                        help="install this older release first and upgrade from it")
    parser.add_argument("--popup-mode", choices=("winit", "auto", "spell"), default="winit")
    parser.add_argument("--text-backend", choices=("libei", "clipboard"), default="libei")
    parser.add_argument("--stress-seconds", type=int, default=0, help="optional sustained input/lifecycle/resource check")
    parser.add_argument("--acceptance", action="store_true", help="require every scenario for this backend/VM, including stress and suspend")
    parser.add_argument("--first-release", action="store_true", help="explicit N/A for previous-version upgrade of the first public release")
    parser.add_argument("--show", action="store_true", help="show the VM display")
    parser.add_argument("--keep", action="store_true", help="keep the overlay disk")
    args = parser.parse_args()
    if args.stress_seconds < 0:
        parser.error("--stress-seconds must be nonnegative")
    environment_ok()
    package = args.package.resolve()
    previous = args.previous.resolve() if args.previous else None
    for path in filter(None, (package, previous)):
        if not path.is_file():
            raise SystemExit(f"{path} not found")
    with package.open("rb") as package_file:
        package_sha256 = hashlib.sha256(package_file.read()).hexdigest()
    vm = Vm(args.vm)
    suspend_exclusion = vm.get("suspend_skip_reason")
    if suspend_exclusion is not None and (
        not isinstance(suspend_exclusion, str) or not suspend_exclusion.strip()
    ):
        raise SystemExit("suspend_skip_reason must be a nonempty explanation or omitted")
    if previous and (package.name.endswith(".tar.gz") or not vm.get("install")):
        raise SystemExit("--previous needs a .deb or .rpm and `install` in vm.json")
    try:
        validate_acceptance(args, suspend_exclusion)
    except StepFailed as error:
        parser.error(str(error))
    report = Report()
    required = set("""boot upload plasma-session install installed-files keyboard-probe test-config
        start-without-input profile-loaded tray-item no-mapper-without-input autostart-entry reboot-with-input
        autostart-launch tray-after-login mapper-running portal-deny portal-consent
        mapper-keys-and-text text-unicode text-unicode-corpus layer-tap-hold macro command
        system-actions russian-layout popups emoji-ipc quick-menu-rule emoji-hotkey game-mode
        screen-lock suspend-resume cli second-instance text-after-second-instance keyboard-replug
        mapper-stop mapper-start stuck-keys-on-stop stuck-keys-on-kill sustained-input-lifecycle
        upgrade-over-running quit remove""".split())
    if previous:
        required.remove("install")
        required.update(("install-previous", "previous-release", "upgrade-from-previous"))
    if args.text_backend == "clipboard":
        required.add("clipboard-preservation")
    if args.acceptance:
        report.require(required)
    guest_package = f"{GUEST}/{package.name}"
    state = {}

    def guest(command, **kwargs):
        return vm.ssh(f"{GUEST_PY} {command}", **kwargs).strip()

    def boot():
        vm.start(show=args.show)
        vm.wait_ssh()

    def session():
        return guest("wait-session")

    def upload():
        vm.ssh(f"mkdir -p {GUEST}")
        vm.copy_to(Path(__file__).with_name("linux-guest.py"), "lhc-e2e/")
        vm.copy_to(Path(__file__).with_name("text_corpus.py"), "lhc-e2e/")
        vm.copy_to(package, "lhc-e2e/")
        if previous:
            vm.copy_to(previous, "lhc-e2e/")

    archive = package.name.endswith(".tar.gz")
    installer = "~/.local/lib/dev.bozonx.left-hand-control/install-linux.sh"
    binary = "~/.local/bin/left-hand-control" if archive else "left-hand-control"

    def install(key="install", path=None):
        if archive:
            vm.ssh(f"cd {GUEST} && rm -rf bundle && mkdir bundle && "
                   f"tar -xzf {shlex.quote(package.name)} -C bundle && "
                   f"cd bundle/*/ && ./install-linux.sh")
        else:
            vm.ssh(vm.get(key).format(package=path or guest_package))

    def check_install():
        return guest("check-install" + ("" if archive else " --packaged"))

    def probe_keyboard():
        # The probe listens while the host presses Shift on the QEMU keyboard.
        probe = vm.ssh(f"sudo {GUEST_PY} probe-keyboard --timeout 20", background=True)
        time.sleep(3)
        for _ in range(5):
            vm.qmp.tap("shift")
            time.sleep(0.5)
        out, err = probe.communicate(timeout=30)
        if probe.returncode:
            raise StepFailed(err.strip())
        state["device"] = out.strip()
        return state["device"]

    def write_config():
        guest(f"write-config --device {shlex.quote(state['device'])} --text-backend {args.text_backend} --popup-mode {args.popup_mode}")

    def start_without_permissions():
        guest(f"start-app --popups {args.popup_mode}")

    def reboot_with_input():
        guest("quit-app")
        if archive:
            # The archive's explicit command: udev rule, uinput and the group.
            vm.ssh(f"{installer} permissions")
        else:
            vm.ssh(f"sudo usermod -aG input {vm.user}")
        # A cold session avoids SSH/PAM restarting user@ while logout is
        # still tearing it down. It also checks autostart after a real reboot.
        before = vm.ssh("cat /proc/sys/kernel/random/boot_id").strip()
        vm.ssh("sudo systemd-run --quiet --on-active=2 --timer-property=AccuracySec=100ms --unit=lhc-e2e-permissions-reboot systemctl reboot")
        time.sleep(10)
        def rebooted():
            try:
                return vm.ssh("cat /proc/sys/kernel/random/boot_id", timeout=10).strip() != before
            except (StepFailed, subprocess.SubprocessError, OSError):
                return False
        wait_for("new boot after input-group change", 180, rebooted, interval=2)
        guest("wait-session --group input")
        # Logout terminates SSH sessions: create the persistent connection only
        # after the new desktop is ready, then avoid repeated handshakes.
        vm.ssh_multiplex = True

    def click_point(x, y):
        # A new/moved window can appear under the stationary tablet pointer.
        # Force motion so Wayland updates pointer focus before the click.
        vm.qmp.execute("input-send-event", events=[
            {"type": "abs", "data": {"axis": "x", "value": 0}},
            {"type": "abs", "data": {"axis": "y", "value": 0}}])
        time.sleep(0.2)
        vm.qmp.click(x, y)

    def typed(name, keys=(), action=None, settle=3):
        """Text that `keys` (or `action`) produce in a fresh receiver window."""
        guest(f"start-receiver --name {name}")
        point = json.loads(guest("focus-receiver"))
        click_point(point["x"], point["y"])
        time.sleep(0.3)
        if action:
            action()
        vm.qmp.tap(*keys)
        time.sleep(settle)
        vm.qmp.screendump(vm.logs / f"{name}.png")
        vm.qmp.tap("ret")
        return receiver_text(guest(f"receiver-result --name {name}"))

    def expect(name, expected, keys=(), action=None, settle=3):
        text = typed(name, keys, action, settle)
        if text != expected:
            raise StepFailed(f"expected {expected!r}, received {text!r}; see {name}.png")
        return text

    def failures(reason):
        return int(guest(f"text-failures --reason {shlex.quote(reason)}"))

    def set_mapper(enabled):
        state = "on" if enabled else "off"
        try:
            guest(f"set-mapper {state}")
        except StepFailed as error:
            if "exited with 255:" not in str(error):
                raise
            # SSH may lose the acknowledgement after executing the command.
            # Reassert a desired state rather than toggling it a second time.
            print("SSH disconnected; rechecking the desired mapper state", flush=True)
            guest(f"set-mapper {state}")
            return f"{state}; verified after SSH reconnect"
        return state

    def portal_deny():
        # The first `text:` asks for RemoteDesktop consent. A declined dialog
        # must not hang the mapper or ask again until the mapper restarts.
        before = failures("user cancelled")
        def deny():
            vm.qmp.tap("t")
            wait_for("portal consent dialog", 20, lambda: guest("portal-visible") == "yes")
            vm.qmp.screendump(vm.logs / "portal-deny-dialog.png")
            # A click, not keys: key presses during a pending text job cancel
            # the job instead of recording the declined dialog.
            relative = vm.get("portal_deny_relative")
            points = relative or vm.get("portal_deny_click", [0.738, 0.659])
            # Plasma 6.3 asks a second time after the first Cancel.
            for index, point in enumerate(points if isinstance(points[0], list) else [points]):
                if relative:
                    position = json.loads(guest(f"portal-deny-point --x {float(point[0])} --y {float(point[1])}"))
                    vm.qmp.screendump(vm.logs / f"portal-deny-positioned-{index}.png")
                    click_point(position["x"], position["y"])
                else:
                    vm.qmp.click(*point)
                if index + 1 < len(points if isinstance(points[0], list) else [points]):
                    time.sleep(0.3)
                    wait_for("next portal consent dialog", 15, lambda: guest("portal-visible") == "yes")
            wait_for("declined text job", 15, lambda: failures("user cancelled") > before)
            vm.qmp.tap("t")
            wait_for("cached consent refusal", 15, lambda: failures("user cancelled") >= before + 2)
            if guest("portal-visible") == "yes":
                raise StepFailed("portal asked again after cancellation")
            vm.qmp.screendump(vm.logs / "portal-deny-again.png")
        text = typed("portal-deny", action=deny)
        cancelled = failures("user cancelled") - before
        if text or cancelled < 2:
            raise StepFailed(f"received {text!r}, {cancelled} `user cancelled` failures "
                             "(expected none and 2); see portal-deny-*.png")
        guest("wait-app --timeout 10")
        return "declined; the next text: failed without a new dialog"

    def portal_approve():
        # Starting the mapper again gives the declined backend another chance.
        set_mapper(False)
        set_mapper(True)
        def approve():
            vm.qmp.tap("t")
            wait_for("portal consent dialog", 20, lambda: guest("portal-visible") == "yes")
            vm.qmp.screendump(vm.logs / "portal-consent.png")
            vm.qmp.tap(*vm.get("portal_keys", ["ret"]))
            wait_for("portal consent dismissed", 15, lambda: guest("portal-visible") == "no")
        return expect("portal-consent", "lhc-ok", action=approve)

    def layer():
        down = lambda key: (key, True)
        up = lambda key: (key, False)
        def keys():
            vm.qmp.sequence(
                down("f"), 0.05, up("f"), 0.3,                              # tap: g
                down("f"), 0.4, down("j"), 0.05, up("j"), 0.05, up("f"), 0.3,  # hold: k
                down("f"), 0.03, down("j"), 0.03, up("j"), 0.03, up("f"), 0.3,  # nested: k
                down("f"), 0.03, down("j"), 0.03, up("f"), 0.03, up("j"), 0.3,  # rolling: g j
                down("j"), 0.05, up("j"), 0.3,                              # outside: j
            )
        return expect("layer", "gkkgjj", action=keys)

    def command():
        guest("command-marker reset")
        vm.qmp.tap("n")
        guest("command-marker check")

    def kde_state():
        return dict(item.split("=") for item in guest("kde-state").split())

    def system_actions():
        def check(key, field, value):
            vm.qmp.tap(key)
            deadline = time.monotonic() + 5
            while kde_state()[field] != value:
                if time.monotonic() > deadline:
                    raise StepFailed(f"`{key}` left {field}={kde_state()[field]}, expected {value}")
                time.sleep(0.3)
        if kde_state() != {"desktop": "1", "layout": "0"}:
            raise StepFailed(f"unexpected start state {kde_state()}")
        check("1", "desktop", "2")   # sys:switchDesktop2
        check("2", "desktop", "1")   # kde:kwin/Switch to Desktop 1
        check("3", "layout", "1")    # sys:switchLayout2
        check("4", "layout", "0")    # sys:switchLayout1
        return "desktop 1→2→1, layout us→ru→us"

    def russian_layout():
        vm.qmp.tap("3")
        time.sleep(1)
        try:
            # Physical keys under ru: KeyB is и, KeyC is с; text: is layout-independent.
            return expect("layout-ru", "исlhc-ok" + TEXT_UNICODE, ["a", "c", "t", "y"], settle=5)
        finally:
            vm.qmp.tap("4")

    def foreground_vt():
        return vm.ssh("sudo fgconsole").strip()

    def emoji_hotkey():
        original_vt = foreground_vt()
        def hotkey():
            before = int(guest("popup-count emoji t4_focused"))
            vm.qmp.tap("ctrl-alt-f11")
            guest(f"popup-wait emoji t4_focused --after {before}")
            vm.qmp.screendump(vm.logs / "emoji-hotkey.png")
            state["vt"] = foreground_vt()
            vm.qmp.tap("q")
            time.sleep(3)
        try:
            text = typed("emoji-hotkey", action=hotkey)
        finally:
            if state.get("vt", original_vt) != original_vt:
                vm.ssh(f"sudo chvt {shlex.quote(original_vt)}")
                time.sleep(3)
        if state["vt"] != original_vt:
            raise StepFailed(f"Ctrl+Alt+F11 switched the console to VT{state['vt']}")
        if text != "🙂":
            raise StepFailed(f"expected '🙂', received {text!r}; see emoji-hotkey.png")
        return text

    def popup_hide_focus():
        def show_hide():
            guest("popups")
            vm.qmp.tap("c")
        return expect("popup-hide-focus", "c", action=show_hide)

    def emoji_ipc():
        def show():
            before = int(guest("popup-count emoji t4_focused"))
            vm.ssh(f"{binary} show emoji")
            guest(f"popup-wait emoji t4_focused --after {before}")
            vm.qmp.tap("q")
            time.sleep(3)
        return expect("emoji-ipc", "🙂", action=show)

    def quick_rule():
        def show():
            before = int(guest("popup-count quick t4_focused"))
            vm.qmp.tap("e")   # app:showQuickMenu1
            guest(f"popup-wait quick t4_focused --after {before}")
            vm.qmp.tap("q")
            time.sleep(3)
        return expect("quick-rule", "quick-ok", action=show)

    def game_mode():
        # KeyG types h with game mode on and j with it off.
        expect("game-off", "j", ["g"])
        guest("game on")
        time.sleep(1)
        expect("game-on", "h", ["g"])
        guest("game auto")
        time.sleep(5)
        expect("game-auto-idle", "j", ["g"])
        guest("fake-game start")
        time.sleep(8)
        expect("game-auto-process", "h", ["g"])
        guest("fake-game stop")
        time.sleep(10)
        expect("game-auto-exited", "j", ["g"])
        return "off, on, auto without and with a game process"

    def screen_lock():
        # The password can only be typed through the mapper: q a d c → qbXc.
        vm.ssh(f"echo '{vm.user}:qbXc' | sudo chpasswd")
        def unlock():
            guest("lock-screen")
            time.sleep(4)
            if guest("screen-locked") != "yes":
                raise StepFailed("the screen did not lock")
            vm.qmp.screendump(vm.logs / "screen-locked.png")
            vm.qmp.tap("q", "a", "d", "c", "ret")
            time.sleep(5)
            if guest("screen-locked") != "no":
                vm.qmp.screendump(vm.logs / "screen-unlock-failed.png")
                vm.ssh("sudo loginctl unlock-sessions", check=False)
                raise StepFailed("the password typed through the mapper did not unlock")
            vm.qmp.tap("c")
        # Nothing typed on the lock screen reaches the window behind it.
        return expect("screen-lock", "c", action=unlock)

    def suspend():
        boot_id = vm.ssh("cat /proc/sys/kernel/random/boot_id").strip()
        before = int(guest("suspend-count"))
        vm.ssh("sudo systemd-run --quiet --on-active=1 --timer-property=AccuracySec=100ms --unit=lhc-e2e-suspend systemctl suspend")
        # The guest may wake on its own right away; otherwise QMP wakes it.
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if vm.qmp.status() == "suspended":
                time.sleep(3)
                vm.qmp.execute("system_wakeup")
                break
            time.sleep(0.5)
        vm.wait_ssh(timeout=120)
        if vm.ssh("cat /proc/sys/kernel/random/boot_id").strip() != boot_id:
            raise StepFailed("the guest rebooted instead of resuming")
        if int(guest("suspend-count")) <= before:
            raise StepFailed("the guest did not complete a suspend cycle")
        time.sleep(3)
        guest("mapper --timeout 20")
        return expect("after-resume", "bXclhc-ok", ["a", "d", "c", "t"])

    def keyboard_replug():
        vm.qmp.replug_keyboard()
        time.sleep(2)
        try:
            guest("mapper --timeout 20")
            return expect("after-replug", "bXc" + TEXT_UNICODE, ["a", "d", "c", "y"], settle=5)
        except StepFailed as error:
            failure = str(error)
        observed = typed("replug-followup", ["a", "d", "c"])
        # A failed text assertion does not mean the mapper stopped. Do not
        # invert its state and invalidate every subsequent lifecycle scenario.
        recovery = "mapper still running"
        try:
            guest("mapper --timeout 1")
        except (StepFailed, subprocess.SubprocessError, OSError):
            set_mapper(True)
            guest("mapper --timeout 20")
            recovery = "mapper restarted"
        raise StepFailed(f"after replug: {failure}; follow-up input: {observed!r}; {recovery}")

    def stuck_on_stop():
        def stop_held():
            vm.qmp.press("shift")
            vm.qmp.press("f")
            time.sleep(0.5)
            try:
                set_mapper(False)
                time.sleep(1)
            finally:
                vm.qmp.release("f")
                vm.qmp.release("shift")
            time.sleep(0.5)
            vm.qmp.tap("a", "b", "c")
        try:
            return expect("stuck-stop", "abc", action=stop_held)
        finally:
            set_mapper(True)

    def stuck_on_kill():
        def kill_held():
            vm.qmp.press("shift")
            time.sleep(0.5)
            guest("kill-app")
            time.sleep(2)
            vm.qmp.release("shift")
            time.sleep(0.5)
            vm.qmp.tap("a", "b", "c")
        try:
            return expect("stuck-kill", "abc", action=kill_held)
        finally:
            guest(f"start-app --popups {args.popup_mode}")
            guest("mapper --timeout 30")

    def previous_release():
        guest(f"start-app --popups {args.popup_mode}")
        guest("quit-app")
        state["previous_config"] = vm.ssh(f"sha256sum {CONFIG}")
        return vm.ssh(f"{binary} --version").strip()

    def upgrade_from_previous():
        before = vm.ssh(f"{binary} --version").strip()
        vm.ssh(vm.get("install").format(package=guest_package))
        after = vm.ssh(f"{binary} --version").strip()
        if after == before:
            raise StepFailed(f"the version did not change: {after}")
        if vm.ssh(f"sha256sum {CONFIG}") != state["previous_config"]:
            raise StepFailed("the upgrade changed the user configuration")
        guest(f"start-app --popups {args.popup_mode}")
        guest("quit-app")
        return f"{before} → {after}, configuration kept"

    def mapper_text():
        return expect("mapper-keys-and-text", "bXclhc-ok", ["a", "d", "c", "t"])

    def upgrade():
        install("reinstall")
        guest("wait-app")
        return "reinstalled over the running instance"

    def remove():
        guest("quit-app", check=False)
        if archive:
            vm.ssh(f"{installer} uninstall")
        else:
            vm.ssh(vm.get("remove"))
        guest("check-removed")

    def clipboard_preservation():
        seed = "lhc-clipboard-before-Привет"
        guest(f"clipboard --value {shlex.quote(seed)}")
        expect("clipboard-unicode", TEXT_UNICODE, ["y"], None, 5)
        if guest("clipboard") != seed:
            raise StepFailed("text injection did not restore the previous clipboard text")
        return "exact Unicode; previous clipboard text restored after paste"

    def stress():
        deadline = time.monotonic() + args.stress_seconds
        samples, cycles, grants = [], 0, 0
        while time.monotonic() < deadline or cycles < 3:
            def input_cycle():
                nonlocal grants
                vm.qmp.tap("a", "d", "c", "t")
                time.sleep(3)
                if guest("portal-visible") == "yes":
                    grants += 1
                    vm.qmp.tap(*vm.get("portal_keys", ["ret"]))
                    time.sleep(3)
            expect(f"stress-{cycles}", "bXclhc-ok", action=input_cycle)
            set_mapper(False)
            set_mapper(True)
            if cycles and cycles % 5 == 0:
                keyboard_replug()
            guest("popups")
            samples.append(json.loads(guest("resources")))
            cycles += 1
        (vm.logs / "stress-resources.json").write_text(json.dumps(samples, indent=2))
        warm = samples[1:]
        if len(warm) > 1:
            for field, limit in (("rss_kib", 65536), ("fds", 16), ("threads", 8)):
                if warm[-1][field] - warm[0][field] > limit:
                    raise StepFailed(f"unbounded {field} growth: {samples}")
        return f"{cycles} exact input/popup/stop/start cycles over at least {args.stress_seconds}s; resources bounded; portal grants={grants}"

    def isolated(name, scenario):
        def prepare():
            vm.qmp.release_all()
            if guest("screen-locked") == "yes":
                vm.ssh("sudo loginctl unlock-sessions")
                wait_for("baseline unlocked desktop", 15, lambda: guest("screen-locked") == "no")
            guest("wait-app")
            guest("fake-game stop")
            guest("game off")
            guest("popups")
            set_mapper(True)
            vm.qmp.tap("2", "4")
            wait_for("baseline desktop/layout", 10,
                     lambda: kde_state() == {"desktop": "1", "layout": "0"})
            expect(name + "-baseline", "bXc", ["a", "d", "c"])
        ready = "prepare-" + name
        report.step(ready, prepare)
        return report.step(name, scenario, requires=(ready,))

    def collect():
        for name in ("app.log", "receiver.log", "metrics.csv", "metrics.csv.spell.csv"):
            if not vm.copy_from("lhc-e2e/" + name, vm.logs / name):
                report.skip("collect-" + name, "file unavailable")
        (vm.logs / "journal.txt").write_text(
            vm.ssh("journalctl --user -b --no-pager -n 2000", check=False, timeout=30), encoding="utf-8")

    try:
        report.step("boot", boot, fatal=True)
        report.step("upload", upload, fatal=True)
        report.step("plasma-session", session, fatal=True)
        if previous:
            report.step("install-previous", install, "install",
                        f"{GUEST}/{previous.name}", fatal=True)
        else:
            report.step("install", install, fatal=True)
        report.step("installed-files", check_install, fatal=True)
        report.step("keyboard-probe", probe_keyboard, fatal=True)
        report.step("test-config", write_config, fatal=True)
        if not previous:
            report.skip("upgrade-from-previous", "N/A: first public release" if args.first_release else "NOT RUN: no --previous")
        if previous:
            report.step("previous-release", previous_release, fatal=True)
            report.step("upgrade-from-previous", upgrade_from_previous, fatal=True)
        report.step("start-without-input", start_without_permissions)
        report.step("profile-loaded", guest, f"check-profile --device {shlex.quote(state['device'])}", fatal=True, requires=("start-without-input",))
        report.step("tray-item", guest, "tray")
        report.step("no-mapper-without-input", guest, "mapper --absent --timeout 8")
        report.step("autostart-entry", guest, "autostart")
        report.step("reboot-with-input", reboot_with_input, fatal=True)
        report.step("autostart-launch", guest, "wait-app --timeout 90")
        report.step("tray-after-login", guest, "tray")
        report.step("mapper-running", guest, "mapper --timeout 30", fatal=True)
        # Clipboard mode still needs RemoteDesktop consent for its paste shortcut.
        report.step("portal-deny", portal_deny)
        report.step("portal-consent", portal_approve)
        if args.text_backend == "clipboard":
            report.step("clipboard-preservation", clipboard_preservation)
        report.step("mapper-keys-and-text", mapper_text)
        report.step("text-unicode", expect, "text-unicode", TEXT_UNICODE, ["y"], None, 5)
        report.step("text-unicode-corpus", expect, "text-corpus", TEXT_CORPUS, ["u"], None, 5)
        report.step("layer-tap-hold", layer)
        report.step("macro", expect, "macro", "nmacO", ["m"])
        report.step("command", command)
        report.step("system-actions", system_actions)
        report.step("russian-layout", russian_layout)
        report.step("popups", popup_hide_focus)
        report.step("emoji-ipc", emoji_ipc)
        report.step("quick-menu-rule", quick_rule)
        report.step("emoji-hotkey", emoji_hotkey)
        report.step("game-mode", game_mode)
        isolated("screen-lock", screen_lock)
        if suspend_exclusion:
            report.skip("suspend-resume", suspend_exclusion)
        else:
            isolated("suspend-resume", suspend)
        report.step("cli", guest, "cli")
        report.step("second-instance", guest, "second-instance")
        report.step("text-after-second-instance", expect, "after-second-instance",
                    TEXT_UNICODE, ["y"], None, 5)
        isolated("keyboard-replug", keyboard_replug)
        report.step("mapper-stop", set_mapper, False)
        report.step("mapper-start", set_mapper, True, requires=("mapper-stop",))
        isolated("stuck-keys-on-stop", stuck_on_stop)
        isolated("stuck-keys-on-kill", stuck_on_kill)
        if args.stress_seconds:
            isolated("sustained-input-lifecycle", stress)
        else:
            report.skip("sustained-input-lifecycle", "not requested; use --stress-seconds >= 180")
        if archive or vm.get("reinstall"):
            report.step("upgrade-over-running", upgrade)
        else:
            report.skip("upgrade-over-running", "no `reinstall` in vm.json")
        report.step("quit", guest, "quit-app")
        report.step("remove", remove)
    except (StepFailed, subprocess.SubprocessError, OSError):
        print("A required step failed; later steps were not run.")
    except KeyboardInterrupt:
        report.steps.append(("controller", "FAILED", "interrupted"))
    finally:
        try:
            if vm.qemu and vm.qemu.poll() is None:
                vm.qmp.screendump(vm.logs / "final.png")
                collect()
        except (StepFailed, subprocess.SubprocessError, OSError, ValueError) as error:
            report.skip("collect-diagnostics", str(error))
        vm.stop(keep=args.keep or report.unsuccessful)

    header = [
        f"Date: {time.strftime('%F %T %z')}",
        f"Commit: {commit()}",
        f"Command: {shlex.join(sys.argv)}",
        f"VM: {vm.name} ({vm.base})",
        f"Memory: {vm.get('memory', 6144)} MiB",
        f"CPU: {vm.get('cpu', 'host')}",
        f"Machine: {vm.machine}",
        f"Acceptance requested: {args.acceptance}; scope: this VM/backend combination only",
        f"Text backend: {args.text_backend}; stress seconds: {args.stress_seconds}",
        f"Suspend exclusion: {suspend_exclusion or 'none'}",
        f"Package: {package.name}" + (f" (upgraded from {previous.name})" if previous else ""),
        f"Package SHA256: {package_sha256}",
        f"Popup mode: {args.popup_mode} (startup environment and worker processes recorded in resources)",
        f"Portal deny clicks: {vm.get('portal_deny_click', [0.738, 0.659])}; relative: {vm.get('portal_deny_relative')}",
        f"Previous-version upgrade: {previous.name if previous else 'N/A: first public release' if args.first_release else 'NOT RUN (no --previous)'}",
        f"Keyboard: {state.get('device', 'not detected')}",
        f"Host: {platform.node()}, {platform.release()}",
        f"Logs and screenshots: {vm.logs}",
    ]
    report.write(header, result_path("linux", vm.name))
    raise SystemExit(1 if report.unsuccessful else 0)


if __name__ == "__main__":
    main()
