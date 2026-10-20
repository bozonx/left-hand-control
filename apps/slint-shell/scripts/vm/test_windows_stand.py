"""Host-side regressions: exact source provenance and cleanup on failed input."""

import contextlib
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch

import importlib.util
from vmlib import Report, StepFailed
from windows_mapper import WindowsMapper

spec = importlib.util.spec_from_file_location("run_windows", Path(__file__).with_name("run-windows.py"))
run_windows = importlib.util.module_from_spec(spec)
spec.loader.exec_module(run_windows)


class WindowsStandTests(unittest.TestCase):
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

    def test_busy_ssh_poll_is_retried_without_losing_guest_result(self):
        ps = Mock(side_effect=[subprocess.TimeoutExpired("ssh", 30),
                               StepFailed("connection reset"), "", "  ",
                               '{"done":false,"receiver":true,"task":"Running"}'])
        self.assertIsNone(run_windows.guest_state(ps))
        self.assertIsNone(run_windows.guest_state(ps))
        self.assertIsNone(run_windows.guest_state(ps))
        self.assertIsNone(run_windows.guest_state(ps))
        self.assertTrue(run_windows.guest_state(ps)["receiver"])
        self.assertEqual(ps.call_count, 5)

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


if __name__ == "__main__":
    unittest.main()
