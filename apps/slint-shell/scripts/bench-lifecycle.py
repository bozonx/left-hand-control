#!/usr/bin/env python3
import csv
import io
import os
from pathlib import Path
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
binary = Path(os.environ.get('SLINT_SHELL_BIN', root.parents[1] / 'target/debug/slint-shell')).resolve()
output = Path(sys.argv[1]).resolve()
count = int(sys.argv[2]) if len(sys.argv) > 2 else 500
if count < 1:
    raise SystemExit('count must be positive')
output.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, SLINT_SHELL_SOCKET=f'lhc-lifecycle-{os.getpid()}.sock',
           SLINT_SHELL_METRICS=str(output / 'parent.csv'))
env.setdefault('SLINT_SHELL_POPUPS', 'spell')
env.setdefault('SLINT_BACKEND', 'winit-software')
env.pop('XDG_ACTIVATION_TOKEN', None)
socket = Path(env['XDG_RUNTIME_DIR']) / 'lhc-slint-shell' / env['SLINT_SHELL_SOCKET']
samples = []

def command(*args):
    subprocess.run([binary, *args], env=env, check=True, timeout=4, capture_output=True)

def sample(label):
    result = subprocess.check_output([root / 'scripts/mem.sh', str(server.pid), label], text=True)
    samples.extend(csv.DictReader(io.StringIO(result)))

with (output / 'server.log').open('w') as log:
    server = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 20
        while True:
            if server.poll() is not None:
                raise RuntimeError('server exited; see server.log')
            try:
                command('ping')
                break
            except (subprocess.SubprocessError, OSError):
                if time.monotonic() > deadline:
                    raise RuntimeError('server did not start')
                time.sleep(.05)
        time.sleep(1)
        sample('idle')
        for popup in ('emoji', 'quick'):
            command('show', popup)
            time.sleep(.3)
            sample(popup + '-visible-requested')
            command('hide')
            time.sleep(.1)
        sample('warm-hidden')
        for i in range(count):
            command('show', 'emoji' if i % 2 == 0 else 'quick')
            time.sleep(.06)
            command('hide')
            time.sleep(.06)
        sample(f'after-{count}-cycles')
        command('quit')
        server.wait(timeout=5)
    finally:
        if server.poll() is None:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
        socket.unlink(missing_ok=True)
        with (output / 'memory.csv').open('w') as target:
            writer = csv.DictWriter(target, fieldnames=['label', 'processes', 'rss_kib', 'pss_kib'])
            writer.writeheader()
            writer.writerows(samples)
print(f'{count} command cycles completed; inspect frame/focus counts before judging lifecycle or leaks')
