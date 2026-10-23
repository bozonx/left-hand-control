"""Reject false acceptance: wrong architecture, missing sources, unsafe manifests."""
import importlib.util
import io
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import Mock
import zipfile
import json
import urllib.request
import urllib.error

from vmlib import Qmp, StepFailed
from windows_mapper import WindowsMapper

spec = importlib.util.spec_from_file_location('artifact_gate', Path(__file__).parents[1] / 'check-release-artifacts.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


package_spec = importlib.util.spec_from_file_location('clean_package_runner', Path(__file__).with_name('run-windows-package.py'))
package_runner = importlib.util.module_from_spec(package_spec)
package_spec.loader.exec_module(package_runner)

windows_spec = importlib.util.spec_from_file_location('windows_runner', Path(__file__).with_name('run-windows.py'))
windows_runner = importlib.util.module_from_spec(windows_spec)
windows_spec.loader.exec_module(windows_runner)


class AutomationGates(unittest.TestCase):
    def test_guest_clock_does_not_change_an_already_synchronized_clock(self):
        from unittest.mock import patch
        ps = Mock(return_value='1000')
        with patch.object(windows_runner.time, 'time', return_value=1001):
            windows_runner.synchronize_clock(ps)
        self.assertEqual(ps.call_count, 2)
        self.assertTrue(all('Set-Date' not in call.args[0] for call in ps.call_args_list))

    def test_guest_clock_corrects_skew_and_rejects_failed_correction(self):
        from unittest.mock import patch
        for final, fails in (('1000', False), ('26200', True)):
            ps = Mock(side_effect=['26200', '', final])
            with patch.object(windows_runner.time, 'time', return_value=1000):
                if fails:
                    with self.assertRaisesRegex(StepFailed, 'not synchronized'):
                        windows_runner.synchronize_clock(ps)
                else:
                    windows_runner.synchronize_clock(ps)
            self.assertIn('FromUnixTimeSeconds(1000)', ps.call_args_list[1].args[0])

    def test_clean_guest_callback_rejects_unauthorized_and_false_completion(self):
        callback = package_runner.Callback()
        url = callback.url.replace('10.0.2.2', '127.0.0.1')
        try:
            for address, payload, code in ((url + '-wrong', {}, 403), (url, {'phase': 'complete', 'status': '0'}, 400)):
                with self.assertRaises(urllib.error.HTTPError) as raised:
                    urllib.request.urlopen(urllib.request.Request(address, data=json.dumps(payload).encode()), timeout=5)
                self.assertEqual(raised.exception.code, code)
                raised.exception.close()
            self.assertEqual(callback.events, [])
        finally:
            callback.close()

    def test_suspend_request_does_not_wait_for_ssh_after_guest_sleeps(self):
        probe = WindowsMapper(Mock(), Mock(return_value=""), Mock())
        probe.read = Mock()
        self.assertEqual(probe.command("receiver", "suspend", wait_response=False), 1)
        probe.read.assert_not_called()

    def test_unlock_does_not_type_credentials_into_an_already_unlocked_desktop(self):
        vm = Mock()
        probe = WindowsMapper(vm, Mock(), Mock())
        probe.receiver = Mock(return_value={"locked": False})
        probe.ps = Mock()
        probe.unlock()
        probe.ps.assert_not_called()
        vm.qmp.tap.assert_not_called()
        vm.qmp.type_ascii.assert_not_called()

    def test_unlock_stops_before_credentials_when_space_already_unlocked_session(self):
        vm = Mock()
        probe = WindowsMapper(vm, Mock(), Mock())
        probe.receiver = Mock(side_effect=[{"locked": True}, {"locked": True}, {"locked": False}])
        probe.ps = Mock(return_value="test-password")
        from unittest.mock import patch
        with patch('windows_mapper.time.sleep'):
            probe.unlock()
        vm.qmp.tap.assert_called_once_with("spc")
        vm.qmp.type_ascii.assert_not_called()

    def test_unavailable_s3_never_falls_back_to_hibernation(self):
        with tempfile.TemporaryDirectory() as tmp:
            vm = Mock(logs=Path(tmp))
            ps = Mock(side_effect=['123', 'The following sleep states are available: Hibernate\nThe following sleep states are not available: Standby (S3)'])
            probe = WindowsMapper(vm, ps, Mock())
            probe.config = Mock()
            probe.command = Mock()
            with self.assertRaisesRegex(StepFailed, 'does not expose S3'):
                probe.suspend_resume()
            probe.command.assert_not_called()

    def test_ascii_keyboard_rejects_unrepresentable_input_before_typing(self):
        qmp = object.__new__(Qmp)
        qmp.tap = Mock()
        with self.assertRaises(StepFailed):
            qmp.type_ascii('valid-prefix\n')
        qmp.tap.assert_not_called()
        qmp.type_ascii('Az09 :\\?')
        self.assertEqual(qmp.tap.call_args.args, ('shift-a', 'z', '0', '9', 'spc', 'shift-semicolon', 'backslash', 'shift-slash'))

    def test_checksum_manifest_rejects_path_traversal_and_duplicates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'SHA256SUMS').write_text('0' * 64 + '  ../escape\n')
            with self.assertRaisesRegex(ValueError, 'Unsafe'):
                gate.verify_manifest(root, 'SHA256SUMS')
            (root / 'file').write_text('ok')
            line = gate.digest(root / 'file') + '  file\n'
            (root / 'SHA256SUMS').write_text(line * 2)
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                gate.verify_manifest(root, 'SHA256SUMS')

    def test_checksum_does_not_replace_architecture_validation(self):
        data = bytearray(256)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 60, 128)
        data[128:132] = b'PE\0\0'
        struct.pack_into('<H', data, 132, 0x14c)
        with self.assertRaisesRegex(ValueError, 'not x64'):
            gate.verify_binary(data, windows=True)
        struct.pack_into('<H', data, 132, 0x8664)
        gate.verify_binary(data, windows=True)

    def test_assembled_distribution_requires_every_locked_registry_source(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            version = '1.0.0'
            prefix = 'left-hand-control-1.0.0'
            elf = bytearray(64)
            elf[:6] = b'\x7fELF\x02\x01'
            struct.pack_into('<H', elf, 18, 62)
            with tarfile.open(root / f'{prefix}-linux-x86_64.tar.gz', 'w:gz') as tf:
                for name, value in [('left-hand-control', elf), ('LICENSE', b'license'), ('install-linux.sh', b'installer')]:
                    member = tarfile.TarInfo('bundle/' + name)
                    member.size = len(value)
                    tf.addfile(member, io.BytesIO(value))
            (root / 'left-hand-control_1.0.0_amd64.deb').write_bytes(b'!<arch>\n')
            (root / f'{prefix}-1.x86_64.rpm').write_bytes(b'\xed\xab\xee\xdb')
            pe = bytearray(256)
            pe[:2] = b'MZ'
            struct.pack_into('<I', pe, 60, 128)
            pe[128:132] = b'PE\0\0'
            struct.pack_into('<H', pe, 132, 0x8664)
            with zipfile.ZipFile(root / f'{prefix}-windows-x86_64.zip', 'w') as zf:
                for name, value in [('left-hand-control.exe', pe), ('LICENSE', b'L'), ('README.md', b'R'), ('features.md', b'F')]:
                    zf.writestr('bundle/' + name, value)
            vendor = root / f'{prefix}-vendor.tar.xz'
            with tarfile.open(vendor, 'w:xz') as tf:
                data = b'[source.vendored-sources]\ndirectory="vendor"\n'
                member = tarfile.TarInfo(f'{prefix}-vendor/.cargo/config.toml')
                member.size = len(data)
                tf.addfile(member, io.BytesIO(data))
            names = [p.name for p in root.iterdir()]
            (root / 'SHA256SUMS').write_text(''.join(f'{gate.digest(root / n)}  {n}\n' for n in names if not n.endswith('.zip')))
            (root / 'WINDOWS-SHA256SUMS').write_text(f'{gate.digest(root / (prefix + "-windows-x86_64.zip"))}  {prefix}-windows-x86_64.zip\n')
            lock = root / 'Cargo.lock'
            lock.write_text('[[package]]\nname="required"\nversion="1.2.3"\nsource="registry+https://example.invalid"\n')
            with self.assertRaisesRegex(ValueError, 'Vendored dependency missing: required 1.2.3'):
                gate.check(root, version, require_vendor=True, lockfile=lock)


if __name__ == '__main__':
    unittest.main()
