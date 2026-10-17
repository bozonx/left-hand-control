#!/usr/bin/env python3
"""Guest side of the Linux VM e2e stand. `run-linux.py` copies this file into
the VM and calls one subcommand per step over ssh as the autologin user.
Only the Python standard library and tools of a KDE Plasma install are used.
"""

import argparse
import glob
import grp
import json
import os
from pathlib import Path
import select
import shutil
import struct
import subprocess
import sys
import time

APP_ID = "dev.bozonx.left-hand-control"
MAPPER_DEVICE = "LeftHandControl Virtual Keyboard"
TRAY_ID = "lhc-slint-shell"
HOME = Path.home()
WORK = HOME / "lhc-e2e"
APP_LOG = WORK / "app.log"
RECEIVER_OUT = WORK / "receiver.txt"
CONFIG_DIR = HOME / ".config" / APP_ID
DATA_DIR = HOME / ".local/share" / APP_ID

# KEY_A, KEY_D, KEY_T are remapped by the e2e layout; KEY_C passes through.
E2E_LAYOUT = """\
description: VM e2e
rules:
  - key: KeyA
    tap: KeyB
  - key: KeyD
    tap: Shift+KeyX
  - key: KeyT
    tap: "text:lhc-ok"
"""


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def wait(what, timeout, probe, interval=0.5):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            fail(f"timed out after {timeout}s waiting for {what}")
        time.sleep(interval)


def binary():
    path = shutil.which("left-hand-control") or shutil.which(
        "left-hand-control", path=str(HOME / ".local/bin"))
    if not path:
        fail("left-hand-control is not installed")
    return path


def app(*arguments, timeout=10):
    return subprocess.run([binary(), *arguments], capture_output=True, text=True,
                          timeout=timeout)


def session_pid(group=None):
    """PID of this user's plasmashell, optionally with `group` in its groups."""
    gid = grp.getgrnam(group).gr_gid if group else None
    for status in glob.glob("/proc/[0-9]*/status"):
        try:
            text = Path(status).read_text()
            if os.stat(status).st_uid != os.getuid():
                continue
        except OSError:
            continue
        fields = dict(line.split(":\t", 1) for line in text.splitlines() if ":\t" in line)
        if fields.get("Name", "").strip() != "plasmashell":
            continue
        if gid is None or str(gid) in fields.get("Groups", "").split():
            return int(Path(status).parent.name)
    return None


def user_environment():
    output = subprocess.run(["systemctl", "--user", "show-environment"],
                            capture_output=True, text=True).stdout
    return dict(line.split("=", 1) for line in output.splitlines() if "=" in line)


def systemd_run(unit, command, output, extra_env=(), errors=None):
    subprocess.run(["systemctl", "--user", "reset-failed", unit],
                   capture_output=True)
    subprocess.run(
        ["systemd-run", "--user", "--quiet", "--collect", f"--unit={unit}",
         f"--property=StandardOutput=append:{output}",
         f"--property=StandardError=append:{errors or output}",
         *(f"--setenv={item}" for item in extra_env), *command],
        check=True,
    )


def unit_active(unit):
    return subprocess.run(["systemctl", "--user", "is-active", "--quiet", unit]).returncode == 0


def mapper_running():
    try:
        return f'N: Name="{MAPPER_DEVICE}"' in Path("/proc/bus/input/devices").read_text()
    except OSError:
        return False


# --- subcommands ---------------------------------------------------------

def cmd_wait_session(args):
    wait(f"a Plasma session{' with group ' + args.group if args.group else ''}",
         args.timeout, lambda: session_pid(args.group), interval=2)
    wait("WAYLAND_DISPLAY in the user manager", 60,
         lambda: "WAYLAND_DISPLAY" in user_environment())
    WORK.mkdir(exist_ok=True)
    print(user_environment().get("XDG_SESSION_TYPE", "wayland"))


