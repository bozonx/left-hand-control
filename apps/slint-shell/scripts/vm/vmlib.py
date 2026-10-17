"""Shared helpers of the local KVM/QEMU e2e stands: VM description, a disposable
overlay disk, QEMU with a QMP socket, key injection and ssh into the guest.

A VM lives in its own directory outside the repository with a `vm.json` that
describes it (see docs/development/guide.md, "E2E в виртуальных машинах").
"""

import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

REPO = Path(__file__).resolve().parents[4]


class StepFailed(Exception):
    pass


class Vm:
    """One VM directory: `vm.json`, the base image and per-run state."""

    def __init__(self, directory):
        self.dir = Path(directory).expanduser().resolve()
        config_path = self.dir / "vm.json"
        if not config_path.is_file():
            raise SystemExit(f"{config_path} not found; see docs/development/guide.md")
        self.config = json.loads(config_path.read_text(encoding="utf-8"))
        self.name = self.config.get("name", self.dir.name)
        self.base = self.dir / self.config.get("base", "base.qcow2")
        self.user = self.config["user"]
        self.key = Path(self.config["ssh_key"]).expanduser()
        self.port = int(self.config.get("ssh_port", 2222))
        stamp = time.strftime("%Y%m%d-%H%M%S")
        self.run_dir = self.dir / "runs" / stamp
        self.logs = self.run_dir / "logs"
        self.qemu = None
        self.swtpm = None
        # UNIX socket paths are limited to 108 bytes: keep them short.
        self.sockets = Path(tempfile.mkdtemp(
            prefix="lhc-vm-", dir=os.environ.get("XDG_RUNTIME_DIR") or "/tmp"))

    def get(self, key, default=None):
        return self.config.get(key, default)

    # --- QEMU lifecycle -------------------------------------------------

    def start(self, show=False):
        if not self.base.is_file():
            raise SystemExit(f"base image {self.base} not found")
        self.logs.mkdir(parents=True)
        overlay = self.run_dir / "run.qcow2"
        subprocess.run(
            ["qemu-img", "create", "-q", "-f", "qcow2", "-F", "qcow2",
             "-b", str(self.base), str(overlay)],
            check=True,
        )
        bus = self.get("disk_bus", "virtio")
        command = [
            "qemu-system-x86_64", "-name", f"lhc-e2e-{self.name}",
            "-enable-kvm", "-cpu", self.get("cpu", "host"),
            "-smp", str(self.get("cpus", 4)), "-m", str(self.get("memory", 6144)),
            "-device", "qemu-xhci", "-device", "usb-tablet",
            "-device", "usb-kbd,id=kbd0",
            "-netdev", f"user,id=net0,hostfwd=tcp:127.0.0.1:{self.port}-:22",
            "-device", f"{self.get('nic', 'virtio-net-pci')},netdev=net0",
            "-device", "virtio-rng-pci",
            "-qmp", f"unix:{self.sockets / 'qmp.sock'},server=on,wait=off",
            "-display", "gtk,gl=off" if show else "none",
        ]
        if self.get("vga"):
            command += ["-vga", "none", "-device", self.get("vga")]
        if self.get("uefi"):
            command += self._uefi_arguments()
        else:
            command += ["-machine", "q35,accel=kvm"]
        if bus == "ahci":
            command += [
                "-device", "ich9-ahci,id=ahci",
                "-drive", f"file={overlay},if=none,id=osdisk,format=qcow2,discard=unmap",
                "-device", "ide-hd,drive=osdisk,bus=ahci.0,bootindex=1",
            ]
        else:
            command += ["-drive", f"file={overlay},if=virtio,format=qcow2,discard=unmap"]
        cache = self.get("cache_disk")
        if cache:
            # Persistent between runs: keeps Cargo's target directory warm.
            command += ["-drive", f"file={self.dir / cache},if=none,id=cache,format=qcow2"]
            command += ["-device", "ide-hd,drive=cache,bus=ahci.1" if bus == "ahci"
                        else "virtio-blk-pci,drive=cache"]
        with (self.logs / "qemu.log").open("w") as log:
            self.qemu = subprocess.Popen(command, stdout=log, stderr=log)
        self.qmp = Qmp(self.sockets / "qmp.sock", self.qemu)

    def _uefi_arguments(self):
        uefi = self.get("uefi")
        vars_copy = self.run_dir / "OVMF_VARS.fd"
        shutil.copy2(self.dir / uefi["vars"], vars_copy)
        arguments = [
            "-machine", "q35,accel=kvm,smm=on",
            "-global", "driver=cfi.pflash01,property=secure,value=on",
            "-drive", f"if=pflash,format=raw,readonly=on,file={uefi['code']}",
            "-drive", f"if=pflash,format=raw,file={vars_copy}",
        ]
        if uefi.get("tpm"):
            # Windows binds state to the TPM: each run starts from a copy of
            # the TPM that belongs to the base image.
            tpm = self.run_dir / "tpm"
            shutil.copytree(self.dir / uefi["tpm"], tpm)
            sock = self.sockets / "swtpm.sock"
            with (self.logs / "swtpm.log").open("w") as log:
                self.swtpm = subprocess.Popen(
                    ["swtpm", "socket", "--tpm2", "--tpmstate", f"dir={tpm}",
                     "--ctrl", f"type=unixio,path={sock}"],
                    stdout=log, stderr=log,
                )
            wait_for("swtpm socket", 10, sock.exists)
            arguments += [
                "-chardev", f"socket,id=chrtpm,path={sock}",
                "-tpmdev", "emulator,id=tpm0,chardev=chrtpm",
                "-device", "tpm-tis,tpmdev=tpm0",
            ]
        return arguments

    def stop(self, keep=False):
        if self.qemu and self.qemu.poll() is None:
            try:
                self.qmp.execute("system_powerdown")
                self.qemu.wait(timeout=90)
            except (OSError, StepFailed, subprocess.TimeoutExpired):
                self.qemu.kill()
                self.qemu.wait()
        if self.swtpm and self.swtpm.poll() is None:
            self.swtpm.terminate()
            self.swtpm.wait()
        shutil.rmtree(self.sockets, ignore_errors=True)
        if not keep:
            for name in ("run.qcow2", "OVMF_VARS.fd", "tpm"):
                path = self.run_dir / name
                if path.is_dir():
                    shutil.rmtree(path)
                else:
                    path.unlink(missing_ok=True)

    # --- ssh ------------------------------------------------------------

    def _ssh_options(self):
        return ["-i", str(self.key), "-o", "BatchMode=yes",
                "-o", "StrictHostKeyChecking=no", "-o", "UserKnownHostsFile=/dev/null",
                "-o", "LogLevel=ERROR", "-o", "ConnectTimeout=5"]

    def ssh(self, command, check=True, timeout=600, background=False):
        """Run `command` in the guest's login shell; returns its stdout."""
        argv = ["ssh", *self._ssh_options(), "-p", str(self.port),
                f"{self.user}@127.0.0.1", command]
        if background:
            return subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    text=True)
        result = subprocess.run(argv, capture_output=True, text=True, timeout=timeout)
        if check and result.returncode:
            raise StepFailed(f"`{command}` exited with {result.returncode}:\n"
                             f"{result.stdout[-3000:]}{result.stderr[-3000:]}")
        return result.stdout

    def copy_to(self, source, destination):
        subprocess.run(["scp", *self._ssh_options(), "-P", str(self.port), "-r",
                        str(source), f"{self.user}@127.0.0.1:{destination}"],
                       check=True, capture_output=True, timeout=600)

    def copy_from(self, source, destination):
        result = subprocess.run(["scp", *self._ssh_options(), "-P", str(self.port), "-r",
                                 f"{self.user}@127.0.0.1:{source}", str(destination)],
                                capture_output=True, timeout=600)
        return result.returncode == 0

    def wait_ssh(self, timeout=600):
        def ready():
            if self.qemu.poll() is not None:
                raise StepFailed(f"QEMU exited with {self.qemu.returncode}; "
                                 f"see {self.logs / 'qemu.log'}")
            try:
                self.ssh("echo ok", timeout=20)
                return True
            except (StepFailed, subprocess.TimeoutExpired):
                return False
        wait_for("ssh in the guest", timeout, ready, interval=5)


