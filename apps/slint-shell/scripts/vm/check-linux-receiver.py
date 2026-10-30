#!/usr/bin/env python3
"""VM component check: QMP keyboard -> native dialog -> exact JSON result.

No application package is installed. Use this small check when changing the
receiver or its cleanup, independently of full product/packaging acceptance.
"""

import argparse
from pathlib import Path
import time
import subprocess

from vmlib import Report, StepFailed, Vm, environment_ok, receiver_text, result_path


class ReceiverProbe:
    SCENARIOS = ("native-text", "accepted-empty", "cancelled-is-failure",
                 "timeout-is-failure", "after-timeout-recovery")

    def __init__(self, vm):
        self.vm = vm

    def guest(self, command):
        return self.vm.ssh("python3 ~/lhc-e2e/linux-guest.py " + command)

    def expect_text(self, name, expected, *keys):
        self.guest("start-receiver --name " + name)
        self.vm.qmp.tap(*keys, "ret")
        text = receiver_text(self.guest("receiver-result --name " + name))
        if text != expected:
            raise StepFailed(f"expected {expected!r}, received {text!r}")
        return repr(text)

    def expect_failure(self, name, expected, *keys):
        self.guest("start-receiver --name " + name)
        self.vm.qmp.tap(*keys)
        try:
            self.guest("receiver-result --name " + name + " --timeout 1")
        except StepFailed as error:
            if "exited with 1:" not in str(error) or expected not in str(error):
                raise
            return expected
        raise StepFailed("receiver failure was silently accepted")

    def run(self, report):
        report.require(self.SCENARIOS)
        report.step("native-text", self.expect_text, "native", "abc", "a", "b", "c")
        report.step("accepted-empty", self.expect_text, "empty", "")
        report.step("cancelled-is-failure", self.expect_failure, "cancelled", "receiver failed:", "esc")
        report.step("timeout-is-failure", self.expect_failure, "timeout", "receiver timed out without an accepted result")
        report.step("after-timeout-recovery", self.expect_text, "after-timeout", "c", "c")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vm", required=True, help="prepared KDE VM with kdialog")
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    environment_ok()
    vm = Vm(args.vm)
    report = Report()
    report.require(("boot", *ReceiverProbe.SCENARIOS))

    def boot():
        vm.start()
        vm.wait_ssh()
        vm.ssh("mkdir -p ~/lhc-e2e")
        for name in ("linux-guest.py", "text_corpus.py"):
            vm.copy_to(Path(__file__).with_name(name), "lhc-e2e/")
        ReceiverProbe(vm).guest("wait-session")

    try:
        report.step("boot", boot, fatal=True)
        ReceiverProbe(vm).run(report)
    except (StepFailed, KeyboardInterrupt) as error:
        if isinstance(error, KeyboardInterrupt):
            report.steps.append(("controller", "FAILED", "interrupted"))
    finally:
        try:
            if vm.qemu and vm.qemu.poll() is None:
                vm.qmp.screendump(vm.logs / "final.png")
                if not vm.copy_from("lhc-e2e/receiver.log", vm.logs / "receiver.log"):
                    report.skip("receiver-log", "file unavailable")
        except (StepFailed, subprocess.SubprocessError, OSError) as error:
            report.skip("diagnostics", str(error))
        finally:
            vm.stop(keep=args.keep or report.unsuccessful)
    report.write([f"Date: {time.strftime('%F %T %z')}", f"VM: {vm.name}",
                  "Scope: native receiver protocol/cleanup only; no application package or product acceptance",
                  f"Logs: {vm.logs}"], result_path("linux", vm.name + "-receiver"))
    raise SystemExit(1 if report.unsuccessful else 0)


if __name__ == "__main__":
    main()
