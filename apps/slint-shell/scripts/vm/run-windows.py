#!/usr/bin/env python3
"""Windows VM e2e: copy the working tree into a disposable overlay of the
prepared Windows 11 VM, run the selected suite in the interactive
session through the scheduled task LHC-E2E and collect the result.

The default smoke suite covers debug UI scenarios, IPC, popups and shutdown.
--suite full runs native tests, a release build and lifecycle/resource checks.
The Windows mapper is not implemented.

Usage: run-windows.py --vm /mnt/disk2/vm/left-hand-control-windows [--show] [--keep]
See docs/development/guide.md, "E2E в виртуальных машинах".
"""

import argparse
import io
import json
import subprocess
import tarfile
import time

from vmlib import REPO, Report, StepFailed, Vm, commit, environment_ok, result_path, wait_for

WORK = r"D:\lhc-e2e"


def source_archive(path):
    """Tracked and untracked-but-not-ignored files: the tree as it is now."""
    names = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=REPO, capture_output=True, check=True,
    ).stdout.decode().split("\0")
    with tarfile.open(path, "w:gz") as archive:
        for name in filter(None, names):
            if (REPO / name).is_file():
                archive.add(REPO / name, arcname=name, recursive=False)
    return len(names)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--vm", required=True, help="VM directory with vm.json")
    parser.add_argument("--suite", choices=("smoke", "full"), default="smoke",
                        help="smoke (default) or full native/release acceptance")
    parser.add_argument("--show", action="store_true", help="show the VM display")
    parser.add_argument("--keep", action="store_true", help="keep the overlay disk")
    parser.add_argument("--timeout", type=float, default=150,
                        help="minutes to wait for the guest run (the first build is slow)")
    args = parser.parse_args()
    environment_ok()
    vm = Vm(args.vm)
    report = Report()
    summary = io.StringIO()

    def ps(command, **kwargs):
        encoding = "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); "
        return vm.ssh(encoding + command, **kwargs)

    def boot():
        vm.start(show=args.show)
        vm.wait_ssh(timeout=900)

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
        count = source_archive(archive)
        ps(f"$ErrorActionPreference = 'Stop'; New-Item -ItemType Directory -Force -Path {WORK}\\src, {WORK}\\out | Out-Null")
        vm.copy_to(archive, "D:/lhc-e2e/src.tar.gz")
        # Keep target\ between runs: the cache disk makes rebuilds incremental.
        ps(f"$ErrorActionPreference = 'Stop'; Get-ChildItem {WORK}\\src -Force | Where-Object Name -ne 'target' | "
           f"Remove-Item -Recurse -Force; tar -xzf {WORK}\\src.tar.gz -C {WORK}\\src; "
           "if ($LASTEXITCODE) { exit $LASTEXITCODE }")
        return f"{count} files"

    def keyboard_baseline():
        """Real QEMU USB input into a native edit control, without a mapper."""
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
        ps(f"$ErrorActionPreference = 'Stop'; Remove-Item {WORK}\\out\\done.txt -ErrorAction SilentlyContinue; "
           f"Set-Content {WORK}\\suite.txt -Value {args.suite}; "
           "Start-ScheduledTask -TaskName LHC-E2E")
        def done():
            try:
                return "True" in ps(f"Test-Path {WORK}\\out\\done.txt", timeout=30)
            except (StepFailed, subprocess.TimeoutExpired):
                return False
        input_checked = False
        checked_task = time.monotonic()

        def progress():
            nonlocal input_checked, checked_task
            if done():
                return True
            if args.suite == "smoke" and not input_checked:
                ready = ps(f"Test-Path {WORK}\\out\\receiver\\snapshot.json", timeout=30)
                if "True" in ready:
                    input_checked = True
                    passed = False
                    try:
                        report.step("hardware-keyboard", keyboard_baseline, fatal=True)
                        passed = True
                    finally:
                        ps(f"Set-Content {WORK}\\out\\input-result.txt -Value {0 if passed else 1}")
            if time.monotonic() - checked_task > 30:
                checked_task = time.monotonic()
                task = ps("$task = Get-ScheduledTask -TaskName LHC-E2E; "
                          "$info = Get-ScheduledTaskInfo -TaskName LHC-E2E; "
                          '\"$($task.State):$($info.LastTaskResult)\"').strip()
                if task.startswith(("Ready:", "Disabled:")) and not done():
                    raise StepFailed(f"LHC-E2E stopped without a completion marker ({task})")
            return False

        wait_for("the guest run", args.timeout * 60, progress, interval=5)
        summary.write(ps(f"Get-Content -Raw -Encoding utf8 {WORK}\\out\\summary.txt"))
        status = ps(f"Get-Content -Raw {WORK}\\out\\done.txt").strip()
        if status != "0" or not summary.getvalue().strip() or "FAILED" in summary.getvalue():
            raise StepFailed(summary.getvalue())

    try:
        report.step("boot", boot, fatal=True)
        report.step("preflight", preflight, fatal=True)
        report.step("upload-source", upload, fatal=True)
        report.step(f"windows-{args.suite}", run)
    except StepFailed:
        print("A required step failed; later steps were not run.")
    finally:
        try:
            if vm.qemu and vm.qemu.poll() is None:
                vm.copy_from("D:/lhc-e2e/out", vm.logs / "guest")
                vm.qmp.screendump(vm.logs / "final.png")
        except (StepFailed, OSError):
            pass
        vm.stop(keep=args.keep)

    header = [
        f"Date: {time.strftime('%F %T %z')}",
        f"Commit: {commit()}",
        f"VM: {vm.name} ({vm.base})",
        f"Suite: {args.suite}",
        f"Logs: {vm.logs}",
        "",
        summary.getvalue().strip(),
    ]
    report.write(header, result_path("windows", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
