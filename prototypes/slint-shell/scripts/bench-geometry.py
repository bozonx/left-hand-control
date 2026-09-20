#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import sys
import time

if not os.environ.get('WAYLAND_DISPLAY', '').startswith('lhc-stage3a-'):
    raise SystemExit('Run only inside the dedicated lhc-stage3a-* virtual compositor')
root = Path(__file__).resolve().parents[1]
binary = root / 'target/debug/slint-shell'
probe = root / 'target/debug/examples/inspect-geometry'
output = Path(sys.argv[1]).resolve()
output.mkdir(parents=True, exist_ok=True)
config = json.loads(subprocess.check_output(['kscreen-doctor', '-j'], text=True))
names = [o['name'] for o in config['outputs'] if o['connected'] and o['enabled']]
if len(names) < 2:
    raise RuntimeError('two virtual outputs required')
rows = []
fixtures = []
env = dict(os.environ, SLINT_SHELL_POPUPS='spell', SLINT_BACKEND='winit-software',
           SLINT_SHELL_OUTPUT=names[-1], SLINT_SHELL_HOTKEYS='off', SLINT_SHELL_SOCKET=f'lhc-geometry-{os.getpid()}.sock',
           SLINT_SHELL_METRICS=str(output / 'parent.csv'))
with (output / 'server.log').open('w') as log:
    server = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
    def command(*args):
        subprocess.run([binary, *args], env=env, check=True, capture_output=True, timeout=5)
    def observe():
        children = Path(f'/proc/{server.pid}/task/{server.pid}/children').read_text().split()
        return json.loads(subprocess.check_output([probe, str(server.pid), *children, *[str(f.pid) for f in fixtures]], text=True, timeout=5))
    def geometry(popup, scale, target, fixture_before=None):
        command('show', popup)
        deadline = time.monotonic() + 5
        row = None
        height = 460 if popup == 'emoji' else 500
        while time.monotonic() < deadline:
            entries = observe()
            (output / 'last-observation.json').write_text(json.dumps(entries, indent=2))
            row = next((r for r in entries if r['height'] == height), None)
            if row:
                break
            time.sleep(.05)
        if row is None:
            raise RuntimeError(f'{popup} geometry missing')
        height = 460 if popup == 'emoji' else 500
        area = row['area']
        assert row['width'] == 520 and row['height'] == height, row
        assert row['output'] == target, row
        assert abs(row['x'] - (area['x'] + (area['width'] - 520) / 2)) <= 1, row
        assert abs(row['y'] + height + 24 - area['y'] - area['height']) <= 1, row
        assert row['skipTaskbar'], row
        assert row['clientWidth'] == row['width'] and row['clientHeight'] == row['height'], row
        if fixture_before:
            fixture_after = next(r for r in entries if r['pid'] == fixture_before['pid'])
            for key in ('x', 'y', 'width', 'height'):
                assert fixture_after[key] == fixture_before[key], (fixture_before, fixture_after)
            assert row['stack'] > fixture_after['stack'], entries
        rows.append(dict(row, scale=scale, popup=popup))
        command('hide')
        time.sleep(.1)
    try:
        deadline = time.monotonic() + 20
        while True:
            if server.poll() is not None:
                raise RuntimeError('server failed; see log')
            try:
                command('ping')
                break
            except subprocess.CalledProcessError:
                if time.monotonic() > deadline:
                    raise
                time.sleep(.1)
        for scale in (1, 1.25, 1.5, 1):
            subprocess.run(['kscreen-doctor', f'output.{names[-1]}.scale.{scale}'], check=True, capture_output=True)
            time.sleep(.3)
            for popup in ('emoji', 'quick'):
                geometry(popup, scale, names[-1])
        command('show', 'emoji')
        time.sleep(.2)
        subprocess.run(['kscreen-doctor', f'output.{names[-1]}.disable'], check=True, capture_output=True)
        time.sleep(.5)
        command('hide')
        for popup in ('emoji', 'quick'):
            geometry(popup, 'output-disabled', names[0])
        panel = subprocess.Popen([root / 'target/debug/examples/panel-fixture'], stdout=log, stderr=log)
        fixtures.append(panel)
        time.sleep(.3)
        for mode in ('normal', 'maximized', 'fullscreen'):
            fixture = subprocess.Popen([root / 'target/debug/examples/window-fixture'],
                                       env=dict(os.environ, SLINT_BACKEND='winit-software', SLINT_FIXTURE_MODE=mode), stdout=log, stderr=log)
            fixtures.append(fixture)
            time.sleep(.5)
            before = next(r for r in observe() if r['pid'] == fixture.pid)
            if mode == 'maximized':
                assert before['width'] == before['area']['width'] and before['height'] == before['area']['height'], before
            if mode == 'fullscreen':
                assert before['fullscreen'], before
            for popup in ('emoji', 'quick'):
                geometry(popup, 'panel-' + mode, names[0], before)
            fixture.terminate()
            fixture.wait(timeout=5)
            fixtures.remove(fixture)
        command('quit')
        server.wait(timeout=5)
    finally:
        for fixture in fixtures:
            fixture.terminate()
            fixture.wait(timeout=5)
        if server.poll() is None:
            server.terminate()
            try:
                server.wait(timeout=5)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
        (output / 'geometry.json').write_text(json.dumps(rows, indent=2))
print(f'{len(rows)} geometry checks passed')
