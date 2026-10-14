#!/bin/sh
# Install an extracted release bundle without elevated privileges.
set -eu

app_id=dev.bozonx.left-hand-control
bundle_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
prefix=${LHC_INSTALL_PREFIX:-"$HOME/.local"}
case "$prefix" in
    /*) ;;
    *) echo 'LHC_INSTALL_PREFIX must be absolute' >&2; exit 1 ;;
esac
case "$prefix" in
    *'
'*|*"$(printf '\r')"*) echo 'Installation prefix must not contain line breaks' >&2; exit 1 ;;
esac
app_dir="$prefix/lib/$app_id"
if [ -L "$app_dir" ]; then
    echo "Refusing to use a symlink as application directory: $app_dir" >&2
    exit 1
fi
launcher="$prefix/bin/left-hand-control"
desktop="$prefix/share/applications/$app_id.desktop"
icon="$prefix/share/icons/hicolor/32x32/apps/$app_id.png"

check_launcher() {
    if [ -e "$launcher" ] || [ -L "$launcher" ]; then
        if [ ! -L "$launcher" ] || [ "$(readlink -- "$launcher")" != "$app_dir/slint-shell" ]; then
            echo "Refusing to replace unrelated launcher: $launcher" >&2
            exit 1
        fi
    fi
}

case "${1:-install}" in
    uninstall)
        check_launcher
        rm -f -- "$launcher" "$desktop" "$icon"
        for name in slint-shell icon.png LICENSE README.md linux.md install-linux.sh; do
            rm -f -- "$app_dir/$name"
        done
        if [ -d "$app_dir" ]; then rmdir -- "$app_dir"; fi
        echo "Removed Left Hand Control from $prefix; configuration was retained."
        exit 0
        ;;
    install) ;;
    *) echo 'Usage: install-linux.sh [install|uninstall]' >&2; exit 1 ;;
esac

check_launcher
for name in slint-shell icon.png LICENSE README.md linux.md; do
    if [ ! -f "$bundle_dir/$name" ]; then
        echo "Missing release bundle file: $name" >&2
        exit 1
    fi
done
mkdir -p -- "$app_dir" "$prefix/bin" "$(dirname -- "$desktop")" "$(dirname -- "$icon")"
# Keep the installed executable path stable; replace its inode so a running
# executable does not prevent an update.
temporary_binary=$(mktemp "$app_dir/.slint-shell.XXXXXXXX")
trap 'rm -f -- "$temporary_binary"' 0
trap 'exit 1' HUP INT TERM
install -m 755 -- "$bundle_dir/slint-shell" "$temporary_binary"
mv -f -- "$temporary_binary" "$app_dir/slint-shell"
install -m 755 -- "$bundle_dir/install-linux.sh" "$app_dir/install-linux.sh"
for name in icon.png LICENSE README.md linux.md; do
    install -m 644 -- "$bundle_dir/$name" "$app_dir/$name"
done
install -m 644 -- "$bundle_dir/icon.png" "$icon"
ln -sfn -- "$app_dir/slint-shell" "$launcher"
# Desktop Entry escaping has an extra string-unescaping stage before Exec.
exec_path=$(printf '%s' "$app_dir/slint-shell" | sed 's/\\/\\\\\\\\/g; s/[$`"]/\\\\&/g; s/%/%%/g')
cat > "$desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Left Hand Control
Exec="$exec_path"
Icon=$app_id
Terminal=false
Categories=Utility;Accessibility;
DESKTOP
chmod 644 -- "$desktop"
echo "Installed Left Hand Control in $app_dir."
echo "Start with: $launcher"
