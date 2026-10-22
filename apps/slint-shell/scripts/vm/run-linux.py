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
import time

from vmlib import Report, StepFailed, Vm, commit, environment_ok, result_path

# scp paths are relative to the guest home; shell commands use ~.
GUEST = "~/lhc-e2e"
GUEST_PY = f"python3 {GUEST}/linux-guest.py"
CONFIG = "~/.config/dev.bozonx.left-hand-control/config.json"
TEXT_UNICODE = "Привет, ёж 😀"  # linux-guest.py TEXT_UNICODE, typed by KeyY


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vm", required=True, help="VM directory with vm.json")
    parser.add_argument("--package", required=True, type=Path,
                        help=".deb, .rpm or .tar.gz built by release-linux.sh")
    parser.add_argument("--previous", type=Path,
                        help="install this older release first and upgrade from it")
    parser.add_argument("--show", action="store_true", help="show the VM display")
    parser.add_argument("--keep", action="store_true", help="keep the overlay disk")
    args = parser.parse_args()
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
    report = Report()
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
        guest(f"write-config --device {shlex.quote(state['device'])}")

    def start_without_permissions():
        guest("start-app")

    def relogin_with_input():
        guest("quit-app")
        if archive:
            # The archive's explicit command: udev rule, uinput and the group.
            vm.ssh(f"{installer} permissions")
        else:
            vm.ssh(f"sudo usermod -aG input {vm.user}")
        # A system unit outlives the ssh session that the logout terminates.
        vm.ssh(f"sudo systemd-run --quiet --on-active=2 --unit=lhc-e2e-relogin "
               f"sh -c 'loginctl terminate-user {vm.user}; systemctl restart display-manager'")
        time.sleep(8)
        vm.wait_ssh()
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
        return guest(f"receiver-result --name {name}", check=False)

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
            time.sleep(3.5)
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
                time.sleep(3)
            vm.qmp.tap("t")
            time.sleep(3)
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
            time.sleep(4)
            vm.qmp.screendump(vm.logs / "portal-consent.png")
            vm.qmp.tap(*vm.get("portal_keys", ["ret"]))
            time.sleep(3)
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
            vm.qmp.tap("ctrl-alt-f11")
            time.sleep(2.5)
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

    def emoji_ipc():
        def show():
            vm.ssh(f"{binary} show emoji")
            time.sleep(2.5)
            vm.qmp.tap("q")
            time.sleep(3)
        return expect("emoji-ipc", "🙂", action=show)

    def quick_rule():
        def show():
            vm.qmp.tap("e")   # app:showQuickMenu1
            time.sleep(2.5)
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
        before = int(guest("suspend-count"))
        vm.ssh("sudo systemd-run --quiet --on-active=1 --unit=lhc-e2e-suspend systemctl suspend")
        # The guest may wake on its own right away; otherwise QMP wakes it.
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if vm.qmp.status() == "suspended":
                time.sleep(3)
                vm.qmp.execute("system_wakeup")
                break
            time.sleep(0.5)
        vm.wait_ssh(timeout=120)
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
        except StepFailed:
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
            guest("start-app")
            guest("mapper --timeout 30")

    def previous_release():
        guest("start-app")
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
        guest("start-app")
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

    def collect():
        vm.copy_from("lhc-e2e/app.log", vm.logs / "app.log")
        vm.copy_from("lhc-e2e/receiver.log", vm.logs / "receiver.log")
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
        report.step("installed-files", check_install)
        report.step("keyboard-probe", probe_keyboard, fatal=True)
        report.step("test-config", write_config, fatal=True)
        if previous:
            report.step("previous-release", previous_release, fatal=True)
            report.step("upgrade-from-previous", upgrade_from_previous, fatal=True)
        report.step("start-without-input", start_without_permissions)
        report.step("tray-item", guest, "tray")
        report.step("no-mapper-without-input", guest, "mapper --absent --timeout 8")
        report.step("autostart-entry", guest, "autostart")
        report.step("relogin-with-input", relogin_with_input, fatal=True)
        report.step("autostart-launch", guest, "wait-app --timeout 90")
        report.step("tray-after-login", guest, "tray")
        report.step("mapper-running", guest, "mapper --timeout 30", fatal=True)
        report.step("portal-deny", portal_deny)
        report.step("portal-consent", portal_approve)
        report.step("mapper-keys-and-text", mapper_text)
        report.step("text-unicode", expect, "text-unicode", TEXT_UNICODE, ["y"], None, 5)
        report.step("layer-tap-hold", layer)
        report.step("macro", expect, "macro", "nmacO", ["m"])
        report.step("command", command)
        report.step("system-actions", system_actions)
        report.step("russian-layout", russian_layout)
        report.step("popups", guest, "popups")
        report.step("emoji-ipc", emoji_ipc)
        report.step("quick-menu-rule", quick_rule)
        report.step("emoji-hotkey", emoji_hotkey)
        report.step("game-mode", game_mode)
        report.step("screen-lock", screen_lock)
        if suspend_exclusion:
            report.skip("suspend-resume", suspend_exclusion)
        else:
            report.step("suspend-resume", suspend)
        report.step("cli", guest, "cli")
        report.step("second-instance", guest, "second-instance")
        report.step("text-after-second-instance", expect, "after-second-instance",
                    TEXT_UNICODE, ["y"], None, 5)
        report.step("keyboard-replug", keyboard_replug)
        report.step("mapper-stop", set_mapper, False)
        report.step("mapper-start", set_mapper, True)
        report.step("stuck-keys-on-stop", stuck_on_stop)
        report.step("stuck-keys-on-kill", stuck_on_kill)
        if archive or vm.get("reinstall"):
            report.step("upgrade-over-running", upgrade)
        else:
            report.skip("upgrade-over-running", "no `reinstall` in vm.json")
        report.step("quit", guest, "quit-app")
        report.step("remove", remove)
    except StepFailed:
        print("A required step failed; later steps were not run.")
    finally:
        try:
            if vm.qemu and vm.qemu.poll() is None:
                vm.qmp.screendump(vm.logs / "final.png")
                collect()
        except (StepFailed, OSError):
            pass
        vm.stop(keep=args.keep)

    header = [
        f"Date: {time.strftime('%F %T %z')}",
        f"Commit: {commit()}",
        f"VM: {vm.name} ({vm.base})",
        f"Memory: {vm.get('memory', 6144)} MiB",
        f"CPU: {vm.get('cpu', 'host')}",
        f"Suspend exclusion: {suspend_exclusion or 'none'}",
        f"Package: {package.name}" + (f" (upgraded from {previous.name})" if previous else ""),
        f"Package SHA256: {package_sha256}",
        "Popup mode: auto (actual backend recorded in app.log)",
        f"Portal deny clicks: {vm.get('portal_deny_click', [0.738, 0.659])}; relative: {vm.get('portal_deny_relative')}",
        f"Previous-version upgrade: {previous.name if previous else 'NOT RUN (no --previous)'}",
        f"Keyboard: {state.get('device', 'not detected')}",
        f"Host: {platform.node()}, {platform.release()}",
        f"Logs and screenshots: {vm.logs}",
    ]
    report.write(header, result_path("linux", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
