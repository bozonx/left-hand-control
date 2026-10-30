#!/usr/bin/env python3
"""Exercise a read-only hint worker at real output scales in a private KWin."""
import json
import os
from pathlib import Path
import socket
import subprocess
import time

if not os.environ.get("WAYLAND_DISPLAY", "").startswith("lhc-a11y-"):
    raise SystemExit("Requires run-isolated.sh private compositor")
out = Path(os.environ["LHC_AUDIT_OUT"])
repo = Path(os.environ["LHC_AUDIT_ROOT"])
outputs = json.loads(subprocess.check_output(["kscreen-doctor", "-j"], text=True))
name = next(o["name"] for o in outputs["outputs"] if o["connected"] and o["enabled"])
env = dict(os.environ, LHC_SOCKET="contrast-hint.sock", WAYLAND_DEBUG="client")
path = Path(env["XDG_RUNTIME_DIR"]) / "lhc-slint-shell" / env["LHC_SOCKET"]
log_path = out / "spell-hint.log"

def command(value):
    with socket.socket(socket.AF_UNIX) as client:
        client.settimeout(3)
        client.connect(str(path))
        client.sendall((json.dumps({"command": value, "token": None}) + "\n").encode())
        reply = client.recv(4096)
        assert reply.startswith(b"queued"), reply

with log_path.open("w") as log:
    worker = subprocess.Popen([str(repo / "target/debug/left-hand-control"), "--spell-worker"], env=env, stdout=log, stderr=log)
    try:
        for _ in range(100):
            if path.exists():
                break
            assert worker.poll() is None, "worker exited during startup"
            time.sleep(.1)
        else:
            raise RuntimeError("worker socket did not appear")
        keys = [{"label": f"K{i:02}", "kind": "action", "action": "Ctrl+C", "emoji": False} for i in range(30)]
        hint = {"name": "Layer / Слой", "left": keys, "right": keys}
        command("layer-hint " + json.dumps(hint))
        for scale, width in [(1.25, 1536), (1.5, 1280), (2, 960), (1, 1920)]:
            trace_start = len(log_path.read_text())
            subprocess.run(["kscreen-doctor", f"output.{name}.scale.{scale}"], check=True, stdout=subprocess.DEVNULL)
            time.sleep(1)
            assert worker.poll() is None, f"worker exited at {scale}"
            # The protocol trace proves the compositor configured the requested
            # output width and the viewport uses it, rather than the old 1290.
            trace = log_path.read_text()[trace_start:]
            assert f"set_destination({width}, 480)" in trace, f"missing viewport {width} at {scale}"
            (out / f"spell-output-{scale}.json").write_text(subprocess.check_output(["kscreen-doctor", "-j"], text=True))
            print(f"Spell hint: output scale {scale}, logical viewport {width}x480 passed", flush=True)
        command("layer-hint null")
        command("quit")
        assert worker.wait(timeout=10) == 0
    finally:
        if worker.poll() is None:
            worker.terminate()
            worker.wait(timeout=10)
