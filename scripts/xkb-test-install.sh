#!/usr/bin/env bash
# Empirical test: install a minimal XKB extension that maps evdev KEY_F20
# (keycode 194 / XKB <FK20>) to the '@' character on all groups.
#
# If after running this script + logging out/in (or after the KWin reload
# at the end) pressing F20 prints '@' regardless of your active keyboard
# layout (US or RU), then the system-wide XKB approach works and we can
# build the full feature on top of it.
#
# This script is REVERSIBLE — see xkb-test-uninstall.sh.
#
# Requires: sudo (for writing to /usr/share/X11/xkb/).

set -euo pipefail

XKB_DIR="/usr/share/X11/xkb"
SYMBOLS_FILE="${XKB_DIR}/symbols/lhc"
RULES_FILE="${XKB_DIR}/rules/evdev"
LST_FILE="${XKB_DIR}/rules/evdev.lst"
XML_FILE="${XKB_DIR}/rules/evdev.xml"

echo "==> Backing up /usr/share/X11/xkb/rules/evdev* (if not already)"
sudo test -f "${RULES_FILE}.lhc-backup" || sudo cp "${RULES_FILE}" "${RULES_FILE}.lhc-backup"
sudo test -f "${LST_FILE}.lhc-backup"   || sudo cp "${LST_FILE}"   "${LST_FILE}.lhc-backup"
sudo test -f "${XML_FILE}.lhc-backup"   || sudo cp "${XML_FILE}"   "${XML_FILE}.lhc-backup"

echo "==> Writing ${SYMBOLS_FILE}"
sudo tee "${SYMBOLS_FILE}" > /dev/null <<'EOF'
// Left Hand Control — empirical test. Maps F20 to '@'.
partial alphanumeric_keys
xkb_symbols "extras" {
    key <FK24> { [ U0040 ] };   // @ — evdev 194 = KEY_F24 (safer than F20, which
                                //    collides with media-mute on some systems).
};
EOF

echo "==> Patching ${RULES_FILE} (adding symbols mapping)"
if ! sudo grep -q "^  lhc:extras" "${RULES_FILE}"; then
    # Insert our option = symbols rule at the end of the '! option = symbols' table.
    sudo awk '
        /^! option[[:space:]]*=[[:space:]]*symbols/ { in_table = 1; print; next }
        in_table && /^![[:space:]]*[a-z]/        { in_table = 0; print "  lhc:extras           = +lhc(extras)"; print ""; print; next }
        { print }
        END { if (in_table) print "  lhc:extras           = +lhc(extras)" }
    ' "${RULES_FILE}" | sudo tee "${RULES_FILE}.new" > /dev/null
    sudo mv "${RULES_FILE}.new" "${RULES_FILE}"
fi

echo "==> Patching ${LST_FILE}"
if ! sudo grep -q "^  lhc:extras" "${LST_FILE}"; then
    sudo awk '
        /^! option/ { in_table = 1; print; next }
        in_table && /^![[:space:]]*[a-z]/ { in_table = 0; print "  lhc:extras           Left Hand Control (test) — F20 prints @"; print ""; print; next }
        { print }
        END { if (in_table) print "  lhc:extras           Left Hand Control (test) — F20 prints @" }
    ' "${LST_FILE}" | sudo tee "${LST_FILE}.new" > /dev/null
    sudo mv "${LST_FILE}.new" "${LST_FILE}"
fi

echo "==> Patching ${XML_FILE}"
if ! sudo grep -q "<name>lhc:extras</name>" "${XML_FILE}"; then
    # Append a new <group> inside <optionList>. Simple string replace.
    sudo python3 - "$XML_FILE" <<'PY'
import sys, re
p = sys.argv[1]
with open(p, "r", encoding="utf-8") as f:
    xml = f.read()
block = """    <group allowMultipleSelection="true">
      <configItem>
        <name>lhc</name>
        <_description>Left Hand Control (test)</_description>
      </configItem>
      <option>
        <configItem>
          <name>lhc:extras</name>
          <_description>Left Hand Control — F20 prints @</_description>
        </configItem>
      </option>
    </group>
"""
xml = xml.replace("</optionList>", block + "</optionList>", 1)
with open(p, "w", encoding="utf-8") as f:
    f.write(xml)
PY
fi

echo "==> Appending lhc:extras to ~/.config/kxkbrc Options="
KXKBRC="$HOME/.config/kxkbrc"
if [ -f "$KXKBRC" ]; then
    if grep -q "^Options=" "$KXKBRC"; then
        if ! grep -E "^Options=.*lhc:extras" "$KXKBRC" >/dev/null; then
            sed -i.lhc-bak 's/^Options=\(.*\)$/Options=\1,lhc:extras/' "$KXKBRC"
        fi
    else
        # Add an Options line under [Layout].
        sed -i.lhc-bak '/^\[Layout\]$/a Options=lhc:extras' "$KXKBRC"
    fi
else
    mkdir -p "$(dirname "$KXKBRC")"
    cat > "$KXKBRC" <<EOF
[Layout]
Options=lhc:extras
EOF
fi

echo "==> Reloading KWin keyboard config via DBus"
qdbus6 org.kde.keyboard /Layouts org.kde.KeyboardLayouts.switchToNextLayout >/dev/null 2>&1 || true
# The canonical reload:
qdbus6 org.kde.keyboard /Layouts reconfigure >/dev/null 2>&1 \
  || qdbus  org.kde.keyboard /Layouts reconfigure >/dev/null 2>&1 \
  || echo "   (DBus reload call failed — you may need to logout/login)"

echo
echo "==> Test install complete."
echo
echo "Now press the F20 key on your keyboard (if you have one) or generate"
echo "it artificially with xdotool / evemu, and check whether '@' appears"
echo "REGARDLESS of your active system layout (US or RU)."
echo
echo "Easiest artificial test (Wayland-safe): run"
echo "    sudo evemu-event /dev/input/eventN --type EV_KEY --code KEY_F20 --value 1 --sync"
echo "    sudo evemu-event /dev/input/eventN --type EV_KEY --code KEY_F20 --value 0 --sync"
echo "in a terminal focused on a text editor, with different layouts active."
echo
echo "To uninstall: run scripts/xkb-test-uninstall.sh"
