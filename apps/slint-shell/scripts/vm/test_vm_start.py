"""A failed QEMU launch must not let SSH tests run against an older guest."""

import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from vmlib import StepFailed, Vm


class VmStartTests(unittest.TestCase):
    def test_start_rejects_exited_qemu_before_ssh_can_reach_another_guest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "base.qcow2").touch()
            (root / "vm.json").write_text(json.dumps({
                "user": "tester", "ssh_key": "unused", "machine": "pc-i440fx-11.1",
            }))
            vm = Vm(root)
            process = Mock(returncode=1)
            process.poll.return_value = 1
            try:
                with patch("vmlib.subprocess.run"), \
                     patch("vmlib.subprocess.Popen", return_value=process), \
                     patch.object(vm, "ssh", return_value="ok") as ssh:
                    with self.assertRaisesRegex(StepFailed, "QEMU exited before QMP"):
                        vm.start()
                    ssh.assert_not_called()
            finally:
                vm.stop(keep=True)


if __name__ == "__main__":
    unittest.main()
