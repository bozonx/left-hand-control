#!/usr/bin/env python3
"""Run clipboard integration tests in a private virtual KWin and D-Bus session."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def main():
    if sys.argv[1:] != ["--isolated"]:
        os.execvp(
            "dbus-run-session",
            ["dbus-run-session", "--", sys.executable, __file__, "--isolated"],
        )
    repo = Path(__file__).resolve().parents[4]
    with tempfile.TemporaryDirectory(prefix="lhc-clipboard-") as directory:
        root = Path(directory)
        for name in ("runtime", "config", "data", "cache"):
            (root / name).mkdir(mode=0o700)
        env = os.environ.copy()
        env.update(
            XDG_RUNTIME_DIR=str(root / "runtime"),
            XDG_CONFIG_HOME=str(root / "config"),
            XDG_DATA_HOME=str(root / "data"),
            XDG_CACHE_HOME=str(root / "cache"),
            WAYLAND_DISPLAY="lhc-test-clipboard",
            KWIN_WAYLAND_NO_LIBINPUT="1",
            KWIN_COMPOSE="Q",
            QT_QPA_PLATFORM="offscreen",
        )
        with (root / "kwin.log").open("w+") as log:
            compositor = subprocess.Popen(
                [
                    "kwin_wayland", "--virtual", "--no-lockscreen",
                    "--no-global-shortcuts", "--no-kactivities",
                    "--socket", "lhc-test-clipboard",
                ],
                env=env, stdout=log, stderr=log,
            )
            try:
                deadline = time.monotonic() + 15
                while not (root / "runtime" / "lhc-test-clipboard").exists():
                    if compositor.poll() is not None or time.monotonic() >= deadline:
                        log.seek(0)
                        print(log.read(), file=sys.stderr)
                        raise RuntimeError("isolated KWin failed to start")
                    time.sleep(0.05)
                result = subprocess.run(
                    [
                        "cargo", "test", "--locked", "-p", "lhc-core",
                        "isolated_clipboard", "--", "--ignored", "--nocapture",
                    ],
                    cwd=repo, env=env, timeout=600,
                )
                if result.returncode:
                    log.seek(0)
                    print(log.read()[-6000:], file=sys.stderr)
                return result.returncode
            finally:
                compositor.terminate()
                try:
                    compositor.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    compositor.kill()
                    compositor.wait()


if __name__ == "__main__":
    sys.exit(main())
