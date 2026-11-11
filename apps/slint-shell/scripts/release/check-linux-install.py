#!/usr/bin/env python3
"""Check the release installer in temporary directories, without user storage."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile

scripts = Path(__file__).resolve().parent
with tempfile.TemporaryDirectory(prefix="tecloid-linux-install-") as directory:
    root = Path(directory)
    bundle = root / "bundle"
    bundle.mkdir()
    shutil.copy2(scripts.parent / "install-linux.sh", bundle / "install-linux.sh")
    shutil.copy2(scripts.parent / "install-gnome-extension.sh", bundle / "install-gnome-extension.sh")
    shutil.copytree(scripts.parents[3] / "crates/tecloid-core/assets/gnome-extension", bundle / "gnome-extension")
    for name in ("LICENSE", "README.md", "features.md"):
        (bundle / name).write_text("fixture", encoding="utf-8")
    packaging = scripts.parents[1] / "packaging/linux"
    for name in (
        "dev.bozonx.tecloid.desktop",
        "dev.bozonx.tecloid.metainfo.xml",
        "60-tecloid.rules",
        "modules-load.conf",
    ):
        shutil.copy2(packaging / name, bundle / name)
    shutil.copytree(packaging / "icons", bundle / "icons")
    shutil.copy2(scripts.parents[1] / "assets/icon.svg", bundle / "icons/scalable.svg")
    (bundle / "tecloid").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    prefix = root / 'prefix with $cash%quote"back`slash\\ space'
    env = dict(os.environ, TECLOID_INSTALL_PREFIX=str(prefix))

    def install():
        subprocess.run(
            ["sh", str(bundle / "install-linux.sh")],
            env=env, check=True, capture_output=True, timeout=10,
        )

    install()
    app = prefix / "lib/dev.bozonx.tecloid"
    launcher = prefix / "bin/tecloid"
    assert launcher.resolve() == app / "tecloid"
    subprocess.run([str(launcher)], check=True, timeout=10)
    desktop = prefix / "share/applications/dev.bozonx.tecloid.desktop"
    subprocess.run(["desktop-file-validate", str(desktop)], check=True, timeout=10)
    exec_line = next(
        line for line in desktop.read_text(encoding="utf-8").splitlines()
        if line.startswith("Exec=")
    )
    assert exec_line.startswith('Exec="') and "\\\\$cash%%quote" in exec_line, exec_line
    metainfo = prefix / "share/metainfo/dev.bozonx.tecloid.metainfo.xml"
    icons = prefix / "share/icons/hicolor"
    installed = [metainfo, icons / "scalable/apps/dev.bozonx.tecloid.svg"]
    installed += [
        icons / f"{size}x{size}/apps/dev.bozonx.tecloid.png"
        for size in (16, 22, 24, 32, 48, 64, 128, 256, 512)
    ]
    assert all(path.is_file() for path in installed)
    extension_data = root / "gnome data"
    extension_env = dict(env, XDG_DATA_HOME=str(extension_data),
                         DBUS_SESSION_BUS_ADDRESS=f"unix:path={root}/absent-bus")
    extension_script = app / "install-gnome-extension.sh"
    extension_dir = extension_data / "gnome-shell/extensions/tecloid@bozonx.dev"
    subprocess.run([str(extension_script)], env=extension_env, check=True,
                   capture_output=True, timeout=10)
    assert (extension_dir / "extension.js").read_bytes() == (bundle / "gnome-extension/tecloid@bozonx.dev/extension.js").read_bytes()
    unrelated = extension_dir.parent / "unrelated"
    unrelated.write_text("keep", encoding="utf-8")
    subprocess.run([str(extension_script), "uninstall"], env=extension_env,
                   check=True, capture_output=True, timeout=10)
    assert not extension_dir.exists() and unrelated.read_text() == "keep"
    extension_dir.symlink_to(root, target_is_directory=True)
    refused = subprocess.run([str(extension_script)], env=extension_env,
                             capture_output=True, timeout=10)
    assert refused.returncode != 0
    extension_dir.unlink()

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

print("Linux installer passed: escaped paths, icons, metadata, launch, update, uninstall, unrelated files, isolated GNOME extension and symlink refusal")
