#!/usr/bin/env python3
"""Linux VM e2e (L4): install a release package into a disposable overlay of a
prepared KDE Plasma VM and check permissions, tray, autostart, the mapper with
real keyboard events from QEMU, `text:` through the portal, popups and removal.

Usage: run-linux.py --vm /mnt/disk2/vm/lhc-fedora-kde --package <file> [--show] [--keep]
See docs/development/guide.md, "E2E в виртуальных машинах".
"""

import argparse
import platform
from pathlib import Path
import shlex
import time

from vmlib import Report, StepFailed, Vm, commit, environment_ok, result_path

# scp paths are relative to the guest home; shell commands use ~.
GUEST = "~/lhc-e2e"
GUEST_PY = f"python3 {GUEST}/linux-guest.py"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vm", required=True, help="VM directory with vm.json")
    parser.add_argument("--package", required=True, type=Path,
                        help=".deb, .rpm or .tar.gz built by release-linux.sh")
    parser.add_argument("--show", action="store_true", help="show the VM display")
    parser.add_argument("--keep", action="store_true", help="keep the overlay disk")
    args = parser.parse_args()
    environment_ok()
    package = args.package.resolve()
    if not package.is_file():
        raise SystemExit(f"{package} not found")
    vm = Vm(args.vm)
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

    archive = package.name.endswith(".tar.gz")
    installer = "~/.local/lib/dev.bozonx.left-hand-control/install-linux.sh"

    def install(key="install"):
        if archive:
            vm.ssh(f"cd {GUEST} && rm -rf bundle && mkdir bundle && "
                   f"tar -xzf {shlex.quote(package.name)} -C bundle && "
                   f"cd bundle/*/ && ./install-linux.sh")
        else:
            vm.ssh(vm.get(key).format(package=guest_package))

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

    def portal_warm_up():
        # The first `text:` asks for RemoteDesktop consent; later ones reuse
        # the saved restore token.
        guest("start-receiver --name warmup")
        vm.qmp.tap("t")
        time.sleep(4)
        vm.qmp.screendump(vm.logs / "portal-consent.png")
        vm.qmp.tap(*vm.get("portal_keys", ["ret"]))
        time.sleep(3)
        vm.qmp.tap("ret")
        return "received: " + repr(guest("receiver-result --name warmup", check=False))

    def mapper_text():
        guest("start-receiver --name main")
        vm.qmp.tap("a", "d", "c", "t")
        time.sleep(3)
        vm.qmp.screendump(vm.logs / "receiver.png")
        vm.qmp.tap("ret")
        return guest("receiver-result --name main --expect bXclhc-ok")

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
            vm.ssh("journalctl --user -b --no-pager -n 2000", check=False), encoding="utf-8")

    try:
        report.step("boot", boot, fatal=True)
        report.step("plasma-session", session, fatal=True)
        report.step("upload", upload, fatal=True)
        report.step("install", install, fatal=True)
        report.step("installed-files", check_install)
        report.step("keyboard-probe", probe_keyboard, fatal=True)
        report.step("test-config", write_config, fatal=True)
        report.step("start-without-input", start_without_permissions)
        report.step("tray-item", guest, "tray")
        report.step("no-mapper-without-input", guest, "mapper --absent --timeout 8")
        report.step("autostart-entry", guest, "autostart")
        report.step("relogin-with-input", relogin_with_input, fatal=True)
        report.step("autostart-launch", guest, "wait-app --timeout 90")
        report.step("tray-after-login", guest, "tray")
        report.step("mapper-running", guest, "mapper --timeout 30")
        report.step("portal-consent", portal_warm_up)
        report.step("mapper-keys-and-text", mapper_text)
        report.step("popups", guest, "popups")
        report.step("mapper-stop", guest, "toggle-mapper")
        report.step("mapper-start", guest, "toggle-mapper")
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
        f"Package: {package.name}",
        f"Keyboard: {state.get('device', 'not detected')}",
        f"Host: {platform.node()}, {platform.release()}",
        f"Logs and screenshots: {vm.logs}",
    ]
    report.write(header, result_path("linux", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
