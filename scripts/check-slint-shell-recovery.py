import argparse
import csv
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("--binary", default="target/debug/slint-shell")
parser.add_argument("--popups", choices=["winit", "spell"], default="spell")
parser.add_argument("--live-preview", action="store_true")
args = parser.parse_args()
binary = str(Path(args.binary).resolve())
preview_source = Path("apps/slint-shell/ui/settings-page.slint")
preview_original = None
preview_modified = None

with tempfile.TemporaryDirectory(prefix="lhc-slint-recovery-") as directory:
    root = Path(directory)
    config = root / "linux/.config/dev.bozonx.left-hand-control/config.json"
    config.parent.mkdir(parents=True)
    config.write_text("{broken", encoding="utf-8")
    log_path = root / "shell.log"
    metrics = root / "metrics.csv"
    env = dict(os.environ, LHC_DEV_DIR=directory,
               SLINT_SHELL_SOCKET=f"lhc-recovery-{os.getpid()}.sock",
               SLINT_SHELL_HOTKEYS="off", SLINT_SHELL_POPUPS=args.popups,
               SLINT_SHELL_METRICS=str(metrics), SLINT_BACKEND="winit-software")

    def log_text():
        return log_path.read_text(encoding="utf-8", errors="replace")

    def send(*command):
        subprocess.run([binary, *command], env=env, check=True,
                       capture_output=True, timeout=5)

    def wait_for(check, description):
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if check():
                return
            if process.poll() is not None:
                raise AssertionError(f"Shell exited while waiting for {description}\n{log_text()}")
            time.sleep(0.05)
        raise AssertionError(f"Timed out waiting for {description}\n{log_text()}")

    def shown(window):
        path = Path(str(metrics) + ".spell.csv") if args.popups == "spell" else metrics
        if not path.exists():
            return False
        with path.open() as stream:
            return any(row.get("window") == window and row.get("event") == "t2_shown"
                       for row in csv.DictReader(stream))

    with log_path.open("w") as output:
        process = subprocess.Popen([binary], env=env, stdout=output,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            wait_for(lambda: "ready; backend=" in log_text(), "IPC readiness")
            assert "configuration unavailable" in log_text()
            config.write_text('{"version":1,"settings":{}}', encoding="utf-8")
            wait_for(lambda: "configuration recovered" in log_text(), "configuration recovery")
            send("show", "quick", "1")
            wait_for(lambda: shown("quick"), "quick popup")
            send("hide")
            if args.live_preview:
                send("show", "settings")
                preview_original = preview_source.read_bytes()
                preview_modified = preview_original + b"\n"
                preview_source.write_bytes(preview_modified)
                wait_for(lambda: "Reloaded component SettingsWindow" in log_text(), "live UI reload")
                send("hide")
                send("show", "quick", "1")
                send("hide")
            if args.popups == "spell":
                wait_for(lambda: bool(re.findall(r"worker pid=(\d+)", log_text())), "worker readiness")
                workers = re.findall(r"worker pid=(\d+)", log_text())
                os.kill(int(workers[-1]), signal.SIGKILL)
                send("show", "emoji", "1")
                wait_for(lambda: len(re.findall(r"worker pid=(\d+)", log_text())) > len(workers),
                         "worker restart")
                wait_for(lambda: shown("emoji"), "queued popup after restart")
                send("hide")
            send("quit")
            process.wait(timeout=10)
            assert process.returncode == 0, log_text()
            assert "panicked" not in log_text(), log_text()
            print(f"Slint recovery passed ({args.popups}): configuration retry and popup delivery")
            if args.live_preview:
                print("Live Preview passed: UI reload, IPC callbacks and clean shutdown")
        finally:
            if preview_original is not None and preview_source.read_bytes() == preview_modified:
                preview_source.write_bytes(preview_original)
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=10)
