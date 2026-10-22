#!/usr/bin/env python3
"""Test a portable ZIP on a clean Windows VM via ISO/QMP, without SSH or Cargo.

The VM base must predate developer tools and boot into its test-user desktop.
Only the supplied, Microsoft-signed VC runtime may be installed when needed.
"""
import argparse
import hashlib
import http.server
import json
from pathlib import Path
import secrets
import signal
import shutil
import subprocess
import threading
import time

from vmlib import REPO, Report, StepFailed, Vm, commit, result_path, wait_for


class Callback:
    def __init__(self):
        self.events = []
        self.token = secrets.token_urlsafe(32)
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                if self.path != '/' + owner.token:
                    self.send_error(403)
                    return
                try:
                    length = int(self.headers.get('Content-Length', 0))
                    if not 0 < length <= 8 * 1024 * 1024:
                        raise ValueError('invalid size')
                    value = json.loads(self.rfile.read(length))
                    if value.get('phase') not in ('runtime-uac', 'complete'):
                        raise ValueError('invalid phase')
                    if value['phase'] == 'complete' and value.get('status') not in ('0', '1'):
                        raise ValueError('invalid status')
                    if not isinstance(value.get('details'), str):
                        raise ValueError('missing transcript')
                except (ValueError, TypeError, AttributeError):
                    self.send_error(400)
                    return
                owner.events.append(value)
                self.send_response(204)
                self.end_headers()

            def log_message(self, *_args):
                pass

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://10.0.2.2:{self.server.server_port}/{self.token}'

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--vm', required=True)
    parser.add_argument('--package', required=True, type=Path)
    parser.add_argument('--runtime-installer', required=True, type=Path)
    parser.add_argument('--cycles', type=int, default=500)
    parser.add_argument('--stress-seconds', type=int, default=180)
    parser.add_argument('--boot-seconds', type=int, default=60)
    parser.add_argument('--timeout', type=int, default=1200)
    args = parser.parse_args()
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    if args.cycles < 100 or args.stress_seconds < 0:
        parser.error('at least 100 cycles; nonnegative stress duration')
    package = args.package.resolve()
    runtime = args.runtime_installer.resolve()
    for path in (package, package.with_name('WINDOWS-SHA256SUMS'), runtime):
        if not path.is_file():
            parser.error(f'missing {path}')
    vm = Vm(args.vm)
    if vm.get('cache_disk'):
        parser.error('clean package acceptance must not attach a developer cache disk')
    report = Report()
    callback = Callback()
    try:
        payload = vm.run_dir / 'payload'
        scripts = payload / 'apps/slint-shell/scripts'
        scripts.mkdir(parents=True)
        for path in (REPO / 'apps/slint-shell/scripts').rglob('*'):
            if path.suffix in ('.ps1', '.cs'):
                destination = scripts / path.relative_to(REPO / 'apps/slint-shell/scripts')
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, destination)
        shutil.copy2(Path(__file__).with_name('windows-package-guest.ps1'), payload / 'lhc-bootstrap.ps1')
        for path in (package, package.with_name('WINDOWS-SHA256SUMS'), runtime):
            shutil.copy2(path, payload / path.name)
        (payload / 'package-test.json').write_text(json.dumps({
            'callback': callback.url, 'cycles': args.cycles, 'seconds': args.stress_seconds,
        }))
        iso = vm.run_dir / 'acceptance.iso'
        subprocess.run(['xorriso', '-as', 'mkisofs', '-quiet', '-J', '-r', '-V', 'LHC-E2E',
                        '-o', str(iso), str(payload)], check=True, capture_output=True)
        vm.config['cdrom'] = str(iso)
        report.step('boot-clean-windows', vm.start, fatal=True)
        time.sleep(args.boot_seconds)
        vm.qmp.screendump(vm.logs / 'before-bootstrap.png')
        vm.qmp.tap('meta_l-r')
        time.sleep(1)
        vm.qmp.type_ascii(r'powershell -NoProfile -ExecutionPolicy Bypass -File D:\lhc-bootstrap.ps1')
        vm.qmp.tap('ret')
        handled = 0

        def completed():
            nonlocal handled
            if vm.qemu.poll() is not None:
                raise StepFailed('QEMU exited before completion')
            while handled < len(callback.events):
                event = callback.events[handled]
                handled += 1
                if event['phase'] == 'runtime-uac':
                    print('Guest requests signed VC runtime installation', flush=True)
                    time.sleep(3)
                    vm.qmp.screendump(vm.logs / 'runtime-uac.png')
                    vm.qmp.tap('alt-y')
                elif event['phase'] == 'complete':
                    (vm.logs / 'guest.log').write_text(event['details'], encoding='utf-8')
                    if event['status'] != '0':
                        raise StepFailed(event['details'][-6000:])
                    if 'WINDOWS PACKAGE PASSED' not in event['details']:
                        raise StepFailed('Guest returned success without package verdict')
                    return "clean baseline without toolchain; packaged CLI/lifecycle/stress passed; see guest.log"
            return None

        report.step('clean-zip-lifecycle-stress', wait_for, 'clean package completion',
                    args.timeout, completed, 1, fatal=True)
    except KeyboardInterrupt:
        report.steps.append(("controller", "FAILED", "interrupted; no clean-OS acceptance"))
    except (StepFailed, subprocess.SubprocessError, OSError) as error:
        if not report.failed:
            report.steps.append(('controller', 'FAILED', str(error)))
    finally:
        if vm.qemu and vm.qemu.poll() is None:
            vm.qmp.screendump(vm.logs / 'final.png')
        callback.close()
        vm.stop(keep=bool(report.failed))
    report.write([
        f'Date: {time.strftime("%F %T %z")}', f'Commit: {commit()}',
        f'VM: {vm.name} ({vm.base}); no cache disk',
        f'Package: {package.name}; SHA256 {hashlib.sha256(package.read_bytes()).hexdigest()}',
        f'VC runtime installer SHA256: {hashlib.sha256(runtime.read_bytes()).hexdigest()}',
        f'Cycles >= {args.cycles}; duration >= {args.stress_seconds}s', f'Logs: {vm.logs}',
    ], result_path('windows', vm.name))
    raise SystemExit(1 if report.failed else 0)


if __name__ == '__main__':
    main()
