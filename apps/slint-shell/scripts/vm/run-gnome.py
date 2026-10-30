#!/usr/bin/env python3
"""GNOME Wayland acceptance in a disposable VM; stop at the first failed step.

Usage: run-gnome.py --vm <prepared Ubuntu GNOME directory> --package <tar.gz> [--keep]
"""

import argparse
import ast
import json
from pathlib import Path
import shlex
import subprocess
import time

from vmlib import Report, StepFailed, Vm, commit, environment_ok, receiver_text, result_path

GUEST = "~/lhc-e2e"
PY = f"python3 {GUEST}/linux-guest.py"
INSTALL = "~/.local/lib/dev.bozonx.left-hand-control"
UUID = "left-hand-control@bozonx.dev"
BRIDGE = "gdbus call --session --dest org.gnome.Shell --object-path /dev/bozonx/LeftHandControl/Gnome --method dev.bozonx.LeftHandControl.Gnome."


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vm", required=True)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--keep", action="store_true")
    parser.add_argument("--examples", type=Path, help="directory with current editor and interactions binaries")
    args = parser.parse_args()
    environment_ok()
    package = args.package.resolve()
    if not package.is_file() or not package.name.endswith(".tar.gz"):
        parser.error("--package must be an existing release .tar.gz")
    if args.examples and any(not (args.examples / name).is_file() for name in ("editor", "interactions")):
        parser.error("--examples needs both editor and interactions binaries")
    vm = Vm(args.vm)
    report = Report()
    state = {}

    def step(name, function, *arguments):
        return report.step(name, function, *arguments, fatal=True)

    def guest(command, **kwargs):
        return vm.ssh(f"{PY} {command}", **kwargs).strip()

    def boot():
        vm.start()
        vm.wait_ssh()
        vm.ssh(f"mkdir -p {GUEST}")
        vm.copy_to(Path(__file__).with_name("linux-guest.py"), "lhc-e2e/")
        vm.copy_to(Path(__file__).with_name("text_corpus.py"), "lhc-e2e/")
        guest("wait-session --timeout 180")
        info = vm.ssh("cat /etc/os-release; gnome-shell --version; "
                      "systemctl --user show-environment | grep -E 'XDG_CURRENT_DESKTOP|XDG_SESSION_TYPE'; "
                      "dpkg-query -W xdg-desktop-portal xdg-desktop-portal-gnome")
        (vm.logs / "environment.txt").write_text(info)
        return info.strip()

    def ui_example(name):
        vm.copy_to(args.examples / name, "lhc-e2e/")
        arguments = " --smoke" if name == "editor" else ""
        home = vm.ssh('printf %s "$HOME"').strip()
        snapshots = f"{home}/lhc-e2e/ui-snapshots/{name}"
        vm.ssh(f"mkdir -p {shlex.quote(snapshots)}")
        return vm.ssh(f"systemd-run --user --wait --pipe --collect --unit=lhc-e2e-ui-{name} "
                      f"--setenv=LHC_EDITOR_SNAPSHOTS={shlex.quote(snapshots)} "
                      f"{GUEST}/{name}{arguments}", timeout=180).strip()

    def install():
        vm.copy_to(package, "lhc-e2e/")
        vm.ssh(f"cd {GUEST} && mkdir bundle && tar -xzf {shlex.quote(package.name)} -C bundle && "
               "cd bundle/* && ./install-linux.sh && ./install-gnome-extension.sh")
        return guest("check-install")

    def keyboard():
        probe = vm.ssh(f"sudo {PY} probe-keyboard --timeout 20", background=True)
        time.sleep(2)
        for _ in range(4):
            vm.qmp.tap("shift")
            time.sleep(0.5)
        out, err = probe.communicate(timeout=25)
        if probe.returncode:
            raise StepFailed(err.strip())
        state["device"] = out.strip()
        guest(f"write-config --device {shlex.quote(state['device'])}")
        return state["device"]

    def permissions():
        vm.ssh(f"{INSTALL}/install-linux.sh permissions")
        # Preserve Ubuntu's installed extensions; enable the bridge after a fresh login.
        script = ("from gi.repository import Gio; s=Gio.Settings.new('org.gnome.shell'); "
                  "v=list(s.get_strv('enabled-extensions')); "
                  f"s.set_strv('enabled-extensions',list(dict.fromkeys(v+['{UUID}','ubuntu-appindicators@ubuntu.com']))); "
                  f"s.set_strv('disabled-extensions',[x for x in s.get_strv('disabled-extensions') if x!='{UUID}']); "
                  "Gio.Settings.sync()")
        vm.ssh(f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus python3 -c {shlex.quote(script)}")
        vm.ssh("sudo systemd-run --quiet --on-active=2 --unit=lhc-e2e-relogin "
               f"sh -c 'systemctl stop display-manager; loginctl terminate-user {shlex.quote(vm.user)}; sleep 3; systemctl start display-manager'")
        time.sleep(8)
        vm.wait_ssh()
        # An SSH login can succeed while terminate-user is still draining the old
        # session. Retry transport loss until the new graphical session is ready.
        session_deadline = time.monotonic() + 180
        while True:
            try:
                guest("wait-session --group input --timeout 15")
                break
            except (StepFailed, subprocess.TimeoutExpired):
                if time.monotonic() >= session_deadline:
                    raise
                time.sleep(2)
        deadline = time.monotonic() + 30
        while True:
            result = vm.ssh(f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus {BRIDGE}Snapshot false", check=False).strip()
            if result:
                return result
            if time.monotonic() >= deadline:
                raise StepFailed("GNOME bridge did not become available after login")
            time.sleep(0.5)

    def tray_recovery():
        def wait_for(description, predicate):
            deadline = time.monotonic() + 30
            while not predicate():
                if time.monotonic() >= deadline:
                    raise StepFailed(f"timed out waiting for {description}")
                time.sleep(0.3)

        def disable_host():
            vm.ssh("gnome-extensions disable ubuntu-appindicators@ubuntu.com")
            wait_for("tray host to disappear", lambda: "false" in vm.ssh(
                "busctl --user call org.freedesktop.DBus /org/freedesktop/DBus "
                "org.freedesktop.DBus NameHasOwner s org.kde.StatusNotifierWatcher"))

        def settings_focused():
            output = vm.ssh(f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus {BRIDGE}Snapshot true")
            window = json.loads(ast.literal_eval(output.strip())[0])["window"]
            return window and window["appId"] == "dev.bozonx.left-hand-control"

        def restore_host():
            pid = vm.ssh("pgrep -u $(id -u) -x left-hand-contr").strip()
            vm.ssh("gnome-extensions enable ubuntu-appindicators@ubuntu.com")
            guest("tray")
            if vm.ssh("pgrep -u $(id -u) -x left-hand-contr").strip() != pid:
                raise StepFailed("tray recovery restarted the application")

        vm.ssh("~/.local/bin/left-hand-control hide")
        disable_host()
        wait_for("settings fallback after tray host loss", settings_focused)
        vm.qmp.screendump(vm.logs / "tray-host-absent.png")
        restore_host()
        disable_host()
        guest("quit-app")
        guest("start-app --popups auto")
        # Leave the host unavailable: the settings fallback must work before
        # restoring it, rather than depending on a successful registration.
        time.sleep(3)
        wait_for("settings fallback at startup without a tray host", settings_focused)
        restore_host()
        return "host loss/reappearance and startup without host: settings reachable; same-process registration recovered"

    def expect(name, expected, keys=(), action=None):
        guest(f"start-receiver --name {name}")
        if action:
            action()
        if keys:
            vm.qmp.tap(*keys)
        time.sleep(3)
        vm.qmp.screendump(vm.logs / f"{name}.png")
        vm.qmp.tap("ret")
        result = receiver_text(guest(f"receiver-result --name {name}"))
        if result != expected:
            raise StepFailed(f"expected {expected!r}, received {result!r}; see {name}.png and app.log")
        return repr(result)

    def layer():
        def keys():
            vm.qmp.tap("f")
            vm.qmp.sequence(("f", True), 0.4, ("j", True), 0.05,
                            ("j", False), 0.05, ("f", False))
        return expect("layer", "gk", action=keys)

    def clipboard_seed():
        vm.copy_to(Path(__file__).with_name("clipboard-owner.py"), "lhc-e2e/")
        vm.ssh(f"rm -f {GUEST}/clipboard-ready; systemd-run --user --collect --unit=lhc-e2e-clipboard-owner python3 {GUEST}/clipboard-owner.py")
        deadline = time.monotonic() + 30
        while "ready" not in vm.ssh(f"test ! -f {GUEST}/clipboard-ready || echo ready"):
            if time.monotonic() >= deadline:
                raise StepFailed("clipboard fixture did not acquire focus and seed data")
            time.sleep(0.3)
        return clipboard_preserved()

    def clipboard_preserved():
        expected = {"text/plain;charset=utf-8": b"lhc-before-unicode",
                    "application/x-lhc-e2e": b"binary\x00\xffpayload"}
        import base64
        for mime, data in expected.items():
            output = vm.ssh("bash -o pipefail -c " + shlex.quote(
                f"wl-paste --no-newline --type {shlex.quote(mime)} | base64 -w0"))
            if base64.b64decode(output.strip()) != data:
                raise StepFailed(f"clipboard format {mime} was not preserved")
        return "original text and binary MIME data preserved"

    def clipboard_interruption(mode):
        vm.ssh(f"rm -f {GUEST}/clipboard-ready {GUEST}/clipboard-paste-received; "
               f"systemd-run --user --collect --unit=lhc-e2e-clipboard-{mode} "
               f"python3 {GUEST}/clipboard-owner.py --paste {mode}")
        def marker(name):
            deadline = time.monotonic() + 15
            while "ready" not in vm.ssh(f"test ! -f {GUEST}/{name} || echo ready"):
                if time.monotonic() >= deadline:
                    raise StepFailed(f"clipboard fixture timed out: {name}")
                time.sleep(0.1)
        marker("clipboard-ready")
        time.sleep(3.5)
        vm.qmp.tap("y")
        marker("clipboard-paste-received")
        if mode == "hold":
            guest("toggle-mapper")
        expected = "lhc-new-owner"
        deadline = time.monotonic() + 6
        while True:
            actual = vm.ssh("wl-paste --no-newline --type 'text/plain;charset=utf-8'", check=False).strip()
            if actual == expected:
                break
            if time.monotonic() >= deadline:
                raise StepFailed(f"{mode}: clipboard overwritten: {actual!r}")
            time.sleep(0.1)
        vm.qmp.screendump(vm.logs / f"clipboard-{mode}.png")
        return "new owner retained" if mode == "replace" else "pending paste cancelled on mapper stop; clipboard restored"

    def consent():
        def keys():
            vm.qmp.tap("t")
            time.sleep(4)
            vm.qmp.screendump(vm.logs / "portal-consent.png")
            clicks = vm.get("portal_clicks", [])
            if clicks:
                for x, y in clicks:
                    vm.qmp.click(x, y)
                    time.sleep(0.4)
            else:
                vm.qmp.tap(*vm.get("portal_keys", ["ret"]))
            time.sleep(4)
        return expect("text-ascii", "lhc-ok", action=keys)

    def quick_popup():
        def choose():
            vm.qmp.tap("e")
            time.sleep(2.5)
            vm.qmp.screendump(vm.logs / "quick-popup-open.png")
            vm.qmp.tap("q")
            time.sleep(3)
        return expect("quick-popup", "quick-ok", action=choose)

    def system_actions():
        for key, field, expected in (("3", "current", 1), ("4", "current", 0),
                                     ("1", "workspace", 1), ("2", "workspace", 0)):
            vm.qmp.tap(key)
            deadline = time.monotonic() + 10
            while True:
                output = vm.ssh(f"DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus {BRIDGE}Snapshot false")
                snapshot = json.loads(ast.literal_eval(output.strip())[0])
                if snapshot[field] == expected:
                    break
                if time.monotonic() >= deadline:
                    raise StepFailed(f"key {key}: expected {field}={expected}, got {snapshot[field]}")
                time.sleep(0.2)
        return "us→ru→us; workspace 1→2→1 via mapper sys: actions"

    def command():
        guest("command-marker reset")
        vm.qmp.tap("n")
        return guest("command-marker check")

    def game_mode():
        expect("game-off", "j", ("g",))
        guest("game on")
        expect("game-on", "h", ("g",))
        guest("game off")
        return expect("game-off-again", "j", ("g",))

    def stop_keys():
        guest("toggle-mapper")
        expect("mapper-stopped-native", "a", keys=("a",))
        guest("toggle-mapper")
        return expect("mapper-restarted", "b", keys=("a",))

    try:
        step("boot-session", boot)
        step("install", install)
        step("keyboard-and-config", keyboard)
        step("no-permissions-start", guest, "start-app --popups auto")
        step("no-permissions-no-grab", guest, "mapper --absent --timeout 5")
        step("quit-before-permissions", guest, "quit-app")
        step("permissions-and-extension", permissions)
        # Autostart is expected to launch the app after the fresh login.
        step("autostart", guest, "wait-app --timeout 90")
        step("mapper-start", guest, "mapper")
        step("tray", guest, "tray")
        step("tray-host-lifecycle", tray_recovery)
        if args.examples:
            step("editor-smoke", ui_example, "editor")
            step("interactions", ui_example, "interactions")
        step("passthrough", expect, "passthrough", "c", ("c",))
        step("remap", expect, "remap", "b", ("a",))
        step("chord", expect, "chord", "X", ("d",))
        step("layer-tap-hold", layer)
        step("stop-restart-native", stop_keys)
        step("system-layout-workspace", system_actions)
        step("command", command)
        step("game-mode-on-off", game_mode)
        step("clipboard-seed", clipboard_seed)
        step("text-portal-ascii", consent)
        step("quick-popup-focus-delivery", quick_popup)
        step("text-unicode", expect, "text-unicode", "Привет, ёж 😀", ("y",))
        step("clipboard-restored", clipboard_preserved)
        step("mixed-macro", expect, "mixed-macro", "nmacO", ("m",))
        step("popup-open-hide", guest, "popups")
        step("clipboard-new-owner", clipboard_interruption, "replace")
        step("clipboard-cancel-on-stop", clipboard_interruption, "hold")
        step("quit", guest, "quit-app")
    except (StepFailed, subprocess.SubprocessError, OSError):
        print("Stopped on the first failed step; remaining acceptance is NOT passed.")
    finally:
        try:
            if vm.qemu and vm.qemu.poll() is None:
                vm.qmp.screendump(vm.logs / "final.png")
                vm.copy_from("lhc-e2e/app.log", vm.logs)
                if args.examples:
                    vm.copy_from("lhc-e2e/ui-snapshots", vm.logs)
                (vm.logs / "journal.txt").write_text(vm.ssh("journalctl --user -b --no-pager", check=False))
                (vm.logs / "extensions.txt").write_text(vm.ssh("gnome-extensions list --enabled; journalctl -b --no-pager _COMM=gnome-shell", check=False))
        finally:
            vm.stop(keep=args.keep)
    report.write([f"Date: {time.strftime('%F %T %z')}", f"Commit: {commit()}",
                  f"VM: {vm.name}", f"Package: {package}",
                  f"Keyboard: {state.get('device', 'not detected')}",
                  f"Logs and screenshots: {vm.logs}",
                  "Scope: GNOME smoke only; lock/suspend/replug, other receivers and upgrade not covered."],
                 result_path("linux", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
