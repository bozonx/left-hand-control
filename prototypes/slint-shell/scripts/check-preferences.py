import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parents[1]
binary = Path(os.environ.get('SLINT_SHELL_BIN', root / 'target/debug/slint-shell')).resolve()
output = Path(sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix='lhc-3b-ipc-'))
output.mkdir(parents=True, exist_ok=True)
env = dict(
    os.environ,
    SLINT_SHELL_SOCKET=f'lhc-3b-{os.getpid()}.sock',
    SLINT_SHELL_METRICS=str(output / 'parent.csv'),
    SLINT_SHELL_POPUPS='spell',
    SLINT_SHELL_HOTKEYS='off',
    SLINT_BACKEND='winit-software',
    RUST_LOG='info',
)
socket = Path(env['XDG_RUNTIME_DIR']) / env['SLINT_SHELL_SOCKET']


def send(command):
    subprocess.run([binary, *command.split()], env=env, check=True, capture_output=True, timeout=4)


with (output / 'preferences.log').open('w') as log:
    process = subprocess.Popen([binary], env=env, stdout=log, stderr=log)
    try:
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError(f'shell exited during startup; see {output}')
            if socket.exists():
                result = subprocess.run([binary, 'ping'], env=env, capture_output=True, timeout=4)
                if result.returncode == 0:
                    break
            time.sleep(.1)
        else:
            raise RuntimeError(f'startup timeout; see {output}')
        for command in [
            'preferences light en', 'show emoji', 'preferences dark ru', 'hide',
            'preferences light en', 'show quick', 'preferences dark ru', 'hide',
            'preferences light en', 'show emoji', 'hide', 'show quick', 'hide',
        ]:
            send(command)
            time.sleep(.4)
        send('quit')
        process.wait(timeout=5)
        assert process.returncode == 0
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)

lines = (output / 'preferences.log').read_text()
for expected in [
    'dark=false, english=true, visible=None',
    'dark=true, english=false, visible=Some("emoji")',
    'dark=true, english=false, visible=Some("quick")',
]:
    assert expected in lines, expected
assert 'Spell preferences:' not in lines
print(f'Spell preferences IPC: passed (hidden, emoji, quick, clean shutdown). Logs: {output}')
