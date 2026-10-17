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
    for name in ("LICENSE", "README.md", "linux.md"):
        (bundle / name).write_text("fixture", encoding="utf-8")
    packaging = scripts.parent / "packaging/linux"
    for name in (
        "dev.bozonx.left-hand-control.desktop",
        "dev.bozonx.left-hand-control.metainfo.xml",
        "60-left-hand-control.rules",
        "modules-load.conf",
    ):
        shutil.copy2(packaging / name, bundle / name)
    shutil.copytree(packaging / "icons", bundle / "icons")
    shutil.copy2(scripts.parent / "assets/icon.svg", bundle / "icons/scalable.svg")
    (bundle / "left-hand-control").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
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
    assert launcher.resolve() == app / "left-hand-control"
    subprocess.run([str(launcher)], check=True, timeout=10)
    desktop = prefix / "share/applications/dev.bozonx.left-hand-control.desktop"
    subprocess.run(["desktop-file-validate", str(desktop)], check=True, timeout=10)
    exec_line = next(
        line for line in desktop.read_text(encoding="utf-8").splitlines()
        if line.startswith("Exec=")
    )
    assert exec_line.startswith('Exec="') and "\\\\$cash%%quote" in exec_line, exec_line
    metainfo = prefix / "share/metainfo/dev.bozonx.left-hand-control.metainfo.xml"
    icons = prefix / "share/icons/hicolor"
    installed = [metainfo, icons / "scalable/apps/dev.bozonx.left-hand-control.svg"]
    installed += [
        icons / f"{size}x{size}/apps/dev.bozonx.left-hand-control.png"
        for size in (16, 22, 24, 32, 48, 64, 128, 256, 512)
    ]
    assert all(path.is_file() for path in installed)
    install()
    retained = prefix / "keep-user-data"
    retained.write_text("keep", encoding="utf-8")
    subprocess.run(
        ["sh", str(app / "install-linux.sh"), "uninstall"],
        env=env, check=True, capture_output=True, timeout=10,
    )
    assert not launcher.exists() and not app.exists() and not desktop.exists()
    assert not any(path.exists() for path in installed)
    assert retained.read_text(encoding="utf-8") == "keep"
    launcher.write_text("unrelated", encoding="utf-8")
    refused = subprocess.run(
        ["sh", str(bundle / "install-linux.sh")],
        env=env, capture_output=True, timeout=10,
    )
    assert refused.returncode != 0
    assert launcher.read_text(encoding="utf-8") == "unrelated"

print("Linux installer passed: escaped paths, icons, metadata, launch, update, uninstall, unrelated files")
