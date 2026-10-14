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
mkdir -- "$staging/$name"
install -m 755 -- "$target_dir/release/slint-shell" "$staging/$name/slint-shell"
install -m 755 -- apps/slint-shell/scripts/install-linux.sh "$staging/$name/install-linux.sh"
install -m 644 -- apps/slint-shell/assets/icon.png LICENSE "$staging/$name/"
install -m 644 -- docs/development/linux.md "$staging/$name/linux.md"
install -m 644 -- docs/development/linux-release.md "$staging/$name/README.md"
archive="$package_dir/$name.tar.gz"
tar -czf "$staging/$name.tar.gz" -C "$staging" "$name"
mv -f -- "$staging/$name.tar.gz" "$archive"
(cd -- "$package_dir" && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
echo "Release bundle: $archive"
