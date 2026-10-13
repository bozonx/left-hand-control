#!/usr/bin/env bash
# Reverse everything xkb-test-install.sh did: restore the original
# /usr/share/X11/xkb/rules/evdev* files from the .lhc-backup copies,
# remove our symbols file, and strip lhc:extras from ~/.config/kxkbrc.

set -euo pipefail

XKB_DIR="/usr/share/X11/xkb"
SYMBOLS_FILE="${XKB_DIR}/symbols/lhc"
RULES_FILE="${XKB_DIR}/rules/evdev"
LST_FILE="${XKB_DIR}/rules/evdev.lst"
XML_FILE="${XKB_DIR}/rules/evdev.xml"

echo "==> Restoring original rule files from .lhc-backup"
for f in "$RULES_FILE" "$LST_FILE" "$XML_FILE"; do
    if sudo test -f "${f}.lhc-backup"; then
        sudo mv "${f}.lhc-backup" "${f}"
    fi
done

echo "==> Removing ${SYMBOLS_FILE}"
sudo rm -f "${SYMBOLS_FILE}"

echo "==> Stripping lhc:extras from ~/.config/kxkbrc"
KXKBRC="$HOME/.config/kxkbrc"
if [ -f "$KXKBRC" ]; then
    sed -i.lhc-bak \
        -e 's/,lhc:extras//g' \
        -e 's/^Options=lhc:extras$/Options=/' \
        -e '/^Options=$/d' \
        "$KXKBRC"
fi

echo "==> Reloading KWin keyboard config"
qdbus6 org.kde.keyboard /Layouts reconfigure >/dev/null 2>&1 \
  || qdbus  org.kde.keyboard /Layouts reconfigure >/dev/null 2>&1 \
  || echo "   (DBus reload failed — logout/login to apply)"

echo "==> Done."
