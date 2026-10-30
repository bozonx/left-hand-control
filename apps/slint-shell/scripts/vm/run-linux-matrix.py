#!/usr/bin/env python3
"""Strict KDE acceptance: DEB and RPM, both text backends and all popup modes.

Every cell uses a fresh overlay and requires suspend, exact input, lifecycle and
at least three minutes of stress. This is Linux desktop acceptance, not Windows
acceptance or a release publication gate.
"""

import argparse
from pathlib import Path
import shlex
import subprocess
import sys
import time

from vmlib import REPO, Report, StepFailed, commit, result_path


def matrix(args):
    if args.first_release and (args.previous_deb or args.previous_rpm):
        raise StepFailed("--first-release excludes previous packages")
    if not args.first_release and not (args.previous_deb and args.previous_rpm):
        raise StepFailed("both --previous-deb and --previous-rpm are required")
    if args.stress_seconds < 180:
        raise StepFailed("matrix requires --stress-seconds >= 180")
    if args.deb_vm.resolve() == args.rpm_vm.resolve():
        raise StepFailed("DEB and RPM acceptance require distinct prepared VM directories")
    commands = []
    for kind in ("deb", "rpm"):
        package = getattr(args, kind + "_package")
        if package.suffix != "." + kind:
            raise StepFailed(f"{kind} VM needs a .{kind} package")
        for popup in ("winit", "auto", "spell"):
            for backend in ("libei", "clipboard"):
                command = [sys.executable, str(Path(__file__).with_name("run-linux.py")),
                           "--vm", str(getattr(args, kind + "_vm").resolve()),
                           "--package", str(package.resolve()), "--acceptance",
                           "--popup-mode", popup, "--text-backend", backend,
                           "--stress-seconds", str(args.stress_seconds)]
                if args.first_release:
                    command.append("--first-release")
                else:
                    command += ["--previous", str(getattr(args, "previous_" + kind).resolve())]
                commands.append((f"{kind}-{popup}-{backend}", command))
    return commands


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for kind in ("deb", "rpm"):
        parser.add_argument(f"--{kind}-vm", required=True, type=Path)
        parser.add_argument(f"--{kind}-package", required=True, type=Path)
        parser.add_argument(f"--previous-{kind}", type=Path)
    parser.add_argument("--first-release", action="store_true")
    parser.add_argument("--stress-seconds", type=int, default=180)
    parser.add_argument("--list", action="store_true", help="print commands without running or claiming acceptance")
    args = parser.parse_args()
    try:
        commands = matrix(args)
    except StepFailed as error:
        parser.error(str(error))
    if args.list:
        for name, command in commands:
            print(name + ": " + shlex.join(command))
        return
    for path in (args.deb_vm / "vm.json", args.rpm_vm / "vm.json", args.deb_package,
                 args.rpm_package, args.previous_deb, args.previous_rpm):
        if path is not None and not path.is_file():
            parser.error(f"required fixture not found: {path}")
    report = Report()
    report.require(name for name, _ in commands)
    try:
        for name, command in commands:
            def run(command=command):
                result = subprocess.run(command, cwd=REPO)
                if result.returncode:
                    raise StepFailed(f"exit {result.returncode}: {shlex.join(command)}")
                return shlex.join(command)
            report.step(name, run)
    except KeyboardInterrupt:
        report.steps.append(("controller", "FAILED", "matrix interrupted"))
    report.write([f"Date: {time.strftime('%F %T %z')}", f"Commit: {commit()}",
                  "Scope: KDE Linux DEB/RPM × winit/auto/spell × libei/clipboard (12 required cells)",
                  "Windows, clean Windows ZIP and hosted release pipeline are outside this matrix."],
                 result_path("linux", "acceptance-matrix"))
    raise SystemExit(1 if report.unsuccessful else 0)


if __name__ == "__main__":
    main()
