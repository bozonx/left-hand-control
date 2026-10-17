#!/bin/sh
# Install an extracted release bundle without elevated privileges; only the
# explicit `permissions` commands use sudo.
set -eu

app_id=dev.bozonx.left-hand-control
binary=left-hand-control
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
launcher="$prefix/bin/$binary"
desktop="$prefix/share/applications/$app_id.desktop"
metainfo="$prefix/share/metainfo/$app_id.metainfo.xml"
icons="$prefix/share/icons/hicolor"
icon_sizes='16 22 24 32 48 64 128 256 512'
bundle_files="$binary LICENSE README.md linux.md $app_id.desktop $app_id.metainfo.xml
60-left-hand-control.rules modules-load.conf icons/scalable.svg"
for size in $icon_sizes; do bundle_files="$bundle_files icons/$size.png"; done
udev_rule=/etc/udev/rules.d/60-left-hand-control.rules
modules_load=/etc/modules-load.d/left-hand-control.conf

check_launcher() {
    if [ -e "$launcher" ] || [ -L "$launcher" ]; then
        if [ ! -L "$launcher" ] || [ "$(readlink -- "$launcher")" != "$app_dir/$binary" ]; then
            echo "Refusing to replace unrelated launcher: $launcher" >&2
            exit 1
        fi
    fi
}

as_root() {
    if [ "$(id -u)" -eq 0 ]; then "$@"; else sudo -- "$@"; fi
}

case "${1:-install}" in
    uninstall)
        check_launcher
        rm -f -- "$launcher" "$desktop" "$metainfo" "$icons/scalable/apps/$app_id.svg"
        for size in $icon_sizes; do
            rm -f -- "$icons/${size}x$size/apps/$app_id.png"
        done
        for name in $bundle_files install-linux.sh; do
            rm -f -- "$app_dir/$name"
        done
        if [ -d "$app_dir/icons" ]; then rmdir -- "$app_dir/icons"; fi
        if [ -d "$app_dir" ]; then rmdir -- "$app_dir"; fi
        echo "Removed Left Hand Control from $prefix; configuration was retained."
        if [ -e "$udev_rule" ]; then
            echo "Device permissions remain; remove them with: sudo rm -f $udev_rule $modules_load"
        fi
        exit 0
        ;;
    permissions)
        user=${SUDO_USER:-$(id -un)}
        if [ "$user" = root ]; then
            echo 'Run this command as the desktop user, not as root.' >&2
            exit 1
        fi
        as_root install -D -m 644 -- "$bundle_dir/60-left-hand-control.rules" "$udev_rule"
        as_root install -D -m 644 -- "$bundle_dir/modules-load.conf" "$modules_load"
        as_root modprobe uinput || echo 'Could not load the uinput module now; it loads on the next boot.' >&2
        as_root udevadm control --reload-rules
        as_root udevadm trigger --subsystem-match=misc --sysname-match=uinput
        if id -nG -- "$user" | tr ' ' '\n' | grep -qx input; then
            echo "Device permissions installed; $user is already in the input group."
        else
            as_root usermod -aG input -- "$user"
            echo "Device permissions installed and $user added to the input group."
            echo 'Log out and back in so that keyboard access takes effect.'
        fi
        exit 0
        ;;
    remove-permissions)
        as_root rm -f -- "$udev_rule" "$modules_load"
        as_root udevadm control --reload-rules
        echo 'Removed the udev rule and uinput autoload.'
        echo "Group membership is unchanged; leave the input group with: sudo gpasswd -d $(id -un) input"
        exit 0
        ;;
    install) ;;
    *) echo 'Usage: install-linux.sh [install|uninstall|permissions|remove-permissions]' >&2; exit 1 ;;
esac

check_launcher
for name in $bundle_files; do
    if [ ! -f "$bundle_dir/$name" ]; then
        echo "Missing release bundle file: $name" >&2
        exit 1
    fi
done
mkdir -p -- "$app_dir/icons" "$prefix/bin" "$(dirname -- "$desktop")" "$(dirname -- "$metainfo")"
# Keep the installed executable path stable; replace its inode so a running
# executable does not prevent an update.
temporary_binary=$(mktemp "$app_dir/.$binary.XXXXXXXX")
trap 'rm -f -- "$temporary_binary"' 0
trap 'exit 1' HUP INT TERM
install -m 755 -- "$bundle_dir/$binary" "$temporary_binary"
mv -f -- "$temporary_binary" "$app_dir/$binary"
install -m 755 -- "$bundle_dir/install-linux.sh" "$app_dir/install-linux.sh"
for name in $bundle_files; do
    if [ "$name" != "$binary" ]; then
        install -m 644 -- "$bundle_dir/$name" "$app_dir/$name"
    fi
done
for size in $icon_sizes; do
    install -D -m 644 -- "$bundle_dir/icons/$size.png" "$icons/${size}x$size/apps/$app_id.png"
done
install -D -m 644 -- "$bundle_dir/icons/scalable.svg" "$icons/scalable/apps/$app_id.svg"
install -m 644 -- "$bundle_dir/$app_id.metainfo.xml" "$metainfo"
ln -sfn -- "$app_dir/$binary" "$launcher"
# Desktop Entry escaping has an extra string-unescaping stage before Exec.
exec_path=$(printf '%s' "$app_dir/$binary" | sed 's/\\/\\\\\\\\/g; s/[$`"]/\\\\&/g; s/%/%%/g')
# The sed replacement needs its own escaping of backslashes and ampersands.
exec_replacement=$(printf '%s' "$exec_path" | sed 's/[\\&|]/\\&/g')
sed "s|^Exec=.*|Exec=\"$exec_replacement\"|" "$bundle_dir/$app_id.desktop" > "$desktop"
chmod 644 -- "$desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$(dirname -- "$desktop")" || true
fi
echo "Installed Left Hand Control in $app_dir."
echo "Start with: $launcher"
if [ ! -e "$udev_rule" ]; then
    echo "Grant keyboard and /dev/uinput access once with: $app_dir/install-linux.sh permissions"
fi
