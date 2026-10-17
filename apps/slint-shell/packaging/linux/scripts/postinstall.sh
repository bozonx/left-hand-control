#!/bin/sh
# Apply the bundled udev rule and load uinput; group membership stays the
# administrator's decision.
set -e
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || true
    udevadm trigger --subsystem-match=misc --sysname-match=uinput || true
fi
modprobe uinput 2>/dev/null || true
echo 'Left Hand Control: reading keyboards needs the input group:'
echo '  sudo usermod -aG input "$USER"   # then log out and back in'
exit 0
