#!/usr/bin/env python3
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

binary = Path(sys.argv[1]).resolve()


def workers(pid):
    """Spell worker children of `pid`; any thread may have started them."""
    found = []
    for children in Path(f"/proc/{pid}/task").glob("*/children"):
        for child in children.read_text().split():
            try:
                cmdline = Path(f"/proc/{child}/cmdline").read_bytes()
            except OSError:
                continue
            if b"--spell-worker" in cmdline:
                found.append(int(child))
    return found

socket = f"lhc-worker-crash-{os.getpid()}.sock"
with tempfile.TemporaryDirectory() as directory:
    log_path = Path(directory) / "server.log"
    with log_path.open("w+") as log:
        env = os.environ | {
            "SLINT_SHELL_POPUPS": "spell",
            "SLINT_SHELL_SOCKET": socket,
            "SLINT_BACKEND": "winit-software",
            "SLINT_SHELL_METRICS": str(Path(directory) / "crash.csv"),
        }
        server = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 15
            children = []
            while time.monotonic() < deadline:
                if server.poll() is not None:
                    raise RuntimeError("parent exited during startup")
                children = workers(server.pid)
                if children:
                    break
                time.sleep(0.05)
            if not children:
                raise RuntimeError("Spell worker was not started")
            while time.monotonic() < deadline:
                ping = subprocess.run(
                    [binary, "ping"],
                    env=env,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    timeout=5,
                )
                if ping.returncode == 0:
                    break
                time.sleep(0.05)
            else:
                raise RuntimeError("parent IPC did not become ready")
            worker = int(children[0])
            os.kill(worker, signal.SIGKILL)
            deadline = time.monotonic() + 20
            replacement = None
            while time.monotonic() < deadline:
                replacement = next((child for child in workers(server.pid) if child != worker), None)
                if replacement is not None:
                    break
                time.sleep(0.1)
            if replacement is None:
                raise RuntimeError("Spell worker did not restart")
            subprocess.run([binary, "show", "emoji"], env=env, check=True, timeout=5)
            if server.poll() is not None:
                raise RuntimeError("parent exited after worker crash")
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                # Killed while running, or while still starting up.
                text = log_path.read_text()
                if "Spell popup process exited" in text or "Spell worker exited" in text:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError("worker failure was not reported")
            print("worker crash reported; replacement started; parent remained responsive")
        finally:
            subprocess.run([binary, "quit"], env=env, timeout=5, check=False)
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
