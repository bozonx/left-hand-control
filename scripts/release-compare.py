#!/usr/bin/env python3
import csv
import os
from pathlib import Path
import socket
import statistics
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
kind = sys.argv[1]
output = Path(sys.argv[2]).resolve()
output.mkdir(parents=True, exist_ok=True)
if kind == "slint":
    binary = root / "prototypes/slint-shell/target/release/slint-shell"
    env = dict(os.environ, SLINT_SHELL_POPUPS="spell", SLINT_BACKEND="winit-software",
               SLINT_SHELL_SOCKET=f"lhc-release-{os.getpid()}.sock",
               SLINT_SHELL_METRICS=str(output / "metrics.csv"))
    socket_path = Path(env["XDG_RUNTIME_DIR"]) / env["SLINT_SHELL_SOCKET"]
elif kind == "tauri":
    binary = root / "src-tauri/target/release/left-hand-control"
    socket_path = output / "tauri.sock"
    env = dict(os.environ, LHC_BENCH_SOCKET=str(socket_path))
else:
    raise SystemExit("usage: release-compare.py slint|tauri OUTPUT")

def command(value):
    if kind == "slint":
        args = [str(binary), "hide"] if value == "hide" else [str(binary), "show", value]
        subprocess.run(args, env=env, check=True, capture_output=True, timeout=5)
    else:
        with socket.socket(socket.AF_UNIX) as client:
            client.connect(str(socket_path))
            client.sendall((value + "\n").encode())
            response = client.makefile().readline().strip()
            if response != "ok":
                raise RuntimeError(response)

def memory(label, process):
    raw = subprocess.check_output([root / "prototypes/slint-shell/scripts/mem.sh", str(process.pid), label], text=True)
    return next(csv.DictReader(raw.splitlines()))

def descendants(root_pid):
    parents = {}
    for status in Path("/proc").glob("[0-9]*/status"):
        try:
            fields = dict(line.split(":", 1) for line in status.read_text().splitlines())
            parents.setdefault(int(fields["PPid"]), []).append(int(status.parent.name))
        except (FileNotFoundError, ProcessLookupError, KeyError, ValueError):
            pass
    pids = [root_pid]
    for pid in pids:
        pids.extend(parents.get(pid, []))
    return pids

def cpu_ticks(root_pid):
    total = 0
    for pid in descendants(root_pid):
        try:
            fields = Path(f"/proc/{pid}/stat").read_text().split()
            total += int(fields[13]) + int(fields[14])
        except (FileNotFoundError, ProcessLookupError):
            pass
    return total

samples = []
latencies = []
first = []
with (output / "process.log").open("w") as log:
    process = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 30
        while not socket_path.exists():
            if process.poll() is not None: raise RuntimeError("process exited")
            if time.monotonic() > deadline: raise RuntimeError("socket timeout")
            time.sleep(.05)
        time.sleep(2)
        command("hide")
        time.sleep(1)
        samples.append(memory("start-hidden", process))
        for name in ("emoji", "quick"):
            start = time.perf_counter_ns(); command(name)
            first.append({"window": name, "milliseconds": f"{(time.perf_counter_ns() - start) / 1e6:.3f}"})
            time.sleep(.5); command("hide"); time.sleep(.3)
        samples.append(memory("popups-warm-hidden", process))
        start = time.perf_counter_ns(); command("settings")
        first.append({"window": "settings", "milliseconds": f"{(time.perf_counter_ns() - start) / 1e6:.3f}"})
        time.sleep(2)
        samples.append(memory("settings-open", process))
        command("hide"); time.sleep(.5)
        samples.append(memory("settings-hidden", process))
        ticks = cpu_ticks(process.pid)
        cpu_start = time.monotonic()
        time.sleep(5)
        cpu_seconds = (cpu_ticks(process.pid) - ticks) / os.sysconf("SC_CLK_TCK")
        cpu_percent = cpu_seconds / (time.monotonic() - cpu_start) * 100
        for name in ("emoji", "quick", "settings"):
            for _ in range(100):
                start = time.perf_counter_ns(); command(name); elapsed = (time.perf_counter_ns() - start) / 1e6
                latencies.append({"window": name, "milliseconds": f"{elapsed:.3f}"})
                command("hide"); time.sleep(.02)
    finally:
        if process.poll() is None:
            if kind == "tauri":
                try: command("quit")
                except Exception: process.terminate()
            else:
                subprocess.run([binary, "quit"], env=env, capture_output=True)
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
        socket_path.unlink(missing_ok=True)

with (output / "memory.csv").open("w", newline="") as target:
    writer = csv.DictWriter(target, fieldnames=["label", "processes", "rss_kib", "pss_kib"])
    writer.writeheader(); writer.writerows(samples)
with (output / "command-latency.csv").open("w", newline="") as target:
    writer = csv.DictWriter(target, fieldnames=["window", "milliseconds"])
    writer.writeheader(); writer.writerows(latencies)
with (output / "first-command-latency.csv").open("w", newline="") as target:
    writer = csv.DictWriter(target, fieldnames=["window", "milliseconds"])
    writer.writeheader(); writer.writerows(first)
with (output / "idle-cpu.csv").open("w", newline="") as target:
    writer = csv.DictWriter(target, fieldnames=["interval_seconds", "cpu_seconds", "cpu_percent"])
    writer.writeheader(); writer.writerow({"interval_seconds": "5", "cpu_seconds": f"{cpu_seconds:.3f}", "cpu_percent": f"{cpu_percent:.3f}"})
for name in ("emoji", "quick", "settings"):
    values = sorted(float(row["milliseconds"]) for row in latencies if row["window"] == name)
    print(name, "p50", statistics.median(values), "p95", values[94], "max", max(values))
