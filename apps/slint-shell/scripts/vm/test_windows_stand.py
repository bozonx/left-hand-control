"""Host-side regressions: exact source provenance and cleanup on failed input."""

import contextlib
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
import urllib.error
import urllib.request
import json
from unittest.mock import Mock, patch

import importlib.util
from vmlib import Report, StepFailed, Vm
from windows_mapper import WindowsMapper, retry_guest_io
import windows_popups

spec = importlib.util.spec_from_file_location("run_windows", Path(__file__).with_name("run-windows.py"))
run_windows = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run_windows)


class WindowsStandTests(unittest.TestCase):
    def test_completion_callback_requires_token_and_valid_result(self):
        callback = run_windows.GuestCompletion()
        url = callback.url.replace('10.0.2.2', '127.0.0.1')
        try:
            for target, body, expected in ((url + '-wrong', b'{}', 403),
                                           (url, b'{"status":"0"}', 400)):
                with self.assertRaises(urllib.error.HTTPError) as raised:
                    urllib.request.urlopen(urllib.request.Request(target, data=body), timeout=5)
                self.assertEqual(raised.exception.code, expected)
                raised.exception.close()
                self.assertIsNone(callback.result)
            result = {'status': '0', 'summary': 'Windows: passed', 'full_log': 'tests passed'}
            with urllib.request.urlopen(urllib.request.Request(url, data=json.dumps(result).encode()), timeout=5) as response:
                self.assertEqual(response.status, 204)
            self.assertEqual(callback.result, result)
        finally:
            callback.close()

    def test_background_ssh_does_not_inherit_controller_stdin(self):
        vm = object.__new__(Vm)
        vm.port = 2241
        vm.user = 'test'
        with patch.object(Vm, '_ssh_options', return_value=[]), \
             patch('vmlib.subprocess.Popen') as popen:
            vm.ssh('echo ready', background=True)
        self.assertEqual(popen.call_args.kwargs['stdin'], subprocess.DEVNULL)

    def test_watch_recycles_a_stalled_remote_process(self):
        vm = Mock()
        process = vm.ssh.return_value
        process.poll.return_value = None
        watch = run_windows.GuestWatch(vm)
        with patch.object(run_windows.time, 'monotonic', side_effect=[0, 0, 91]), \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertIsNone(watch.poll(False))
            self.assertIsNone(watch.poll(False))
        process.terminate.assert_called_once()
        self.assertIsNone(watch.process)

    def test_build_watch_reuses_one_remote_process_until_the_phase_is_ready(self):
        vm = Mock()
        process = vm.ssh.return_value
        process.poll.side_effect = [None, None, 0]
        process.returncode = 0
        process.communicate.return_value = ('{"done":false,"receiver":true,"task":"Running"}', '')
        watch = run_windows.GuestWatch(vm)
        self.assertIsNone(watch.poll(False))
        self.assertIsNone(watch.poll(False))
        self.assertTrue(watch.poll(False)['receiver'])
        vm.ssh.assert_called_once()
        self.assertIn('$afterInput = $false', vm.ssh.call_args.args[0])
        self.assertTrue(vm.ssh.call_args.kwargs['background'])
        self.assertIsNone(watch.process)

    def test_watch_reconnects_after_transport_loss_and_closes_an_unfinished_phase(self):
        vm = Mock()
        failed = Mock()
        failed.poll.return_value = 255
        failed.returncode = 255
        failed.communicate.return_value = ('', 'connection reset')
        pending = Mock()
        pending.poll.return_value = None
        pending.wait.side_effect = [subprocess.TimeoutExpired('ssh', 5), 0]
        vm.ssh.side_effect = [failed, pending]
        watch = run_windows.GuestWatch(vm)
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertIsNone(watch.poll(True))
        self.assertIsNone(watch.poll(True))
        self.assertIn('$afterInput = $true', vm.ssh.call_args.args[0])
        watch.stop()
        pending.terminate.assert_called_once()
        pending.kill.assert_called_once()
        self.assertEqual(pending.wait.call_count, 2)
        pending.stdout.close.assert_called_once()
        pending.stderr.close.assert_called_once()
        self.assertIsNone(watch.process)

    def test_popup_diagnostic_failures_preserve_the_original_error(self):
        vm = Mock()
        vm.logs = Path('test-logs')
        vm.qmp.screendump.side_effect = OSError('QMP unavailable')
        probe = Mock()
        probe.ps.return_value = 'True'
        probe.reset.side_effect = StepFailed('original popup mismatch')
        probe.read.side_effect = [json.dumps({'profile': 'test', 'port': 1234, 'exe': 'lhc', 'pid': 1}), StepFailed('SSH unavailable during cleanup')]
        with patch.object(windows_popups, 'WindowsMapper', return_value=probe), \
             contextlib.redirect_stdout(io.StringIO()), \
             self.assertRaisesRegex(StepFailed, 'original popup mismatch'):
            windows_popups.check_popups(vm, Mock(), Report())

    def test_successful_popups_still_fail_when_cleanup_fails(self):
        vm = Mock()
        vm.logs = Path('test-logs')
        probe = Mock()
        probe.ps.return_value = 'True'
        probe.read.side_effect = StepFailed('quit transport failed')
        report = Mock()
        # Successful scenario execution is covered by the VM; isolate final cleanup.
        with patch.object(windows_popups, 'WindowsMapper', return_value=probe), \
             self.assertRaisesRegex(StepFailed, 'quit transport failed'):
            windows_popups.check_popups(vm, Mock(), report)
        self.assertEqual(report.step.call_count, 7)

    def test_interrupted_run_preserves_diagnostics_and_records_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            report_path = Path(temporary) / 'report.txt'
            vm = Mock()
            vm.name = 'test-vm'
            vm.base = Path(temporary) / 'base.qcow2'
            vm.logs = Path(temporary) / 'logs'
            vm.qemu = None
            vm.get.return_value = 8192
            vm.start.side_effect = KeyboardInterrupt
            with patch.object(run_windows, 'Vm', return_value=vm), \
                 patch.object(run_windows, 'environment_ok'), \
                 patch.object(run_windows, 'commit', return_value='test'), \
                 patch.object(run_windows, 'result_path', return_value=report_path), \
                 patch('sys.argv', ['run-windows.py', '--vm', temporary, '--suite', 'full']), \
                 contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as raised:
                run_windows.main()
            self.assertEqual(raised.exception.code, 1)
            vm.stop.assert_called_once_with(keep=True)
            self.assertIn('controller: FAILED', report_path.read_text())
            self.assertIn('full acceptance was not completed', report_path.read_text())

    def test_idempotent_io_retries_transport_failures_but_not_guest_errors(self):
        operation = Mock(side_effect=[StepFailed('ssh exited with 255: timed out'),
                                      subprocess.TimeoutExpired('ssh', 30), 'response'])
        with patch('windows_mapper.time.sleep'):
            self.assertEqual(retry_guest_io(operation), 'response')
        self.assertEqual(operation.call_count, 3)
        operation = Mock(side_effect=StepFailed('PowerShell exited with 1: access denied'))
        with self.assertRaisesRegex(StepFailed, 'access denied'):
            retry_guest_io(operation)
        self.assertEqual(operation.call_count, 1)

    def test_failed_result_marker_does_not_hide_scenario_failure(self):
        ps = Mock(side_effect=StepFailed('ssh exited with 255: timed out'))
        report = Report()
        with patch('windows_mapper.time.sleep'), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(StepFailed, 'original scenario'):
                try:
                    raise StepFailed('original scenario')
                finally:
                    run_windows.signal_input_result(ps, report, False)
            with self.assertRaisesRegex(StepFailed, 'exited with 255'):
                run_windows.signal_input_result(ps, report, True)

    def test_source_archive_preserves_dirty_and_untracked_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / ".gitignore").write_text("cache\n")
            (root / "tracked").write_text("before")
            subprocess.run(["git", "-C", str(root), "add", ".gitignore", "tracked"], check=True)
            (root / "tracked").write_text("after")
            (root / "new").write_text("untracked")
            (root / "cache").write_text("ignored")
            archive = root / "cache"
            with patch.object(run_windows, "REPO", root):
                count, digest = run_windows.source_archive(archive)
            self.assertEqual(count, 3)
            self.assertEqual(digest, hashlib.sha256(archive.read_bytes()).hexdigest())
            with tarfile.open(archive) as source:
                self.assertEqual(set(source.getnames()), {".gitignore", "tracked", "new"})
                self.assertEqual(source.extractfile("tracked").read(), b"after")
                self.assertEqual(source.extractfile("new").read(), b"untracked")

    def test_guest_state_rejects_incomplete_or_invalid_results(self):
        for raw in ('', '  '):
            self.assertIsNone(run_windows.parse_guest_state(raw))
        for raw in ('broken JSON', '{}', '[]'):
            with self.assertRaises(StepFailed):
                run_windows.parse_guest_state(raw)
        self.assertTrue(run_windows.parse_guest_state('\ufeff{"done":false,"receiver":true,"task":"Running"}')["receiver"])

    def test_guest_failure_is_reported_before_missing_hardware_step(self):
        with self.assertRaisesRegex(StepFailed, "native tests FAILED"):
            run_windows.validate_guest_result("1", "native tests FAILED", False)
        with self.assertRaisesRegex(StepFailed, "without hardware"):
            run_windows.validate_guest_result("0", "smoke passed", False)
        run_windows.validate_guest_result("0", "smoke passed", True)

    def test_failed_scenario_stops_mapper_and_releases_physical_keys(self):
        order = []
        vm = Mock()
        vm.qmp.release.side_effect = lambda key: order.append(("release", key))
        report = Report()
        driver = WindowsMapper(vm, Mock(return_value="True"), report)
        driver.held.add("f")
        driver.mapper = Mock(side_effect=lambda op, **_: order.append(("mapper", op)))
        def cleanup(target, op):
            order.append((target, op))
            return {"ok": True}
        driver.command = Mock(side_effect=cleanup)
        driver.passthrough = Mock(side_effect=StepFailed("receiver mismatch"))
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(StepFailed):
            driver.run()
        self.assertEqual(order, [("mapper", "start"), ("receiver", "trace"), ("mapper", "stop"), ("release", "f")])
        self.assertEqual(report.failed, 1)
        self.assertFalse(driver.held)
        vm.qmp.screendump.assert_not_called()

    def test_cleanup_transport_failure_keeps_the_original_scenario_error(self):
        vm = Mock()
        driver = WindowsMapper(vm, Mock(return_value='True'), Report())
        driver.held.add('f')
        driver.mapper = Mock()
        driver.receiver = Mock()
        driver.passthrough = Mock(side_effect=StepFailed('original mismatch'))
        driver.command = Mock(side_effect=StepFailed('ssh unavailable during cleanup'))
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(StepFailed, 'original mismatch'):
            driver.run()
        vm.qmp.release.assert_called_once_with('f')


if __name__ == "__main__":
    unittest.main()
