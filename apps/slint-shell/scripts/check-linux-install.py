#!/usr/bin/env python3
"""Check the release installer in temporary directories, without user storage."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile

scripts = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="lhc-linux-install-") as directory:
    root = Path(directory)
    bundle = root / "bundle"
    bundle.mkdir()
    shutil.copy2(scripts / "install-linux.sh", bundle / "install-linux.sh")
    for name in ("icon.png", "LICENSE", "README.md", "linux.md"):
        (bundle / name).write_text("fixture", encoding="utf-8")
    (bundle / "slint-shell").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    prefix = root / 'prefix with $cash%quote"back`slash\\ space'
    env = dict(os.environ, LHC_INSTALL_PREFIX=str(prefix))

    def install():
        subprocess.run(
            ["sh", str(bundle / "install-linux.sh")],
            env=env, check=True, capture_output=True, timeout=10,
        )

    install()
    app = prefix / "lib/dev.bozonx.left-hand-control"
    launcher = prefix / "bin/left-hand-control"
    assert launcher.resolve() == app / "slint-shell"
    subprocess.run([str(launcher)], check=True, timeout=10)
    desktop = prefix / "share/applications/dev.bozonx.left-hand-control.desktop"
    subprocess.run(["desktop-file-validate", str(desktop)], check=True, timeout=10)
    install()
    retained = prefix / "keep-user-data"
    retained.write_text("keep", encoding="utf-8")
    subprocess.run(
        ["sh", str(app / "install-linux.sh"), "uninstall"],
        env=env, check=True, capture_output=True, timeout=10,
    )
    assert not launcher.exists() and not app.exists() and not desktop.exists()
    assert retained.read_text(encoding="utf-8") == "keep"
    launcher.write_text("unrelated", encoding="utf-8")
    refused = subprocess.run(
        ["sh", str(bundle / "install-linux.sh")],
        env=env, capture_output=True, timeout=10,
    )
    assert refused.returncode != 0
    assert launcher.read_text(encoding="utf-8") == "unrelated"

print("Linux installer passed: escaped paths, launch, update, uninstall, unrelated files")
