#!/usr/bin/env python3
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

binary = Path(sys.argv[1]).resolve()
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
                child_file = Path(f"/proc/{server.pid}/task/{server.pid}/children")
                children = child_file.read_text().split() if child_file.exists() else []
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
            time.sleep(0.2)
            subprocess.run([binary, "show", "emoji"], env=env, check=True, timeout=5)
            time.sleep(0.2)
            if server.poll() is not None:
                raise RuntimeError("parent exited after worker crash")
            log.flush()
            if "Spell popup process unavailable:" not in log_path.read_text():
                raise RuntimeError("worker failure was not reported")
            print("worker crash reported; parent remained responsive")
        finally:
            subprocess.run([binary, "quit"], env=env, timeout=5, check=False)
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
