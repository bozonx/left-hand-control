"""Receiver transport failures and incomplete desktop coverage must never pass."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
from types import SimpleNamespace
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

from vmlib import Report, StepFailed, receiver_text


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


guest = module("linux_guest", "linux-guest.py")
linux = module("linux_runner", "run-linux.py")
windows = module("windows_runner_profile", "run-windows.py")
matrix_runner = module("linux_matrix", "run-linux-matrix.py")
receiver_probe = module("receiver_probe", "check-linux-receiver.py")


class LinuxStandTests(unittest.TestCase):
    def test_portal_denial_stops_after_the_job_is_cancelled(self):
        click = Mock()
        visible = Mock(return_value=False)
        with patch.object(linux.time, "sleep"):
            linux.decline_portal_dialogs([[0.7, 0.6], [0.6, 0.6]], click,
                                         lambda: click.call_count == 1, visible)
        self.assertEqual(click.call_count, 1)
        visible.assert_not_called()

    def test_portal_denial_handles_a_second_dialog(self):
        click = Mock()
        with patch.object(linux.time, "sleep"):
            linux.decline_portal_dialogs([[0.7, 0.6], [0.6, 0.6]], click,
                                         lambda: click.call_count == 2, lambda: True)
        self.assertEqual(click.call_count, 2)

    def test_portal_denial_still_requires_a_cancelled_job(self):
        with patch.object(linux.time, "sleep"), patch.object(linux, "wait_for") as wait:
            def probe(what, timeout, callback):
                if not callback():
                    raise StepFailed("timed out: " + what)
            wait.side_effect = probe
            with self.assertRaisesRegex(StepFailed, "declined text job"):
                linux.decline_portal_dialogs([[0.7, 0.6]], Mock(), lambda: False, lambda: False)

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.work = patch.object(guest, "WORK", Path(self.tmp.name))
        self.work.start()
        self.addCleanup(self.work.stop)
        self.args = SimpleNamespace(name="probe", timeout=0, expect=None)

    def result(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            guest.cmd_receiver_result(self.args)
        return receiver_text(output.getvalue())

    def test_timeout_cannot_be_accepted_as_empty_input(self):
        with patch.object(guest, "unit_active", return_value=True), patch.object(guest.subprocess, "run") as run:
            with self.assertRaises(SystemExit):
                self.result()
            self.assertIn("stop", run.call_args.args[0])
            self.assertIn("--no-block", run.call_args.args[0])
            self.assertLessEqual(run.call_args.kwargs["timeout"], 5)

    def test_exited_receiver_without_result_fails(self):
        self.args.timeout = 5
        with patch.object(guest, "unit_active", return_value=False), self.assertRaises(SystemExit):
            self.result()

    def test_cancelled_receiver_does_not_pass_negative_input_assertion(self):
        guest.receiver_path("probe").write_text(json.dumps({"status": "failed", "text": "", "exit_code": 1}))
        with self.assertRaises(SystemExit):
            self.result()

    def test_empty_whitespace_and_unicode_are_exact_results(self):
        for text in ("", "  ", "\nhello\n\n", "Привет 👩‍💻\n"):
            guest.receiver_path("probe").write_text(json.dumps({"status": "accepted", "text": text, "exit_code": 0}))
            self.assertEqual(self.result(), text)

    def test_dialog_wrapper_removes_only_one_framing_newline(self):
        for raw, expected in ((b'\n', ''), (b'\nhello\n\n', '\nhello\n'), (b'  \n', '  ')):
            completed = subprocess.CompletedProcess([], 0, raw, b'')
            with patch.object(guest, "user_environment", return_value={}), patch.object(guest.subprocess, "run", return_value=completed):
                guest.cmd_receive(self.args)
            self.assertEqual(self.result(), expected)

    def test_wrapper_records_cancel_as_failure(self):
        completed = subprocess.CompletedProcess([], 1, b'', b'cancelled')
        with patch.object(guest, "user_environment", return_value={}), patch.object(guest.subprocess, "run", return_value=completed):
            guest.cmd_receive(self.args)
        with self.assertRaises(SystemExit):
            self.result()

    def test_application_resetting_the_fixture_is_an_explicit_setup_failure(self):
        with patch.object(guest, "CONFIG_DIR", Path(self.tmp.name)):
            config = Path(self.tmp.name) / "config.json"
            config.write_text('{"settings":{}}')
            with self.assertRaises(SystemExit):
                guest.cmd_check_profile(SimpleNamespace(device="/dev/test"))
            config.write_text(json.dumps({"settings": {"devices": {"keyboards": ["/dev/test"]},
                                                       "activeLayoutId": "user:E2E", "layoutMode": "manual"}}))
            with contextlib.redirect_stdout(io.StringIO()):
                guest.cmd_check_profile(SimpleNamespace(device="/dev/test"))

    def test_host_rejects_transport_noise_missing_and_malformed_results(self):
        for raw in ('', 'ssh failed', '{}', 'null', '[]', '{"status":"failed","text":""}',
                    '{"status":"accepted","text":null}', '{"status":"accepted","text":"","exit_code":1}',
                    '{"status":"accepted","text":"","exit_code":false}', '{"status":"accepted","text":""}'):
            with self.assertRaises(StepFailed):
                receiver_text(raw)

    def test_popup_ipc_ack_without_frame_or_focus_does_not_pass(self):
        with patch.object(guest, "app", return_value=SimpleNamespace(returncode=0)), \
             patch.object(guest, "metric_count", return_value=0), \
             patch.object(guest, "wait", side_effect=SystemExit(1)), \
             self.assertRaises(SystemExit):
            guest.cmd_popups(SimpleNamespace())

    def test_failed_prerequisite_blocks_without_running_scenario(self):
        report = Report()
        scenario = Mock()
        with contextlib.redirect_stdout(io.StringIO()):
            report.step("start", lambda: (_ for _ in ()).throw(StepFailed("start failed")))
            self.assertFalse(report.step("held-keys", scenario, requires=("start",)))
        scenario.assert_not_called()
        self.assertEqual(report.steps[-1][1], "BLOCKED")
        self.assertTrue(report.unsuccessful)

    def test_a_later_failed_precondition_invalidates_an_earlier_pass(self):
        report = Report()
        report.require(("start",))
        report.steps = [("start", "passed", ""), ("start", "FAILED", "restart failed")]
        self.assertFalse(report.passed("start"))
        self.assertEqual(report.incomplete, ["start"])

    def test_required_skipped_or_unreached_step_is_incomplete(self):
        for status in ("skipped", "BLOCKED", None):
            report = Report()
            report.require(("suspend",))
            if status:
                report.steps.append(("suspend", status, "unavailable"))
            with contextlib.redirect_stdout(io.StringIO()):
                path = Path(self.tmp.name) / "summary.txt"
                report.write([], path)
            self.assertTrue(report.unsuccessful)
            self.assertIn("Acceptance: INCOMPLETE", path.read_text())
            if status is None:
                self.assertIn("suspend: NOT RUN", path.read_text())

    def test_complete_profile_allows_only_explicit_nonrequired_skip(self):
        report = Report()
        report.require(("suspend",))
        with contextlib.redirect_stdout(io.StringIO()):
            report.step("suspend", lambda: "resumed")
            report.skip("physical-hardware", "VM scope")
        self.assertFalse(report.unsuccessful)

    def test_linux_acceptance_rejects_short_stress_missing_upgrade_and_no_s3(self):
        args = SimpleNamespace(acceptance=True, stress_seconds=180, previous=None, first_release=True)
        linux.validate_acceptance(args)
        with self.assertRaises(StepFailed):
            linux.validate_acceptance(args, "S3 not available")
        for field, value in (("stress_seconds", 179), ("first_release", False), ("previous", Path("old.deb"))):
            changed = SimpleNamespace(**{**vars(args), field: value})
            with self.assertRaises(StepFailed):
                linux.validate_acceptance(changed)

    def test_negative_receiver_probe_does_not_accept_transport_failure(self):
        probe = receiver_probe.ReceiverProbe(Mock())
        probe.guest = Mock(side_effect=["ready", StepFailed("ssh exited with 255")])
        with self.assertRaisesRegex(StepFailed, "ssh exited with 255"):
            probe.expect_failure("cancelled", "receiver failed:", "esc")
        probe.guest = Mock(side_effect=["ready", StepFailed("ssh exited with 255: receiver timed out")])
        with self.assertRaisesRegex(StepFailed, "ssh exited with 255"):
            probe.expect_failure("timeout", "receiver timed out")
        probe.guest = Mock(side_effect=["ready", StepFailed("guest exited with 1: receiver failed:")])
        self.assertEqual(probe.expect_failure("cancelled", "receiver failed:", "esc"), "receiver failed:")
        probe.guest = Mock(side_effect=["ready", "unexpected success"])
        with self.assertRaisesRegex(StepFailed, "silently accepted"):
            probe.expect_failure("timeout", "receiver timed out")

    def test_matrix_covers_every_backend_and_desktop_package_combination(self):
        args = SimpleNamespace(deb_vm=Path("debian"), rpm_vm=Path("fedora"),
                               deb_package=Path("new.deb"), rpm_package=Path("new.rpm"),
                               previous_deb=None, previous_rpm=None, first_release=True,
                               stress_seconds=180)
        commands = matrix_runner.matrix(args)
        self.assertEqual(len(commands), 12)
        self.assertEqual(len({name for name, _ in commands}), 12)
        for _, command in commands:
            self.assertIn("--acceptance", command)
            self.assertIn("--first-release", command)
        args.first_release = False
        with self.assertRaises(StepFailed):
            matrix_runner.matrix(args)
        args.previous_deb = Path("old.deb")
        with self.assertRaises(StepFailed):
            matrix_runner.matrix(args)
        args.previous_rpm = Path("old.rpm")
        self.assertEqual(len(matrix_runner.matrix(args)), 12)
        args.rpm_vm = args.deb_vm
        with self.assertRaises(StepFailed):
            matrix_runner.matrix(args)

    def test_windows_acceptance_rejects_smoke_and_diagnostic_filters(self):
        args = SimpleNamespace(acceptance=True, suite="full", extended=True,
                               mapper_from=None, mapper_only=None, stress_seconds=180,
                               previous_package=None, first_release=True)
        windows.validate_acceptance(args)
        for field, value in (("suite", "smoke"), ("extended", False), ("mapper_only", "mapper-remap"),
                             ("mapper_from", "mapper-remap"), ("stress_seconds", 179)):
            with self.assertRaises(StepFailed):
                windows.validate_acceptance(SimpleNamespace(**{**vars(args), field: value}))


if __name__ == "__main__":
    unittest.main()