def cmd_check_install(args):
    path = binary()
    version = app("--version").stdout.strip()
    prefixes = [Path("/usr/share"), HOME / ".local/share"]
    problems = []
    def present(relative):
        return any((prefix / relative).exists() for prefix in prefixes)
    for relative in (f"applications/{APP_ID}.desktop", f"metainfo/{APP_ID}.metainfo.xml",
                     f"icons/hicolor/scalable/apps/{APP_ID}.svg",
                     f"icons/hicolor/64x64/apps/{APP_ID}.png"):
        if not present(relative):
            problems.append(f"missing {relative}")
    if shutil.which("desktop-file-validate"):
        for prefix in prefixes:
            entry = prefix / f"applications/{APP_ID}.desktop"
            if entry.exists():
                result = subprocess.run(["desktop-file-validate", str(entry)],
                                        capture_output=True, text=True)
                if result.returncode:
                    problems.append(f"desktop-file-validate: {result.stdout}{result.stderr}")
    if args.packaged:
        if not Path("/usr/lib/udev/rules.d/60-left-hand-control.rules").exists():
            problems.append("udev rule is not installed")
        # The uaccess rule gives the active session's user write access.
        if not os.access("/dev/uinput", os.W_OK):
            problems.append("/dev/uinput is not writable for the session user")
    if problems:
        fail("\n".join(problems))
    print(f"{path}: {version}")


def cmd_probe_keyboard(args):
    """Report the keyboard node that receives the host's QMP key events.
    Needs read access to /dev/input (run through sudo before the user has it)."""
    candidates = {}
    for node in sorted(glob.glob("/dev/input/event*")):
        caps = Path(f"/sys/class/input/{Path(node).name}/device/capabilities/key")
        try:
            words = caps.read_text().split()
        except OSError:
            continue
        bits = int("".join(f"{int(word, 16):064b}" for word in words) or "0", 2)
        if bits >> 30 & 1 and bits >> 42 & 1:  # KEY_A and KEY_LEFTSHIFT
            try:
                candidates[os.open(node, os.O_RDONLY | os.O_NONBLOCK)] = node
            except OSError:
                pass
    if not candidates:
        fail("no readable keyboards")
    size = struct.calcsize("llHHi")
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select(list(candidates), [], [], 0.5)
        for fd in ready:
            data = os.read(fd, size * 64)
            for offset in range(0, len(data) - size + 1, size):
                _, _, kind, code, _ = struct.unpack_from("llHHi", data, offset)
                if kind == 1 and code == 42:
                    node = candidates[fd]
                    for link in glob.glob("/dev/input/by-id/*-event-kbd"):
                        if os.path.realpath(link) == node:
                            node = link
                    print(node)
                    return
    fail("no keyboard received the probe keys; is usb-kbd attached?")


def cmd_write_config(args):
    (DATA_DIR / "layouts").mkdir(parents=True, exist_ok=True)
    (DATA_DIR / "layouts/E2E.yaml").write_text(E2E_LAYOUT, encoding="utf-8")
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    settings = {
        "inputDevicePath": args.device,
        "layoutMode": "manual",
        "currentLayoutId": "user:E2E",
        "manualActiveLayoutId": "user:E2E",
        "startMapperOnLaunch": True,
        "launchOnStartup": True,
        "locale": "en-US",
    }
    (CONFIG_DIR / "config.json").write_text(
        json.dumps({"version": 1, "settings": settings}, indent=2), encoding="utf-8")


def cmd_start_app(args):
    WORK.mkdir(exist_ok=True)
    env = ["RUST_LOG=info", f"LHC_POPUPS={args.popups}"]
    systemd_run("lhc-e2e-app", [binary()], APP_LOG, env)
    cmd_wait_app(args)


def cmd_wait_app(args):
    wait("an answer to `ping`", args.timeout, lambda: app("ping").returncode == 0)


def cmd_quit_app(args):
    app("quit")
    wait("the process to exit", 15,
         lambda: subprocess.run(["pgrep", "-u", str(os.getuid()), "-x", "left-hand-contr"],
                                capture_output=True).returncode != 0)


def cmd_mapper(args):
    if args.absent:
        # A failing start is retried for a few seconds after launch.
        time.sleep(args.timeout)
        if mapper_running():
            fail("the mapper virtual keyboard exists")
        return
    wait("the mapper virtual keyboard", args.timeout, mapper_running)


def cmd_toggle_mapper(args):
    before = mapper_running()
    if app("toggle-mapper").returncode:
        fail("toggle-mapper failed")
    wait(f"the mapper to {'stop' if before else 'start'}", 15,
         lambda: mapper_running() != before)


