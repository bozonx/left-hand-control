#!/usr/bin/env python3
"""Windows VM e2e: copy the working tree into a disposable overlay of the
prepared Windows 11 VM, run check-windows-automated.ps1 in the interactive
session through the scheduled task LHC-E2E and collect the result.

The Windows mapper is not implemented, so this covers the shell: native tests,
release build, the portable interaction scenario, IPC lifecycle, popups,
hotkey registration and resource bounds.

Usage: run-windows.py --vm /mnt/disk2/vm/left-hand-control-windows [--show] [--keep]
See docs/development/guide.md, "E2E в виртуальных машинах".
"""

import argparse
import io
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
        return vm.ssh(command, **kwargs)

    def boot():
        vm.start(show=args.show)
        vm.wait_ssh(timeout=900)

    def upload():
        archive = vm.run_dir / "src.tar.gz"
        count = source_archive(archive)
        vm.copy_to(archive, "D:/lhc-e2e/src.tar.gz")
        # Keep target\ between runs: the cache disk makes rebuilds incremental.
        ps(f"Get-ChildItem {WORK}\\src -Force | Where-Object Name -ne 'target' | "
           f"Remove-Item -Recurse -Force; tar -xzf {WORK}\\src.tar.gz -C {WORK}\\src; "
           "if ($LASTEXITCODE) { exit $LASTEXITCODE }")
        return f"{count} files"

    def run():
        ps(f"Remove-Item {WORK}\\out\\done.txt -ErrorAction SilentlyContinue; "
           "Start-ScheduledTask -TaskName LHC-E2E")
        def done():
            try:
                return "True" in ps(f"Test-Path {WORK}\\out\\done.txt", timeout=30)
            except (StepFailed, subprocess.TimeoutExpired):
                return False
        wait_for("the guest run", args.timeout * 60, done, interval=20)
        summary.write(ps(f"Get-Content -Raw -Encoding utf8 {WORK}\\out\\summary.txt"))
        if "FAILED" in summary.getvalue():
            raise StepFailed(summary.getvalue())

    try:
        report.step("boot", boot, fatal=True)
        report.step("upload-source", upload, fatal=True)
        report.step("windows-automated", run)
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
        f"Logs: {vm.logs}",
        "",
        summary.getvalue().strip(),
    ]
    report.write(header, result_path("windows", vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == "__main__":
    main()
