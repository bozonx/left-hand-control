# -*- mode: ruby -*-
# vi: set ft=ruby :
#
# Local E2E VM matrix for Linux desktop environments.
#
# Quick start:
#   vagrant up fedora-kde --provider=virtualbox
#   vagrant ssh fedora-kde
#   cd /vagrant && pnpm install && pnpm test:e2e:linux-kde

Vagrant.configure("2") do |config|
  config.vm.provider :libvirt do |lv, override|
    lv.memory = 4096
    lv.cpus = 4
    lv.graphics_type = "spice"
    lv.video_type = "virtio"
  end

  config.vm.provider :virtualbox do |vb, override|
    vb.memory = 4096
    vb.cpus = 4
    vb.gui = true
    vb.customize ["modifyvm", :id, "--vram", "128"]
  end

  UBUNTU_BOX = "ubuntu/noble64"
  FEDORA_BOX = "bento/fedora-41"

  def base_provision(vm, name)
    vm.vm.provision "system-#{name}", type: "shell", privileged: true, inline: yield(:system)
    vm.vm.provision "dev-#{name}", type: "shell", privileged: false, inline: yield(:dev)
  end

  config.vm.define "ubuntu-gnome" do |gnome|
    gnome.vm.box = UBUNTU_BOX
    gnome.vm.hostname = "ubuntu-gnome"
    base_provision(gnome, "ubuntu-gnome") do |stage|
      case stage
      when :system
        <<~SH
          set -e
          export DEBIAN_FRONTEND=noninteractive
          apt-get update
          apt-get install -y ubuntu-desktop-minimal gdm3 gnome-session gnome-shell
          apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev \
            librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
            libssl-dev pkg-config build-essential curl wget git file
          apt-get install -y xdotool xdg-desktop-portal-gnome xdg-desktop-portal
          curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
          apt-get install -y nodejs
          npm install -g pnpm@9
          ln -sf "$(which pnpm)" /usr/local/bin/pnpm
          systemctl set-default graphical.target
          systemctl enable gdm3
          sed -i 's/^#*AutomaticLoginEnable=.*/AutomaticLoginEnable=true/' /etc/gdm3/custom.conf
          sed -i 's/^#*AutomaticLogin=.*/AutomaticLogin=vagrant/' /etc/gdm3/custom.conf
        SH
      when :dev
        <<~SH
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          . "$HOME/.cargo/env"
          rustup default stable
          cargo install tauri-driver --locked
        SH
      end
    end
  end

  config.vm.define "ubuntu-kde" do |kde|
    kde.vm.box = UBUNTU_BOX
    kde.vm.hostname = "ubuntu-kde"
    base_provision(kde, "ubuntu-kde") do |stage|
      case stage
      when :system
        <<~SH
          set -e
          export DEBIAN_FRONTEND=noninteractive
          apt-get update
          apt-get install -y kde-plasma-desktop sddm
          apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev \
            librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
            libssl-dev pkg-config build-essential curl wget git file
          apt-get install -y qdbus-qt5 qdbus-qt6 xdotool \
            xdg-desktop-portal-kde xdg-desktop-portal
          curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
          apt-get install -y nodejs
          npm install -g pnpm@9
          ln -sf "$(which pnpm)" /usr/local/bin/pnpm
          systemctl set-default graphical.target
          systemctl enable sddm
          mkdir -p /etc/sddm.conf.d
          cat > /etc/sddm.conf.d/autologin.conf <<EOF
[Autologin]
User=vagrant
Session=plasma.desktop
EOF
        SH
      when :dev
        <<~SH
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          . "$HOME/.cargo/env"
          rustup default stable
          cargo install tauri-driver --locked
        SH
      end
    end
  end

  config.vm.define "fedora-gnome" do |gnome|
    gnome.vm.box = FEDORA_BOX
    gnome.vm.hostname = "fedora-gnome"
    base_provision(gnome, "fedora-gnome") do |stage|
      case stage
      when :system
        <<~SH
          set -e
          dnf update -y
          dnf group install -y "GNOME Desktop Environment"
          dnf install -y gdm
          dnf install -y webkit2gtk4.1 webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
            librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
            openssl-devel pkgconf gcc curl wget git file
          dnf install -y xdotool xdg-desktop-portal-gnome xdg-desktop-portal
          dnf install -y nodejs npm
          npm install -g pnpm@9
          ln -sf "$(which pnpm)" /usr/local/bin/pnpm
          systemctl set-default graphical.target
          systemctl enable gdm
          sed -i 's/^#*AutomaticLoginEnable=.*/AutomaticLoginEnable=true/' /etc/gdm/custom.conf
          sed -i 's/^#*AutomaticLogin=.*/AutomaticLogin=vagrant/' /etc/gdm/custom.conf
        SH
      when :dev
        <<~SH
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          . "$HOME/.cargo/env"
          rustup default stable
          cargo install tauri-driver --locked
        SH
      end
    end
  end

  config.vm.define "fedora-kde" do |kde|
    kde.vm.box = FEDORA_BOX
    kde.vm.hostname = "fedora-kde"
    base_provision(kde, "fedora-kde") do |stage|
      case stage
      when :system
        <<~SH
          set -e
          dnf update -y
          dnf group install -y "KDE Plasma Workspaces"
          dnf install -y sddm
          dnf install -y webkit2gtk4.1 webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
            librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
            openssl-devel pkgconf gcc curl wget git file
          dnf install -y qdbus xdotool qt6-qttools \
            xdg-desktop-portal-kde xdg-desktop-portal
          dnf install -y nodejs npm
          npm install -g pnpm@9
          ln -sf "$(which pnpm)" /usr/local/bin/pnpm
          systemctl set-default graphical.target
          systemctl enable sddm
          mkdir -p /etc/sddm.conf.d
          cat > /etc/sddm.conf.d/autologin.conf <<EOF
[Autologin]
User=vagrant
Session=plasma.desktop
EOF
        SH
      when :dev
        <<~SH
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          . "$HOME/.cargo/env"
          rustup default stable
          cargo install tauri-driver --locked
        SH
      end
    end
  end

  config.vm.define "fedora-sway" do |sway|
    sway.vm.box = FEDORA_BOX
    sway.vm.hostname = "fedora-sway"
    base_provision(sway, "fedora-sway") do |stage|
      case stage
      when :system
        <<~SH
          set -e
          dnf update -y
          dnf install -y sway waybar swayidle swaylock foot lightdm
          dnf install -y webkit2gtk4.1 webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
            librsvg2-devel libsoup3-devel javascriptcoregtk4.1-devel \
            openssl-devel pkgconf gcc curl wget git file
          dnf install -y xdotool xdg-desktop-portal-wlr xdg-desktop-portal
          dnf install -y nodejs npm
          npm install -g pnpm@9
          ln -sf "$(which pnpm)" /usr/local/bin/pnpm
          systemctl set-default graphical.target
          systemctl enable lightdm
          mkdir -p /etc/lightdm/lightdm.conf.d
          cat > /etc/lightdm/lightdm.conf.d/autologin.conf <<EOF
[Seat:*]
autologin-user=vagrant
autologin-session=sway
EOF
        SH
      when :dev
        <<~SH
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          . "$HOME/.cargo/env"
          rustup default stable
          cargo install tauri-driver --locked
        SH
      end
    end
  end
end
