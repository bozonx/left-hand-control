#!/usr/bin/env python3
"""Check that a release tag matches the workspace version, CHANGELOG.md and
AppStream metainfo; with --notes, print the changelog section of the version."""

import argparse
from pathlib import Path
import re
import sys
import tomllib
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[3]


def changelog_section(version: str) -> str | None:
    text = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    match = re.search(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}\n(.*?)(?=^## |\Z)",
        text, re.M | re.S,
    )
    return match.group(1).strip() if match else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("tag", help="release tag, e.g. v0.3.0")
    parser.add_argument("--notes", action="store_true", help="print release notes")
    args = parser.parse_args()

    version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))[
        "workspace"]["package"]["version"]
    errors = []
    if args.tag != f"v{version}":
        errors.append(f"tag {args.tag} does not match workspace version {version}")
    notes = changelog_section(version)
    if notes is None:
        errors.append(f"CHANGELOG.md has no '## [{version}] - YYYY-MM-DD' section")
    metainfo = root / "apps/slint-shell/packaging/linux/dev.bozonx.left-hand-control.metainfo.xml"
    releases = ET.parse(metainfo).getroot().find("releases")
    newest = releases[0] if releases is not None and len(releases) else None
    if newest is None or newest.get("version") != version or not newest.get("date"):
        errors.append(f"newest <release> in metainfo is not version {version} with a date")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 1
    if args.notes:
        print(notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