class Qmp:
    """Minimal QMP client: commands, key taps and screenshots."""

    def __init__(self, path, process):
        self.path = path
        self.process = process
        self.sock = None
        self.buffer = b""

    def _connect(self):
        if self.sock:
            return
        def connect():
            if self.process.poll() is not None:
                raise StepFailed("QEMU exited before QMP was available")
            try:
                sock = socket.socket(socket.AF_UNIX)
                sock.connect(str(self.path))
                self.sock = sock
                return True
            except OSError:
                return False
        wait_for("QMP socket", 30, connect, interval=0.2)
        self.sock.settimeout(30)
        self._read()  # greeting
        self.execute("qmp_capabilities")

    def _read(self):
        while b"\n" not in self.buffer:
            chunk = self.sock.recv(65536)
            if not chunk:
                raise StepFailed("QMP connection closed")
            self.buffer += chunk
        line, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(line)

    def execute(self, command, **arguments):
        self._connect()
        message = {"execute": command}
        if arguments:
            message["arguments"] = arguments
        self.sock.sendall(json.dumps(message).encode() + b"\n")
        while True:
            reply = self._read()
            if "return" in reply:
                return reply["return"]
            if "error" in reply:
                raise StepFailed(f"QMP {command}: {reply['error'].get('desc')}")

    def _keys(self, keys, down):
        events = [{"type": "key", "data": {"down": down,
                                           "key": {"type": "qcode", "data": key}}}
                  for key in keys]
        self.execute("input-send-event", events=events)

    def tap(self, *chords, delay=0.08):
        """Tap QEMU qcodes; `shift-x` presses a chord. Pauses keep the
        guest's tap/hold timing unambiguous."""
        for chord in chords:
            keys = chord.split("-")
            for key in keys:
                self._keys([key], True)
                time.sleep(0.02)
            for key in reversed(keys):
                self._keys([key], False)
                time.sleep(0.02)
            time.sleep(delay)

    def screendump(self, path):
        try:
            self.execute("screendump", filename=str(path), format="png")
        except StepFailed:
            pass


