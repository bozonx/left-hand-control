#!/usr/bin/env python3
"""Windows VM e2e: copy the working tree into a disposable overlay of the
prepared Windows 11 VM, run the selected suite in the interactive
session through the scheduled task LHC-E2E and collect the result.

The default smoke suite covers debug UI scenarios, IPC, popups and shutdown.
--suite full runs native tests, a release build and lifecycle/resource checks.
Both suites exercise the mapper through QEMU hardware keys and a native receiver.

Usage: run-windows.py --vm /mnt/disk2/vm/left-hand-control-windows [--show] [--keep]
See docs/development/guide.md, "E2E в виртуальных машинах".
"""

import argparse
import hashlib
import io
import json
import http.server
import secrets
import subprocess
import tarfile
import threading
import time

from windows_mapper import WindowsMapper, retry_guest_io
from windows_popups import check_popups
from vmlib import REPO, Report, StepFailed, Vm, commit, environment_ok, result_path, wait_for

WORK = r"D:\lhc-e2e"


class GuestCompletion:
    """Test-only callback over QEMU user networking when guest SSH is exhausted."""

    def __init__(self):
        self.result = None
        token = secrets.token_urlsafe(32)
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                if self.path != '/' + token:
                    self.send_error(403)
                    return
                try:
                    size = int(self.headers.get('Content-Length', '0'))
                    if not 0 < size <= 8 * 1024 * 1024:
                        raise ValueError('invalid result size')
                    result = json.loads(self.rfile.read(size))
                    if (not isinstance(result, dict) or result.get('status') not in ('0', '1')
                            or not isinstance(result.get('summary'), str) or not result['summary'].strip()
                            or not isinstance(result.get('full_log', ''), str)):
                        raise ValueError('invalid guest result')
                except (ValueError, TypeError):
                    self.send_error(400)
                    return
                if owner.result is None:
                    owner.result = result
                self.send_response(204)
                self.end_headers()

            def log_message(self, *_args):
                pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://10.0.2.2:{self.server.server_port}/{token}'

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


def source_archive(path):
    """Tracked and untracked-but-not-ignored files: the tree as it is now."""
    names = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=REPO, capture_output=True, check=True,
    ).stdout.decode().strip("\0").split("\0")
    with tarfile.open(path, "w:gz") as archive:
        for name in filter(None, names):
            if (REPO / name).is_file():
                archive.add(REPO / name, arcname=name, recursive=False)
    return sum(bool(name) and (REPO / name).is_file() for name in names), hashlib.sha256(path.read_bytes()).hexdigest()


def parse_guest_state(raw):
    if not raw.strip():
        return None
    try:
        state = json.loads(raw.lstrip("\ufeff"))
        if not isinstance(state, dict) or not all(key in state for key in ("done", "receiver", "task")):
            raise StepFailed(f"Incomplete guest state: {raw!r}")
        return state
    except ValueError as error:
        raise StepFailed(f"Invalid guest state: {raw!r}") from error


class GuestWatch:
    """Bounded remote polling, with recovery from a stalled SSH process."""

    def __init__(self, vm):
        self.vm = vm
        self.process = None
        self.started = 0

    def poll(self, input_checked):
        if self.process is None:
            after_input = "$true" if input_checked else "$false"
            command = (
                "$ErrorActionPreference = 'Stop'; "
                f"$afterInput = {after_input}; $began = Get-Date; "
                "while ($true) { "
                f"$done = Test-Path {WORK}\\out\\done.txt; "
                f"$receiver = Test-Path {WORK}\\out\\receiver\\snapshot.json; "
                "$task = [string](Get-ScheduledTask -TaskName LHC-E2E).State; "
                "if ($done -or (-not $afterInput -and $receiver) -or ((Get-Date) - $began).TotalSeconds -ge 30 -or "
                "($task -in @('Ready', 'Disabled') -and ((Get-Date) - $began).TotalSeconds -gt 30)) { "
                "[pscustomobject]@{done=$done; receiver=$receiver; task=$task} | ConvertTo-Json -Compress; break }; "
                "Start-Sleep -Seconds 2 }"
            )
            self.process = self.vm.ssh(command, background=True)
            self.started = time.monotonic()
        if self.process.poll() is None:
            if time.monotonic() - self.started > 90:
                print("Guest monitor stalled; reconnecting", flush=True)
                self.stop()
            return None
        process, self.process = self.process, None
        output, error = process.communicate()
        if process.returncode:
            print(f"Guest monitor disconnected; retrying: {error[-500:].strip()}", flush=True)
            return None
        return parse_guest_state(output)

    def stop(self):
        if self.process is not None:
            process, self.process = self.process, None
            try:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired) as error:
                print(f"Guest monitor cleanup failed: {error}", flush=True)
            finally:
                if process.stdout:
                    process.stdout.close()
                if process.stderr:
                    process.stderr.close()


