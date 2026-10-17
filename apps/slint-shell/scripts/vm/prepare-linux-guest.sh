#!/bin/sh
# Prepare a freshly installed KDE Plasma VM once, before its base image is frozen:
# ssh key login, passwordless sudo, SDDM or Plasma Login autologin into Plasma Wayland, no
# screen lock or suspend, tools used by linux-guest.py. Run inside the guest as root:
#   sh prepare-linux-guest.sh <user> '<ssh public key>'
set -eu

user=$1
key=$2
if [ "$(id -u)" -ne 0 ]; then
    echo 'Run as root inside the guest.' >&2
    exit 1
fi
home=$(getent passwd "$user" | cut -d: -f6)
[ -n "$home" ] || { echo "No user $user" >&2; exit 1; }

if command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
        openssh-server sudo python3 kdialog desktop-file-utils
    ssh_service=ssh
elif command -v dnf >/dev/null 2>&1; then
    dnf install -y -q openssh-server sudo python3 kdialog desktop-file-utils
    ssh_service=sshd
elif command -v zypper >/dev/null 2>&1; then
    zypper --non-interactive install openssh-server sudo python3 kdialog desktop-file-utils
    ssh_service=sshd
elif command -v pacman >/dev/null 2>&1; then
    pacman -S --noconfirm --needed openssh sudo python kdialog desktop-file-utils
    ssh_service=sshd
else
    echo 'Unknown package manager.' >&2
    exit 1
fi
systemctl enable --now "$ssh_service"

install -d -m 700 -o "$user" -g "$(id -gn "$user")" "$home/.ssh"
printf '%s\n' "$key" > "$home/.ssh/authorized_keys"
chown "$user:$(id -gn "$user")" "$home/.ssh/authorized_keys"
chmod 600 "$home/.ssh/authorized_keys"

printf '%s ALL=(ALL) NOPASSWD: ALL\n' "$user" > /etc/sudoers.d/lhc-e2e
chmod 440 /etc/sudoers.d/lhc-e2e

session=plasma
[ -e /usr/share/wayland-sessions/plasma.desktop ] || session=plasmawayland
# Plasma Login Manager (Fedora 44+) is an SDDM fork with the same [Autologin] keys.
dm_conf=/etc/sddm.conf.d
[ -e /usr/lib/systemd/system/plasmalogin.service ] && dm_conf=/etc/plasmalogin.conf.d
install -d "$dm_conf"
printf '[Autologin]\nUser=%s\nSession=%s\n' "$user" "$session" \
    > "$dm_conf/zz-lhc-e2e.conf"

# A locked screen or a suspended guest would swallow the injected keys.
config="$home/.config"
install -d -o "$user" -g "$(id -gn "$user")" "$config"
printf '[Daemon]\nAutolock=false\nLockOnResume=false\n' > "$config/kscreenlockerrc"
printf '[AC][SuspendAndShutdown]\nAutoSuspendAction=0\n[AC][Display]\nTurnOffDisplayWhenIdle=false\n' \
    > "$config/powerdevilrc"
chown "$user:$(id -gn "$user")" "$config/kscreenlockerrc" "$config/powerdevilrc"

# A cloud image would otherwise search for a datasource on every disposable boot.
[ -d /etc/cloud ] && touch /etc/cloud/cloud-init.disabled

# The stand checks the behaviour without input permissions first.
gpasswd -d "$user" input 2>/dev/null || true

echo "Prepared. Reboot once to confirm autologin, then shut down and freeze the disk as base.qcow2."
