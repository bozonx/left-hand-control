#!/bin/sh
# Build every Linux release artifact (tar.gz, deb, rpm, vendored sources,
# SHA256SUMS) into <cargo-target>/release-artifacts. Needs nfpm in PATH.
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")/../../../.." && pwd)
cd -- "$repo_dir"
metadata=$(cargo metadata --locked --format-version 1 --no-deps)
version=$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "slint-shell"))')
target_dir=$(printf '%s' "$metadata" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
out="$target_dir/release-artifacts"
rm -rf -- "$out"
mkdir -p -- "$out"

apps/slint-shell/scripts/release/package-linux.sh
binary="$target_dir/release/left-hand-control"
if ldd "$binary" | grep 'not found'; then
    echo 'The release binary has unresolved libraries.' >&2
    exit 1
fi
"$binary" --version
cp -- "$target_dir/packages/left-hand-control-$version-linux-$(uname -m).tar.gz" "$out/"

staging=$(mktemp -d "$target_dir/.release.XXXXXXXX")
trap 'rm -rf -- "$staging"' 0
trap 'exit 1' HUP INT TERM
cat > "$staging/changelog.yml" <<CHANGELOG
- semver: $version-1
  date: $(date -u +%Y-%m-%dT%H:%M:%SZ)
  packager: bozonx <ipkozyrin@gmail.com>
  deb:
    urgency: medium
    distributions: [unstable]
  changes:
    - note: "See https://github.com/bozonx/left-hand-control/blob/v$version/CHANGELOG.md"
CHANGELOG
escape() { printf '%s' "$1" | sed 's/[\\&|]/\\&/g'; }
sed -e "s|@BINARY@|$(escape "$binary")|" -e "s|@CHANGELOG@|$(escape "$staging/changelog.yml")|" \
    apps/slint-shell/packaging/linux/nfpm.yaml > "$staging/nfpm.yaml"
VERSION=$version nfpm package -f "$staging/nfpm.yaml" -p deb \
    -t "$out/left-hand-control_${version}_amd64.deb"
VERSION=$version nfpm package -f "$staging/nfpm.yaml" -p rpm \
    -t "$out/left-hand-control-$version-1.x86_64.rpm"

# Offline distribution builds and the GPL source offer need every dependency.
vendor_name="left-hand-control-$version-vendor"
mkdir -p -- "$staging/$vendor_name/.cargo"
# Extracted over the source tree, `.cargo/config.toml` points at `vendor/`.
cargo vendor --locked --versioned-dirs "$staging/$vendor_name/vendor" \
    | sed "s|\"$staging/$vendor_name/vendor\"|\"vendor\"|" \
    > "$staging/$vendor_name/.cargo/config.toml"
tar -cJf "$out/$vendor_name.tar.xz" -C "$staging" "$vendor_name"

(cd -- "$out" && sha256sum -- *.tar.gz *.deb *.rpm *.tar.xz > SHA256SUMS)
echo "Release artifacts: $out"