def cmd_tray(args):
    def items():
        result = subprocess.run(
            ["busctl", "--user", "--json=short", "get-property",
             "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
             "org.kde.StatusNotifierWatcher", "RegisteredStatusNotifierItems"],
            capture_output=True, text=True)
        return json.loads(result.stdout)["data"] if result.returncode == 0 else []

    def ours():
        for item in items():
            service, path = (item.split("@", 1) if "@" in item
                             else (item.split("/", 1)[0], "/" + item.split("/", 1)[1]))
            result = subprocess.run(
                ["busctl", "--user", "--json=short", "get-property", service, path,
                 "org.kde.StatusNotifierItem", "Id"], capture_output=True, text=True)
            if result.returncode == 0 and json.loads(result.stdout)["data"] == TRAY_ID:
                return item
        return None
    print(wait("the tray item", args.timeout, ours))


def cmd_autostart(args):
    entry = HOME / ".config/autostart" / f"{APP_ID}.desktop"
    wait(str(entry), 10, entry.exists)
    exec_line = next((line for line in entry.read_text().splitlines()
                      if line.startswith("Exec=")), "")
    target = os.path.realpath(binary())
    if target not in exec_line and binary() not in exec_line:
        fail(f"{entry}: {exec_line!r} does not start {target}")
    print(exec_line)


def cmd_start_receiver(args):
    RECEIVER_OUT.unlink(missing_ok=True)
    unit = f"lhc-e2e-receiver-{args.name}"
    systemd_run(unit, ["kdialog", "--title", "LHC e2e receiver",
                       "--inputbox", "Mapper output"], RECEIVER_OUT,
                errors=WORK / "receiver.log")
    # Let KWin map and focus the dialog before the host types.
    time.sleep(args.settle)


def cmd_receiver_result(args):
    unit = f"lhc-e2e-receiver-{args.name}"
    finished = True
    deadline = time.monotonic() + args.timeout
    while unit_active(unit):
        if time.monotonic() >= deadline:
            finished = False
            subprocess.run(["systemctl", "--user", "stop", unit], capture_output=True)
            break
        time.sleep(0.3)
    text = RECEIVER_OUT.read_text(errors="replace").strip("\n") if RECEIVER_OUT.exists() else ""
    if args.expect is None:
        print(text)
        return
    if not finished:
        fail(f"the receiver was not accepted; text so far unknown, output {text!r}")
    if text != args.expect:
        fail(f"expected {args.expect!r}, received {text!r}")
    print(text)


def cmd_popups(args):
    for popup in ("quick", "emoji"):
        for command in (("show", popup), ("hide",)):
            result = app(*command)
            if result.returncode:
                fail(f"{' '.join(command)}: {result.stdout}{result.stderr}")
            time.sleep(1)
    if app("ping").returncode:
        fail("the instance stopped answering after the popups")


def cmd_check_removed(args):
    if shutil.which("left-hand-control") or (HOME / ".local/bin/left-hand-control").exists():
        fail("the binary is still installed")
    if not (CONFIG_DIR / "config.json").exists():
        fail("the user configuration was removed with the package")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    p = sub.add_parser("wait-session")
    p.add_argument("--group")
    p.add_argument("--timeout", type=float, default=300)
    p = sub.add_parser("check-install")
    p.add_argument("--packaged", action="store_true")
    p = sub.add_parser("probe-keyboard")
    p.add_argument("--timeout", type=float, default=20)
    p = sub.add_parser("write-config")
    p.add_argument("--device", required=True)
    p = sub.add_parser("start-app")
    p.add_argument("--popups", default="auto")
    p.add_argument("--timeout", type=float, default=30)
    p = sub.add_parser("wait-app")
    p.add_argument("--timeout", type=float, default=60)
    sub.add_parser("quit-app")
    p = sub.add_parser("mapper")
    p.add_argument("--absent", action="store_true")
    p.add_argument("--timeout", type=float, default=20)
    sub.add_parser("toggle-mapper")
    p = sub.add_parser("tray")
    p.add_argument("--timeout", type=float, default=20)
    sub.add_parser("autostart")
    p = sub.add_parser("start-receiver")
    p.add_argument("--name", default="main")
    p.add_argument("--settle", type=float, default=2.5)
    p = sub.add_parser("receiver-result")
    p.add_argument("--name", default="main")
    p.add_argument("--expect")
    p.add_argument("--timeout", type=float, default=10)
    sub.add_parser("popups")
    sub.add_parser("check-removed")
    args = parser.parse_args()
    globals()["cmd_" + args.command.replace("-", "_")](args)


if __name__ == "__main__":
    main()