def wait_for(what, timeout, probe, interval=0.5):
    deadline = time.monotonic() + timeout
    while True:
        value = probe()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise StepFailed(f"timed out waiting for {what}")
        time.sleep(interval)


class Report:
    """Named steps with their outcome; failures do not stop later steps
    unless the caller says so."""

    def __init__(self):
        self.steps = []

    def step(self, name, function, *arguments, fatal=False):
        print(f"{name:<28} ", end="", flush=True)
        try:
            detail = function(*arguments)
            self.steps.append((name, "passed", detail or ""))
            print("passed" + (f" ({detail})" if detail else ""))
            return True
        except (StepFailed, subprocess.SubprocessError, OSError) as error:
            self.steps.append((name, "FAILED", str(error).strip()))
            print(f"FAILED\n    {str(error).strip()[:2000]}")
            if fatal:
                raise
            return False

    def skip(self, name, reason):
        self.steps.append((name, "skipped", reason))
        print(f"{name:<28} skipped ({reason})")

    @property
    def failed(self):
        return sum(1 for _, result, _ in self.steps if result == "FAILED")

    def write(self, header, path):
        lines = list(header) + [""]
        for name, result, detail in self.steps:
            lines.append(f"- {name}: {result}" + (f" — {detail}" if detail else ""))
        passed = sum(1 for _, result, _ in self.steps if result == "passed")
        lines += ["", f"Passed: {passed}, failed: {self.failed}"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"\nPassed: {passed}, failed: {self.failed}\nSummary: {path}")


def result_path(platform, name):
    directory = REPO / "docs/testing/results" / platform
    base = f"{time.strftime('%F')}-vm-{name}"
    path = directory / f"{base}.txt"
    n = 2
    while path.exists():
        path = directory / f"{base}-{n}.txt"
        n += 1
    return path


def commit():
    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=REPO,
                          capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "diff", "--quiet", "HEAD"], cwd=REPO).returncode
    return f"{head} (with uncommitted changes)" if dirty else head




def environment_ok():
    missing = [tool for tool in ("qemu-system-x86_64", "qemu-img", "ssh", "scp")
               if not shutil.which(tool)]
    if missing:
        raise SystemExit("missing host tools: " + ", ".join(missing))
    if not os.access("/dev/kvm", os.R_OK | os.W_OK):
        raise SystemExit("/dev/kvm is not accessible to this user")
