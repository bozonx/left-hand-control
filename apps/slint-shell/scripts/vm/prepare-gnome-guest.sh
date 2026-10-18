#!/bin/sh
# Run as root in a disposable Ubuntu GNOME guest, never on the host.
set -eu
user=${1:-tester}
id "$user"
if ! command -v gnome-shell >/dev/null 2>&1; then
    echo 'Install ubuntu-desktop-minimal, linux-generic, zenity, python3-gi, openssh-server, desktop-file-utils, wl-clipboard, fonts-noto-color-emoji and gnome-shell-extension-appindicator first.' >&2
    exit 1
fi
mkdir -p /etc/gdm3 /var/lib/AccountsService/users
cat > /etc/gdm3/custom.conf <<CONFIG
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=$user
CONFIG
cat > "/var/lib/AccountsService/users/$user" <<CONFIG
[User]
Session=ubuntu
XSession=ubuntu
SystemAccount=false
CONFIG
# Use only the shared key for SSH; unlock the local account for GDM autologin.
passwd -d "$user"
printf 'PasswordAuthentication no\n' > /etc/ssh/sshd_config.d/90-lhc-e2e.conf
mkdir -p /etc/cloud
: > /etc/cloud/cloud-init.disabled
systemctl set-default graphical.target
systemctl enable ssh
# No input membership in the frozen base: the runner checks missing permissions.
gpasswd -d "$user" input 2>/dev/null || true
guest_home=$(getent passwd "$user" | cut -d: -f6)
install -d -o "$user" -g "$(id -gn "$user")" "$guest_home/.config"
printf 'yes\n' > "$guest_home/.config/gnome-initial-setup-done"
chown "$user:$(id -gn "$user")" "$guest_home/.config/gnome-initial-setup-done"
install -d -o "$user" -g "$(id -gn "$user")" "$guest_home/.config/gnome-initial-setup"
: > "$guest_home/.config/gnome-initial-setup/upgrade-26.04-done"
chown "$user:$(id -gn "$user")" "$guest_home/.config/gnome-initial-setup/upgrade-26.04-done"
runuser -u "$user" -- dbus-run-session sh -c '
    gsettings set org.gnome.desktop.session idle-delay 0
    gsettings set org.gnome.desktop.screensaver lock-enabled false
    gsettings set org.gnome.settings-daemon.plugins.power sleep-inactive-ac-timeout 0
    version=$(gnome-shell --version | cut -d " " -f 3)
    gsettings set org.gnome.shell welcome-dialog-last-shown-version "$version"
'
