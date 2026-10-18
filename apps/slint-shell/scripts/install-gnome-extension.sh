#!/bin/sh
# Explicit, user-local installation; enabling takes effect after a GNOME login.
set -eu
uuid=left-hand-control@bozonx.dev
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
source_dir="$bundle_dir/gnome-extension/$uuid"
# Also usable directly from a source checkout.
if [ ! -d "$source_dir" ]; then source_dir="$bundle_dir/../../../crates/lhc-core/assets/gnome-extension/$uuid"; fi
destination="$data_dir/gnome-shell/extensions/$uuid"
if [ -L "$destination" ]; then
    echo "Refusing a symlink extension directory: $destination" >&2
    exit 1
fi
case "${1:-install}" in
    install)
        mkdir -p -- "$destination"
        for file in metadata.json extension.js; do
            install -m 644 -- "$source_dir/$file" "$destination/$file"
        done
        echo "Installed $uuid in $destination."
        echo "Log out of GNOME and back in, then run: gnome-extensions enable $uuid"
        ;;
    uninstall)
        if command -v gnome-extensions >/dev/null 2>&1; then
            gnome-extensions disable "$uuid" 2>/dev/null || true
        fi
        rm -f -- "$destination/metadata.json" "$destination/extension.js"
        if [ -d "$destination" ]; then rmdir -- "$destination"; fi
        ;;
    *) echo 'Usage: install-gnome-extension.sh [install|uninstall]' >&2; exit 1 ;;
esac
