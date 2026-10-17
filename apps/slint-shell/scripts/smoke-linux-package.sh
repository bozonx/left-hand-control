#!/bin/sh
# Install a release package in a disposable container as root, check its files,
# libraries and non-graphical commands, then remove it.
# Usage: smoke-linux-package.sh <file.deb|file.rpm|file.tar.gz>
set -eu

package=$(realpath -- "$1")
app_id=dev.bozonx.left-hand-control
if [ "$(id -u)" -ne 0 ]; then
    echo 'Run this check as root inside a disposable container.' >&2
    exit 1
fi

check_binary() {
    if ldd "$1" | grep 'not found'; then
        echo "Unresolved libraries in $1" >&2
        exit 1
    fi
    "$1" --version
    "$1" --help >/dev/null
}

# AppStream 0.x (Ubuntu 22.04) predates tags the metainfo uses, such as <developer>.
validate_metainfo() {
    if appstreamcli validate --no-net "$1"; then return; fi
    case "$(appstreamcli --version)" in
        *' 0.'*) echo 'Ignoring findings of an AppStream 0.x validator.' ;;
        *) exit 1 ;;
    esac
}

case "$package" in
    *.deb)
        export DEBIAN_FRONTEND=noninteractive
        apt-get update -qq
        apt-get install -y -qq "$package" desktop-file-utils appstream >/dev/null
        ;;
    *.rpm)
        if command -v dnf >/dev/null 2>&1; then
            dnf install -y -q "$package" desktop-file-utils appstream
        else
            zypper --non-interactive install --allow-unsigned-rpm "$package" desktop-file-utils AppStream
        fi
        ;;
    *.tar.gz)
        pacman -Sy --noconfirm --needed fontconfig freetype2 libxkbcommon wayland \
            desktop-file-utils appstream >/dev/null
        work=$(mktemp -d)
        tar -xzf "$package" -C "$work"
        prefix="$work/prefix"
        LHC_INSTALL_PREFIX=$prefix sh "$work"/left-hand-control-*/install-linux.sh
        check_binary "$prefix/bin/left-hand-control"
        desktop-file-validate "$prefix/share/applications/$app_id.desktop"
        validate_metainfo "$prefix/share/metainfo/$app_id.metainfo.xml"
        LHC_INSTALL_PREFIX=$prefix sh "$prefix/lib/$app_id/install-linux.sh" uninstall
        test ! -e "$prefix/bin/left-hand-control"
        echo "Smoke check passed: $package"
        exit 0
        ;;
    *) echo "Unknown package type: $package" >&2; exit 1 ;;
esac

check_binary /usr/bin/left-hand-control
for path in \
    /usr/share/applications/$app_id.desktop \
    /usr/share/metainfo/$app_id.metainfo.xml \
    /usr/share/icons/hicolor/scalable/apps/$app_id.svg \
    /usr/share/icons/hicolor/512x512/apps/$app_id.png \
    /usr/lib/udev/rules.d/60-left-hand-control.rules \
    /usr/lib/modules-load.d/left-hand-control.conf; do
    test -f "$path" || { echo "Missing $path" >&2; exit 1; }
done
desktop-file-validate /usr/share/applications/$app_id.desktop
validate_metainfo /usr/share/metainfo/$app_id.metainfo.xml

case "$package" in
    *.deb) apt-get remove -y -qq left-hand-control >/dev/null ;;
    *.rpm)
        if command -v dnf >/dev/null 2>&1; then dnf remove -y -q left-hand-control
        else zypper --non-interactive remove left-hand-control; fi
        ;;
esac
for path in /usr/bin/left-hand-control /usr/lib/udev/rules.d/60-left-hand-control.rules; do
    test ! -e "$path" || { echo "Not removed: $path" >&2; exit 1; }
done
echo "Smoke check passed: $package"
