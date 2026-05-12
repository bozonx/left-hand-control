# -*- mode: ruby -*-
# vi: set ft=ruby :

Vagrant.configure("2") do |config|
  # Shared VM settings for libvirt (KVM/QEMU)
  config.vm.provider :libvirt do |lv|
    lv.memory = 4096
    lv.cpus = 4
    lv.graphics_type = "spice"
    lv.video_type = "virtio"
  end

  # --- Ubuntu 24.04 GNOME ---
  config.vm.define "ubuntu-gnome" do |gnome|
    gnome.vm.box = "generic/ubuntu2404"
    gnome.vm.hostname = "ubuntu-gnome"
    gnome.vm.provision "shell", privileged: true, inline: <<~SHELL
      export DEBIAN_FRONTEND=noninteractive
      apt-get update

      # Desktop Environment
      apt-get install -y ubuntu-desktop-minimal gdm3 gnome-session gnome-shell

      # Tauri build deps
      apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev \
        librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
        libssl-dev pkg-config build-essential curl wget git file

      # E2E deps
      apt-get install -y xdotool xdg-desktop-portal-gnome xdg-desktop-portal

      # Node + pnpm
      curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
      apt-get install -y nodejs
      npm install -g pnpm@9

      # Rust
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      . "$HOME/.cargo/env"
      rustup default stable

      # tauri-driver
      cargo install tauri-driver --locked

      # Autologin for vagrant user via GDM
      sed -i 's/^#*AutomaticLoginEnable=.*/AutomaticLoginEnable=true/' /etc/gdm3/custom.conf
      sed -i 's/^#*AutomaticLogin=.*/AutomaticLogin=vagrant/' /etc/gdm3/custom.conf
    SHELL
  end

  # --- Ubuntu 24.04 KDE ---
  config.vm.define "ubuntu-kde" do |kde|
    kde.vm.box = "generic/ubuntu2404"
    kde.vm.hostname = "ubuntu-kde"
    kde.vm.provision "shell", privileged: true, inline: <<~SHELL
      export DEBIAN_FRONTEND=noninteractive
      apt-get update

      # Desktop Environment
      apt-get install -y kde-plasma-desktop sddm

      # Tauri build deps
      apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev \
        librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
        libssl-dev pkg-config build-essential curl wget git file

      # KDE E2E deps
      apt-get install -y qdbus-qt5 qdbus-qt6 xdotool \
        xdg-desktop-portal-kde xdg-desktop-portal

      # Node + pnpm
      curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
      apt-get install -y nodejs
      npm install -g pnpm@9

      # Rust
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      . "$HOME/.cargo/env"
      rustup default stable

      # tauri-driver
      cargo install tauri-driver --locked

      # Autologin for vagrant user via SDDM
      mkdir -p /etc/sddm.conf.d
      cat > /etc/sddm.conf.d/autologin.conf <<EOF
[Autologin]
User=vagrant
Session=plasma.desktop
EOF
    SHELL
  end

  # --- Fedora 41 GNOME ---
  config.vm.define "fedora-gnome" do |gnome|
    gnome.vm.box = "generic/fedora41"
    gnome.vm.hostname = "fedora-gnome"
    gnome.vm.provision "shell", privileged: true, inline: <<~SHELL
      dnf update -y

      # Desktop Environment
      dnf group install -y "GNOME Desktop Environment"
      dnf install -y gdm

      # Tauri build deps
      dnf install -y webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
        librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
        openssl-devel pkgconf gcc curl wget git file

      # E2E deps
      dnf install -y xdotool xdg-desktop-portal-gnome xdg-desktop-portal

      # Node + pnpm
      dnf install -y nodejs20 npm
      npm install -g pnpm@9

      # Rust
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      . "$HOME/.cargo/env"
      rustup default stable

      # tauri-driver
      cargo install tauri-driver --locked

      # Autologin for vagrant user via GDM
      sed -i 's/^#*AutomaticLoginEnable=.*/AutomaticLoginEnable=true/' /etc/gdm/custom.conf
      sed -i 's/^#*AutomaticLogin=.*/AutomaticLogin=vagrant/' /etc/gdm/custom.conf
    SHELL
  end

  # --- Fedora 41 KDE ---
  config.vm.define "fedora-kde" do |kde|
    kde.vm.box = "generic/fedora41"
    kde.vm.hostname = "fedora-kde"
    kde.vm.provision "shell", privileged: true, inline: <<~SHELL
      dnf update -y

      # Desktop Environment
      dnf group install -y "KDE Plasma Workspaces"
      dnf install -y sddm

      # Tauri build deps
      dnf install -y webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
        librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
        openssl-devel pkgconf gcc curl wget git file

      # KDE E2E deps
      dnf install -y qdbus xdotool qt6-qttools \
        xdg-desktop-portal-kde xdg-desktop-portal

      # Node + pnpm
      dnf install -y nodejs20 npm
      npm install -g pnpm@9

      # Rust
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      . "$HOME/.cargo/env"
      rustup default stable

      # tauri-driver
      cargo install tauri-driver --locked

      # Autologin for vagrant user via SDDM
      mkdir -p /etc/sddm.conf.d
      cat > /etc/sddm.conf.d/autologin.conf <<EOF
[Autologin]
User=vagrant
Session=plasma.desktop
EOF
    SHELL
  end

  # --- Fedora 41 Sway ---
  config.vm.define "fedora-sway" do |sway|
    sway.vm.box = "generic/fedora41"
    sway.vm.hostname = "fedora-sway"
    sway.vm.provision "shell", privileged: true, inline: <<~SHELL
      dnf update -y

      # Desktop Environment
      dnf install -y sway waybar swayidle swaylock foot
      dnf install -y lightdm

      # Tauri build deps
      dnf install -y webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
        librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
        openssl-devel pkgconf gcc curl wget git file

      # E2E deps
      dnf install -y xdotool xdg-desktop-portal-wlr xdg-desktop-portal

      # Node + pnpm
      dnf install -y nodejs20 npm
      npm install -g pnpm@9

      # Rust
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      . "$HOME/.cargo/env"
      rustup default stable

      # tauri-driver
      cargo install tauri-driver --locked

      # Autologin for vagrant user via LightDM + sway
      mkdir -p /etc/lightdm/lightdm.conf.d
      cat > /etc/lightdm/lightdm.conf.d/autologin.conf <<EOF
[Seat:*]
autologin-user=vagrant
autologin-session=sway
EOF
    SHELL
  end
end
