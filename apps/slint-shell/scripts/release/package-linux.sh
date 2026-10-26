#!/bin/sh
# Build a native Linux release bundle; output stays under Cargo's target directory.
set -eu

if [ "$(uname -s)" != Linux ]; then
    echo 'This script builds native Linux bundles only.' >&2
    exit 1
fi
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)
cd -- "$repo_dir"
metadata=$(cargo metadata --locked --format-version 1 --no-deps)
version=$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "slint-shell"))')
target_dir=$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
name="left-hand-control-$version-linux-$(uname -m)"
package_dir="$target_dir/packages"
mkdir -p -- "$package_dir"
staging=$(mktemp -d "$package_dir/.bundle.XXXXXXXX")
trap 'rm -rf -- "$staging"' 0
trap 'exit 1' HUP INT TERM
cargo build --release --locked -p slint-shell --features spell
mkdir -p -- "$staging/$name/icons"
install -m 755 -- "$target_dir/release/left-hand-control" "$staging/$name/left-hand-control"
install -m 755 -- apps/slint-shell/scripts/install-linux.sh "$staging/$name/install-linux.sh"
install -m 755 -- apps/slint-shell/scripts/install-gnome-extension.sh "$staging/$name/install-gnome-extension.sh"
cp -R -- crates/lhc-core/assets/gnome-extension "$staging/$name/gnome-extension"
packaging=apps/slint-shell/packaging/linux
install -m 644 -- LICENSE "$packaging/dev.bozonx.left-hand-control.desktop" \
    "$packaging/dev.bozonx.left-hand-control.metainfo.xml" "$packaging/60-left-hand-control.rules" \
    "$packaging/modules-load.conf" "$staging/$name/"
install -m 644 -- "$packaging"/icons/*.png "$staging/$name/icons/"
install -m 644 -- apps/slint-shell/assets/icon.svg "$staging/$name/icons/scalable.svg"
install -m 644 -- docs/features.md "$staging/$name/features.md"
install -m 644 -- docs/install-linux.md "$staging/$name/README.md"
archive="$package_dir/$name.tar.gz"
tar -czf "$staging/$name.tar.gz" -C "$staging" "$name"
mv -f -- "$staging/$name.tar.gz" "$archive"
(cd -- "$package_dir" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "Release bundle: $archive"