def validate_guest_result(status, summary, input_checked):
    if status != "0" or not summary.strip() or "FAILED" in summary:
        raise StepFailed(summary or f"Guest exit status: {status}")
    if not input_checked:
        raise StepFailed("Guest completed without hardware/mapper acceptance")


def signal_input_result(ps, report, passed):
    try:
        retry_guest_io(lambda: ps(f"Set-Content {WORK}\\out\\input-result.txt -Value {0 if passed else 1}", timeout=30))
    except (StepFailed, subprocess.TimeoutExpired):
        if passed:
            raise
        # Keep the original scenario error when the failure marker cannot cross SSH.
        report.skip("signal-guest-input-failure", "SSH unavailable; original input failure preserved")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vm", required=True, help="VM directory with vm.json")
    parser.add_argument("--suite", choices=("smoke", "full", "mapper"), default="smoke",
                        help="smoke (default), full native/release acceptance, or mapper-only diagnostics")
    parser.add_argument("--memory", type=int, help="guest RAM in MiB; overrides vm.json for this run only")
    parser.add_argument("--mapper-from", help="mapper-only diagnostics: begin at this named scenario")
    parser.add_argument("--show", action="store_true", help="show the VM display")
    parser.add_argument("--keep", action="store_true", help="keep the overlay disk")
    parser.add_argument("--timeout", type=float, default=150,
                        help="minutes to wait for the guest run (the first build is slow)")
    args = parser.parse_args()
    if args.mapper_from and args.suite != "mapper":
        parser.error("--mapper-from requires --suite mapper")
    environment_ok()
    vm = Vm(args.vm)
    # A stalled Windows OpenSSH multiplexed connection also blocks log collection.
    vm.ssh_multiplex = False
    watch = GuestWatch(vm)
    if args.memory:
        if args.memory < 4096:
            parser.error("Windows acceptance needs at least 4096 MiB")
        vm.config["memory"] = args.memory
    report = Report()
    summary = io.StringIO()
    completion = None

    def ps(command, **kwargs):
        encoding = "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); "
        return vm.ssh(encoding + command, **kwargs)

    def boot():
        vm.start(show=args.show)
        vm.wait_ssh(timeout=900)
        # The cache disk survives even when a failed run could not export logs.
        if vm.copy_from("D:/lhc-e2e/out", vm.logs / "previous-guest"):
            print("Recovered previous guest logs from the cache disk.", flush=True)

    def preflight():
        wait_for("interactive desktop", 180, lambda: "True" in ps(
            "[bool](Get-Process explorer -ErrorAction SilentlyContinue)", timeout=30))
        return ps(
            "$ErrorActionPreference = 'Stop'; "
            "$volume = Get-Volume -DriveLetter D; "
            "if ($volume.FileSystemLabel -ne 'LHC-CACHE') { throw 'D: is not LHC-CACHE' }; "
            "$task = Get-ScheduledTask -TaskName LHC-E2E; "
            "if ($task.State -in @('Running', 'Disabled')) { throw 'LHC-E2E is running or disabled' }; "
            "if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { throw 'No desktop session' }; "
            "$env:Path = \"$env:USERPROFILE\\.cargo\\bin;$env:Path\"; "
            "& cargo --version; if ($LASTEXITCODE) { throw 'Cargo unavailable' }"
        ).strip()

    def upload():
        archive = vm.run_dir / "src.tar.gz"
        count, digest = source_archive(archive)
        ps(f"$ErrorActionPreference = 'Stop'; New-Item -ItemType Directory -Force -Path {WORK}\\src, {WORK}\\out | Out-Null")
        vm.copy_to(archive, "D:/lhc-e2e/src.tar.gz")
        # Preserve timestamps only when content is identical. Restamping the
        # whole tree rebuilds vendored Slint; keeping host timestamps can make
        # Cargo miss edits that predate the guest's cached build.
        synced = ps(f"""
$ErrorActionPreference = 'Stop'
$stage = '{WORK}\\incoming'
$source = '{WORK}\\src'
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $stage | Out-Null
tar -xmzf {WORK}\\src.tar.gz -C $stage
if ($LASTEXITCODE) {{ exit $LASTEXITCODE }}
$updated = 0
foreach ($file in Get-ChildItem $stage -Recurse -File) {{
    $relative = $file.FullName.Substring($stage.Length + 1)
    $destination = Join-Path $source $relative
    if ((Test-Path -LiteralPath $destination -PathType Leaf) -and
        (Get-FileHash -LiteralPath $file.FullName).Hash -eq (Get-FileHash -LiteralPath $destination).Hash) {{ continue }}
    New-Item -ItemType Directory -Force -Path (Split-Path $destination) | Out-Null
    Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
    $updated++
}}
$removed = 0
foreach ($entry in Get-ChildItem $source -Force | Where-Object Name -ne 'target') {{
    $oldFiles = if ($entry.PSIsContainer) {{ Get-ChildItem $entry.FullName -Recurse -File -Force }} else {{ @($entry) }}
    foreach ($file in $oldFiles) {{
        $relative = $file.FullName.Substring($source.Length + 1)
        if (-not (Test-Path -LiteralPath (Join-Path $stage $relative) -PathType Leaf)) {{
            Remove-Item -LiteralPath $file.FullName -Force
            $removed++
        }}
    }}
}}
Remove-Item $stage -Recurse -Force
"Source sync: $updated updated, $removed removed"
""", timeout=180)
        return f"{count} files; source archive {archive}; SHA256 {digest}; {synced.strip()}"

    def keyboard_baseline():
        """Real QEMU USB input into a native edit control, without a mapper."""
        probe = WindowsMapper(vm, ps, report)
        if args.suite != "mapper":
            wait_for("popup shell startup", 45, lambda: "True" in probe.ps(
                f"Test-Path {WORK}\\out\\popups\\ready.json", timeout=30))
        probe.reset()
        snapshot = f"{WORK}\\out\\receiver\\snapshot.json"

        def state():
            # Permit atomic snapshot replacement while the reader has it open.
            raw = ps(
                f"$stream = [IO.File]::Open('{snapshot}', [IO.FileMode]::Open, "
                "[IO.FileAccess]::Read, ([IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete)); "
                "$reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8); "
                "try { $reader.ReadToEnd() } finally { $reader.Dispose() }", timeout=30)
            try:
                return json.loads(raw.lstrip("\ufeff"))
            except ValueError as error:
                raise StepFailed(f"Invalid receiver snapshot: {error}") from error

        def focused():
            snapshot = state()
            return snapshot.get("form_active") and snapshot.get("editor_focused")

        wait_for("focused input receiver", 30, focused)
        for name, keys, expected in (
            ("native", ("a", "b", "c"), "abc"),
            ("shift", ("shift-x",), "abcX"),
            ("control-and-release", ("ctrl-home", "shift-end", "o", "k"), "ok"),
        ):
            vm.qmp.tap(*keys)
            wait_for(name, 20, lambda: state().get("text") == expected)
            vm.qmp.screendump(vm.logs / f"keyboard-{name}.png")
        def released():
            events = ps(f"Get-Content -Raw -Encoding utf8 {WORK}\\out\\receiver\\events.jsonl")
            try:
                records = [json.loads(line.lstrip("\ufeff"))
                           for line in events.splitlines() if line.strip()]
            except ValueError:
                return False  # A final event may still be appending.
            # Text alone would miss a lost key-up: include Shift and Control too.
            return all(any(row.get("event") == event and row.get("key_code") == key
                           for row in records)
                       for key in (16, 17, 35, 36, 65, 66, 67, 88, 79, 75)
                       for event in ("key-down", "key-up"))

        wait_for("receiver key-down/up records", 10, released)
        return "QEMU USB keyboard -> WinForms: abc, abcX, Ctrl+Home, Shift+End -> ok; key-down/up recorded"

    def run():
        nonlocal completion
        completion = GuestCompletion()
        ps(f"Set-Content {WORK}\\completion-url.txt -Value '{completion.url}'")
        ps(f"$ErrorActionPreference = 'Stop'; Remove-Item {WORK}\\out\\* -Recurse -Force -ErrorAction SilentlyContinue; "
           f"Set-Content {WORK}\\suite.txt -Value {args.suite}; "
           "Start-ScheduledTask -TaskName LHC-E2E")
        input_checked = False
        started = time.monotonic()

        def progress():
            nonlocal input_checked
            if vm.qemu.poll() is not None:
                raise StepFailed("QEMU exited before guest completion")
            if completion.result is not None:
                return True
            state = watch.poll(input_checked)
            if state is None:
                return False
            if state["done"]:
                return True
            if state["receiver"] and not input_checked:
                input_checked = True
                passed = False
                try:
                    report.step("hardware-keyboard", keyboard_baseline, fatal=True)
                    if args.suite != "mapper":
                        check_popups(vm, ps, report)
                    WindowsMapper(vm, ps, report).run(start_at=args.mapper_from)
                    passed = True
                finally:
                    signal_input_result(ps, report, passed)
            if time.monotonic() - started > 30 and state["task"] in ("Ready", "Disabled"):
                raise StepFailed(f"LHC-E2E stopped without a completion marker ({state['task']})")
            return False

        wait_for("the guest run", args.timeout * 60, progress, interval=10)
        if completion.result is not None:
            result = completion.result
            summary.write(result['summary'])
            status = result['status']
            (vm.logs / 'guest-completion.json').write_text(json.dumps(result), encoding='utf-8')
            (vm.logs / 'full-callback.log').write_text(result.get('full_log', ''), encoding='utf-8')
        else:
            summary.write(ps(f"Get-Content -Raw -Encoding utf8 {WORK}\\out\\summary.txt"))
            status = ps(f"Get-Content -Raw {WORK}\\out\\done.txt").strip()
        validate_guest_result(status, summary.getvalue(), input_checked)

    try:
        report.step("boot", boot, fatal=True)
        report.step("preflight", preflight, fatal=True)
        report.step("upload-source", upload, fatal=True)
        report.step(f"windows-{args.suite}", run)
    except StepFailed:
        print("A required step failed; later steps were not run.")
    except KeyboardInterrupt:
        report.steps.append(("controller", "FAILED", "interrupted; full acceptance was not completed"))
        print("Interrupted; collecting available diagnostics and stopping the VM.")
    finally:
        watch.stop()
        if completion is not None:
            completion.close()
        try:
            if vm.qemu and vm.qemu.poll() is None:
                if not vm.copy_from("D:/lhc-e2e/out", vm.logs / "guest"):
                    report.skip("collect-guest-logs", "SSH export failed; logs remain on the persistent cache disk")
                vm.qmp.screendump(vm.logs / "final.png")
        except (StepFailed, OSError):
            pass
        vm.stop(keep=args.keep or bool(report.failed))

    header = [
        f"Date: {time.strftime('%F %T %z')}",
        f"Commit: {commit()}",
        f"VM: {vm.name} ({vm.base})",
        f"Suite: {args.suite}",
        f"Mapper build: {'release' if args.suite == 'full' else 'debug'}",
        f"Popup shell build: {'none' if args.suite == 'mapper' else 'release' if args.suite == 'full' else 'debug'}",
        f"Mapper from: {args.mapper_from or 'beginning'}",
        f"Memory: {vm.get('memory', 6144)} MiB",
        f"Logs: {vm.logs}",
        "",
        summary.getvalue().strip(),
    ]
    report.write(header, result_path("windows", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
