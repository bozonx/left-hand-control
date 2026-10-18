# Руководство разработчика: запуск, проверки, e2e и выпуск

Единое руководство по работе с репозиторием: как собрать и запустить приложение
при разработке, какие проверки и где запускать, как устроены e2e-уровни (на хосте
и в виртуальных машинах) и как выпустить релиз. Все команды выполняются из корня
репозитория.

Связанные документы:

- [AGENTS.md](../../AGENTS.md) — архитектура, правила кода, границы изменений;
- [features.md](../features.md) — как работают функции приложения;
- [platforms.md](platforms.md) — матрица окружений и добавление backend'ов;
- [install-linux.md](../install-linux.md) — установка для пользователей (входит в архив релиза);
- [testing/windows.md](../testing/windows.md) — установка Windows VM;
- [testing/manual-acceptance.md](../testing/manual-acceptance.md) — чек-лист ручной приёмки;
- [apps/slint-shell/README.md](../../apps/slint-shell/README.md) — стенды задержек, Spell lifecycle, диагностика.

Основная среда разработки и приёмки — **KDE Plasma 6 на Wayland**. Windows и
macOS собираются с минимальными адаптерами оболочки. На Windows есть mapper
(хуки сессии и `SendInput`), ещё не принятый; на macOS mapper не реализован.

## 1. Окружение

Rust stable не ниже `1.92` (edition 2024, `rust-version` в корневом `Cargo.toml`).
Skia собирается из исходников при первой сборке, поэтому нужны clang, cmake, ninja
и Python.

**Manjaro / Arch:**

```sh
sudo pacman -S --needed base-devel clang cmake ninja python pkgconf \
  fontconfig freetype2 libxkbcommon wayland dbus pam noto-fonts-emoji \
  wl-clipboard
```

**Debian / Ubuntu:**

```sh
sudo apt install build-essential clang cmake ninja-build python3 pkg-config \
  libfontconfig1-dev libfreetype-dev libxkbcommon-dev libwayland-dev \
  libdbus-1-dev libpam0g-dev fonts-noto-color-emoji wl-clipboard
```

`libpam0g-dev` (`pam` на Arch) нужен только для линковки тестов Spell; release-бинарник
libpam не требует.

Для отдельных функций дополнительно:

| Функция | Что нужно |
| --- | --- |
| Mapper (перехват клавиатуры) | Чтение `/dev/input/event*` (группа `input`) и запись в `/dev/uinput` (udev-правило) |
| Глобальные хоткеи Ctrl+Alt+F11 / F12 | Чтение `/dev/input/event*`; устройство не захватывается |
| Ввод литерального текста (`text:`) | `xdg-desktop-portal` и бэкенд окружения (`xdg-desktop-portal-kde`) |
| Условия по активному окну | KDE Wayland — ничего (KWin-скрипт через D-Bus); Hyprland — `hyprctl`; Sway — `swaymsg`; другие wlroots-композиторы — ничего (`wlr-foreign-toplevel`); X11 — `xprop`, для условий по заголовку и окон без рамки ещё `xdotool` (+ `xrandr`) |
| Диагностический стенд возврата ввода (`--features probes`, `LHC_INSERT=1`) | KDE Wayland, data-control, запись в `/dev/uinput` |

Права на устройства ввода для разработки (один раз, затем перезайти в сессию):

```sh
sudo install -D -m 644 apps/slint-shell/packaging/linux/60-left-hand-control.rules \
  /etc/udev/rules.d/60-left-hand-control.rules
sudo install -D -m 644 apps/slint-shell/packaging/linux/modules-load.conf \
  /etc/modules-load.d/left-hand-control.conf
sudo modprobe uinput
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=misc --sysname-match=uinput
sudo usermod -aG input "$USER"
```

Правило даёт активной локальной сессии (`uaccess`) и группе `input` запись в
`/dev/uinput`. Чтение `/dev/input/event*` требует группы `input`, а она даёт всем
процессам пользователя чтение любых клавиатур и мышей (права уровня кейлоггера).
Поэтому ни инсталлятор, ни пакеты не добавляют пользователя в группу без явной команды.

## 2. Запуск при разработке

```sh
cargo run -p slint-shell                                   # попапы — окна winit
LHC_POPUPS=auto cargo run -p slint-shell --features spell   # попапы — layer-shell, где он есть
cargo build -p slint-shell --release --features spell       # release-сборка
```

Окна при старте скрыты, приложение живёт в трее. Настройки открываются кликом по
иконке трея или командой `target/debug/left-hand-control show settings`.

| `LHC_POPUPS` | Поведение |
| --- | --- |
| `winit` (по умолчанию) | Попапы — окна без рамки в том же процессе |
| `auto` | Если композитор поддерживает `zwlr_layer_shell_v1` (KDE, Hyprland, Sway) — отдельный процесс Spell; иначе (GNOME) — winit |
| `spell` | Только layer-shell; без протокола запуск завершается ошибкой |

`auto` и `spell` требуют сборки с `--features spell`. Рендерер окна настроек задаёт
`SLINT_BACKEND`: `winit-software`, `winit-femtovg` или `winit-skia`; цветные emoji в
окнах winit рисует только `winit-skia`. Spell-попапы всегда используют Skia software.

### Hot reload интерфейса

```sh
SLINT_LIVE_PREVIEW=1 cargo run -p slint-shell --features slint/live-preview --features spell
```

Перед запуском завершите работающий экземпляр. После сохранения `.slint`
интерфейс перезагружается, связи с Rust сохраняются; при синтаксической ошибке
остаётся предыдущая версия. Изменения Rust и интерфейса между Slint и Rust
(переименование свойства или callback) требуют пересборки. Live Preview включайте
только из командной строки, никогда в обычных или release-сборках. Русский PO-каталог
в этом режиме подключается переводчиком времени выполнения.

### Конфигурация

Debug-сборки никогда не трогают реальный конфиг пользователя:

- по умолчанию — `<repo>/dev-files/linux/.config/dev.bozonx.left-hand-control/config.json`,
  данные и библиотека раскладок — в `dev-files/linux/.local/share/dev.bozonx.left-hand-control/`;
- `LHC_DEV_DIR=/путь` задаёт другой каталог с той же структурой.

Release-сборки используют `~/.config/dev.bozonx.left-hand-control/` и
`~/.local/share/dev.bozonx.left-hand-control/`. Пути разрешает только
`lhc_core::storage::StoragePaths::resolve()`. Конфиг читается при запуске и меняется через приложение. После ручного
редактирования файла перезапустите приложение; сохранение из работающего приложения
может перезаписать ручные изменения. Файлы пишутся атомарно через временный
файл с правами `0600`.

### Управление запущенным экземпляром

Одновременно работает один экземпляр на `XDG_RUNTIME_DIR`. Бинарник с аргументами —
клиент: отправляет команду через IPC и завершается. Второй запуск без аргументов
открывает настройки работающего экземпляра.

```sh
target/debug/left-hand-control show settings      # show emoji|quick [страница]
target/debug/left-hand-control hide
target/debug/left-hand-control toggle-mapper
target/debug/left-hand-control game toggle        # auto|on|off|toggle
target/debug/left-hand-control preferences dark ru
target/debug/left-hand-control ping
target/debug/left-hand-control quit
target/debug/left-hand-control --version          # --version и --help не требуют экземпляра
```

Ответ `queued` означает, что команда поставлена в очередь UI, а не что окно уже
готово к вводу.

### Переменные окружения

| Переменная | Назначение |
| --- | --- |
| `RUST_LOG` | Фильтр логов; по умолчанию `info,zbus=warn,tracing=warn` |
| `LHC_DEV_DIR` | Каталог конфигурации debug-сборок |
| `SLINT_BACKEND` | Рендерер окна настроек |
| `LHC_POPUPS` | `winit` / `auto` / `spell` |
| `LHC_HOTKEYS=off` | Не слушать хоткеи Ctrl+Alt+F11 / F12 |
| `LHC_INPUT=/dev/input/eventN` | Хоткеи только с одного устройства |
| `LHC_OUTPUT=<имя>` | Монитор для Spell-попапов |
| `LHC_METRICS=/путь.csv` | CSV задержек для измерительных скриптов |
| `LHC_SOCKET` | Имя IPC-сокета; нужно для параллельного тестового экземпляра |
| `LHC_INSERT=1` | Только с `--features probes`: диагностический Spell worker с возвратом фокуса |

Диагностические переменные стендов (`LHC_TEST_INJECT`, `LHC_ISOLATED_INPUT`,
`LHC_SPELL_LIFECYCLE`, `LHC_SPELL_INITIAL`) описаны в `apps/slint-shell/README.md`.

Второй экземпляр рядом с основным (трей у него тоже появится, mapper запустится
только в одном процессе):

```sh
LHC_DEV_DIR=/tmp/lhc-dev LHC_SOCKET=lhc-dev.sock LHC_HOTKEYS=off cargo run -p slint-shell
LHC_SOCKET=lhc-dev.sock target/debug/left-hand-control quit
```

### Окружения

| Окружение | Состояние |
| --- | --- |
| KDE Plasma 6, Wayland | Основная среда: Spell-попапы, фокус, вставка, раскладка, системные действия |
| Hyprland, Sway | Layer-shell есть, не принято |
| GNOME Wayland | Нет layer-shell: `auto` уходит в winit; трею нужно расширение AppIndicator |
| X11 | Не поддерживается: работает как есть (окна winit, поведение зависит от WM), приёмки и e2e нет |
| Windows | Оболочка и mapper (хуки сессии), приёмка не пройдена |
| macOS | Оболочка собирается и запускается; mapper — заглушка |

Подробная матрица — в [platforms.md](platforms.md).

## 3. Проверки

### Перед коммитом (то же, что CI)

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo clippy --locked -p lhc-core -p slint-shell \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets              # сборка без Spell
```

Один тест: `cargo test --locked -p slint-shell --lib <имя>` или
`cargo test --locked -p lhc-core <имя>`. Тест
`every_slint_string_has_a_russian_translation` проверяет переводы всех строк `@tr`.

### UI-сценарии

Нужна графическая сессия; каждый сценарий использует временный конфиг и сам
завершается: `cargo run --locked -p slint-shell --example <имя>`.

| Сценарий | Что проверяет |
| --- | --- |
| `editor -- --smoke` | Редактор клавиш, переключение RU/EN (обязателен при изменениях UI) |
| `interactions` | Настройки, сохранение, навигация, тема, язык, скрытые попапы (обязателен при изменениях UI) |
| `library` | Библиотека, активация, условия |
| `action-picker` | Диалог выбора клавиш и действий |
| `macros`, `menus` | Редакторы макросов, Эмоджи и Быстрых действий (`LHC_MENUS_NAV=1` — навигация) |
| `undo-redo`, `inline-edit` | История текстовых полей и inline-редакторы |
| `drag-scroll`, `notifications` | Перетаскивание с автопрокруткой, тосты |
| `conditions`, `layer-keys` | Диалог условий (`SNAP=/tmp/x.ppm`), дополнительные клавиши слоёв |

С `LHC_EDITOR_SNAPSHOTS=<каталог>` пример `interactions` сохраняет снимки экранов.
Те же сценарии работают в Live Preview: добавьте `SLINT_LIVE_PREVIEW=1` и
`--features slint/live-preview`.

### Скрипты оболочки

| Команда | Что проверяет |
| --- | --- |
| `python3 apps/slint-shell/scripts/check-recovery.py --popups spell` | Восстановление конфига и доставку попапа после перезапуска Spell (`--popups winit`; `--live-preview` — перезагрузку `.slint`) |
| `python3 apps/slint-shell/scripts/check-preferences.py` | Смену темы и языка по IPC при открытых попапах, штатный `quit` |
| `python3 apps/slint-shell/scripts/check-worker-crash.py target/debug/left-hand-control` | Перезапуск упавшего Spell worker |
| `python3 apps/slint-shell/scripts/check-linux-install.py` | Инсталлятор архива во временном префиксе (нужен `desktop-file-validate`) |
| `python3 apps/slint-shell/scripts/test-clipboard.py` | Вставку через буфер в отдельном виртуальном KWin (нужны `kwin_wayland`, `dbus-run-session`); рабочий буфер не затрагивается |

### Игровой режим

```sh
cargo test --locked -p lhc-core gamemode
cargo test --locked -p lhc-core compositor_owner_restart
cargo test --locked -p lhc-core --test game_mode_lifecycle
cargo test --locked -p slint-shell --lib game_mode
```

Проверки KWin запускают собственный `dbus-daemon`; рабочий KWin не трогается.
Ручная приёмка игрового режима — раздел 13 [чек-листа](../testing/manual-acceptance.md).

Стенды задержек и lifecycle (`bench-*`, feature `probes`, `scripts/bench-virtual.sh`)
описаны в `apps/slint-shell/README.md`.

## 4. E2E-проверки

### Уровни

| Уровень | Что проверяет | Где | Когда |
| --- | --- | --- | --- |
| L0. Unit/integration | Ядро, конфиг, движок, IPC, i18n | GitHub CI (`ci.yml`) | Каждый push/PR |
| L1. Headless e2e маппера | Путь evdev → engine → uinput без рабочего стола | Хост; GitHub CI, если на раннере есть `uinput` | Каждый PR |
| L2. Smoke пакетов | Зависимости, файлы, установка и удаление в контейнерах | GitHub CI (`release.yml`), локально в Docker | Тег релиза |
| L3. Desktop-сценарии | UI, попапы, Spell worker, настройки + L1 | Машина разработчика (KDE Wayland) | Перед релизом и после изменений UI |
| L4. VM-матрица | Установка пакета с нуля, права, перелогин, автозапуск, трей, mapper на клавишах QEMU, `text:` через portal, обновление, удаление; Windows-оболочка | Локальные KVM/QEMU VM | Перед релизом, после изменений упаковки, прав, трея, portal |
| L5. Ручная приёмка | [manual-acceptance.md](../testing/manual-acceptance.md) | Реальное железо | Мажорные релизы |

Desktop-e2e (L3–L5) в GitHub **не запускаются**: на облачных раннерах нет
Plasma-сессии, трея и portal; VM без KVM слишком медленная; self-hosted раннер в
публичном репозитории позволил бы PR из форка выполнить код на машине разработчика.

Результаты L3/L4 пишутся в `docs/testing/results/<linux|windows>/<дата>[-vm-<имя>].txt`:
дата, коммит (с пометкой о незакоммиченных изменениях), окружение и список шагов.

### L1: e2e маппера

```sh
cargo test --locked -p lhc-core --test mapper_e2e -- --ignored
```

`crates/lhc-core/tests/mapper_e2e.rs` создаёт через uinput виртуальную «физическую»
клавиатуру, запускает mapper с тестовым профилем на ней и читает выход его
виртуального устройства. Сценарии: passthrough, remap, chord, tap/hold, слой,
double tap, макрос с паузой, остановка без залипших клавиш, отключение устройства.
Оба устройства захватываются (grab), нажатия не попадают в сессию; межпроцессный
лок mapper переносится во временный `XDG_RUNTIME_DIR`, поэтому тест не мешает
запущенному приложению. Нужны запись в `/dev/uinput` и чтение `/dev/input`.

### L3: прогон на машине разработчика

```sh
apps/slint-shell/scripts/e2e-local.sh            # --no-mapper пропускает L1
```

Скрипт собирает оболочку со Spell, по очереди запускает все UI-сценарии,
`check-recovery.py`, `check-preferences.py`, `check-worker-crash.py`,
`check-linux-install.py` и L1, печатает сводку и пишет
`docs/testing/results/linux/<дата>.txt`. Во время прогона не пользуйтесь
клавиатурой и мышью.

### L4: e2e в виртуальных машинах

Стенд — `apps/slint-shell/scripts/vm/`:

| Файл | Где выполняется | Назначение |
| --- | --- | --- |
| `vmlib.py` | Хост | `vm.json`, одноразовый overlay, QEMU с QMP, нажатия клавиш, скриншоты, ssh, отчёт |
| `run-linux.py` | Хост | Сценарий Linux VM |
| `linux-guest.py` | Гость Linux (копируется сам) | Шаги внутри сессии Plasma |
| `prepare-linux-guest.sh` | Гость Linux, один раз | Подготовка базового образа |
| `run-windows.py` | Хост | Сценарий Windows VM |
| `windows-guest.ps1` | Гость Windows (задача LHC-E2E) | Запуск `check-windows-automated.ps1` в интерактивной сессии |
| `prepare-windows-guest.ps1` | Гость Windows, один раз | Подготовка базового образа |

Каждый прогон создаёт `runs/<время>/run.qcow2` поверх замороженного базового образа
и удаляет его после остановки (`--keep` оставляет), поэтому VM всегда стартует с
чистого состояния. Логи, скриншоты QMP и журнал гостя остаются в `runs/<время>/logs/`.
`--show` показывает экран VM в окне GTK, по умолчанию VM работает без окна.

**Хост.** Нужны `qemu-system-x86_64`, `qemu-img`, доступ к `/dev/kvm`, `ssh`/`scp`,
для Windows — OVMF с Secure Boot и `swtpm`. Для Linux-гостей с Plasma хватает
стандартного `VGA` (Plasma работает на llvmpipe). `virtio-vga` на Arch/Manjaro
требует двух пакетов, `qemu-hw-display-virtio-vga` и `qemu-hw-display-virtio-gpu`:
без второго QEMU 11 падает с SIGSEGV при создании устройства. Ключ
доступа к гостям создаётся один раз: `ssh-keygen -t ed25519 -N '' -f ~/.ssh/lhc-vm`.
Образы лежат вне репозитория, по каталогу на VM, например `/mnt/disk2/vm/lhc-fedora-kde/`.

**`vm.json`** в каталоге VM:

```json
{
  "name": "fedora44-kde",
  "base": "base.qcow2",
  "user": "tester",
  "ssh_key": "~/.ssh/lhc-vm",
  "ssh_port": 2231,
  "vga": "VGA",
  "memory": 6144,
  "cpus": 4,
  "install": "sudo dnf install -y {package}",
  "reinstall": "sudo dnf reinstall -y {package}",
  "remove": "sudo dnf remove -y left-hand-control",
  "portal_keys": ["ret"]
}
```

| Ключ | Значение |
| --- | --- |
| `base` | Замороженный образ (путь относительно каталога VM) |
| `user`, `ssh_key`, `ssh_port` | Пользователь с автологином; ключ; порт хоста, проброшенный на 22 гостя (у каждой VM свой) |
| `vga`, `nic`, `cpu`, `disk_bus` | Устройства QEMU; по умолчанию без `vga`, `virtio-net-pci`, `host`, `virtio` |
| `install`, `reinstall`, `remove` | Команды пакетного менеджера; `{package}` — путь к пакету в госте. Для `.tar.gz` не нужны: используется `install-linux.sh`. Без `reinstall` шаг обновления пропускается |
| `portal_keys` | Клавиши QEMU (qcode), подтверждающие диалог RemoteDesktop portal |
| `portal_deny_click` | Точка кнопки Deny в этом диалоге, доли ширины и высоты экрана, или список точек, которые нажимаются по очереди; по умолчанию `[0.738, 0.659]` (1280×800, Plasma 6.7). Plasma 6.3 после первого Cancel показывает второй диалог: `[[0.678, 0.655], [0.610, 0.655]]` |
| `uefi`, `cache_disk` | Для Windows: `{"code", "vars", "tpm"}` и постоянный диск кэша сборки |

Для Debian/Kubuntu: `"install": "sudo apt-get install -y {package}"`,
`"reinstall": "sudo apt-get install -y --reinstall {package}"`,
`"remove": "sudo apt-get remove -y left-hand-control"`; для openSUSE — `zypper
--non-interactive install --allow-unsigned-rpm {package}` и
`zypper --non-interactive install --force --allow-unsigned-rpm {package}`.

#### Linux VM: подготовка образа (один раз)

| VM | Зачем |
| --- | --- |
| Fedora KDE (Plasma 6, Wayland) | rpm, SELinux, основная RPM-аудитория |
| Debian 13 KDE | deb, базовый glibc, консервативные пакеты |
| Kubuntu (актуальный релиз с Plasma 6) | deb, Ubuntu-специфика |
| openSUSE Tumbleweed KDE | rpm через zypper |
| Arch KDE | tar.gz и будущий AUR |

Начинать с Fedora KDE и Debian 13 KDE.

**Fedora KDE из облачного образа** — без интерактивного установщика, около 15 минут
(так собран `/mnt/disk2/vm/lhc-fedora-kde`, Fedora 44, Plasma 6.7):

```sh
repo=$PWD; vm=/mnt/disk2/vm/lhc-fedora-kde; mkdir -p "$vm/install" && cd "$vm/install"
u=https://dl.fedoraproject.org/pub/fedora/linux/releases/44/Cloud/x86_64/images
curl -fLO "$u/Fedora-Cloud-44-1.7-x86_64-CHECKSUM" -fLO "$u/Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2"
sha256sum -c --ignore-missing Fedora-Cloud-44-1.7-x86_64-CHECKSUM
qemu-img convert -O qcow2 Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2 ../base.qcow2
qemu-img resize ../base.qcow2 40G
mkdir -p seed
printf 'instance-id: lhc-fedora-kde\nlocal-hostname: lhc-fedora-kde\n' > seed/meta-data
cat > seed/user-data <<EOF
#cloud-config
users:
  - name: tester
    groups: [wheel]
    sudo: "ALL=(ALL) NOPASSWD:ALL"
    shell: /bin/bash
    ssh_authorized_keys: ["$(cat ~/.ssh/lhc-vm.pub)"]
EOF
xorriso -as mkisofs -o seed.iso -V cidata -J -r seed
qemu-system-x86_64 -enable-kvm -machine q35 -cpu host -smp 4 -m 6144 \
  -drive file=../base.qcow2,if=virtio -drive file=seed.iso,media=cdrom \
  -vga none -device VGA -display none -device qemu-xhci -device usb-tablet \
  -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:2231-:22 -daemonize -pidfile qemu.pid
ssh='ssh -i ~/.ssh/lhc-vm -p 2231 -o StrictHostKeyChecking=no tester@127.0.0.1'
$ssh 'sudo dnf -y install @kde-desktop-environment && sudo systemctl set-default graphical.target'
$ssh 'cat > /tmp/prep.sh' < "$repo/apps/slint-shell/scripts/vm/prepare-linux-guest.sh"
$ssh "sudo sh /tmp/prep.sh tester '$(cat ~/.ssh/lhc-vm.pub)' && sudo systemctl reboot"
```

Затем шаги 3–4 ниже. Fedora 44 KDE использует Plasma Login Manager вместо SDDM;
`prepare-linux-guest.sh` пишет автологин в `/etc/plasmalogin.conf.d/` и отключает
cloud-init, чтобы одноразовые загрузки не искали источник данных.

**Debian 13 KDE из облачного образа** — так же, около 20 минут (так собран
`/mnt/disk2/vm/lhc-debian13-kde`, Debian 13.7, Plasma 6.3.6, `ssh_port` 2232). Отличия от
Fedora: образ `generic` с обычным ядром, а не облачным, как у `genericcloud`; группа `sudo` вместо `wheel`, KDE ставится `task-kde-desktop`:

```sh
u=https://cloud.debian.org/images/cloud/trixie/latest
curl -fLO "$u/SHA512SUMS" -fLO "$u/debian-13-generic-amd64.qcow2"
sha512sum -c --ignore-missing SHA512SUMS
# далее как для Fedora, с groups: [sudo] в user-data и
$ssh 'sudo cloud-init status --wait; sudo apt-get update && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y task-kde-desktop && sudo systemctl set-default graphical.target'
```

В `vm.json` — команды apt (см. выше) и `portal_deny_click` для Plasma 6.3.

**Другие дистрибутивы с установочного ISO:**

1. Создайте диск и установите дистрибутив с KDE Plasma 6 и обычным пользователем:
   ```sh
   vm=/mnt/disk2/vm/lhc-debian13-kde; mkdir -p "$vm"
   qemu-img create -f qcow2 "$vm/base.qcow2" 40G
   qemu-system-x86_64 -enable-kvm -machine q35 -cpu host -smp 4 -m 6144 \
     -drive file="$vm/base.qcow2",if=virtio -cdrom <iso> -boot d \
     -vga none -device VGA -display gtk,gl=off \
     -device qemu-xhci -device usb-tablet -nic user,model=virtio-net-pci
   ```
2. В госте от root выполните подготовку (скрипт можно передать через
   `python3 -m http.server` на хосте, гость видит хост как `10.0.2.2`):
   ```sh
   sh prepare-linux-guest.sh tester "$(cat ~/.ssh/lhc-vm.pub)"   # ключ — содержимое lhc-vm.pub с хоста
   ```
   Скрипт ставит `openssh-server`, `python3`, `kdialog`, `desktop-file-utils`,
   даёт пользователю `sudo` без пароля, включает автологин SDDM или Plasma Login в Plasma Wayland,
   отключает блокировку экрана и сон, убирает пользователя из группы `input`.
3. Перезагрузите гостя и убедитесь, что он сам входит в Plasma. Приложение не
   устанавливайте. Выключите VM и больше не запускайте `base.qcow2` напрямую:
   прогоны используют его как неизменяемую основу overlay (`chmod a-w base.qcow2`
   защищает от случайного запуска).
4. Создайте `vm.json`.

#### Linux VM: прогон

Пакет собирается `release-linux.sh` (раздел 5) или для быстрой проверки
`package-linux.sh` (только tar.gz):

```sh
apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/lhc-fedora-kde \
  --package target/release-artifacts/left-hand-control-<версия>-1.x86_64.rpm
```

Опция `--previous <старый .rpm/.deb>` сначала ставит предыдущий релиз, запускает
его с тестовым конфигом и обновляет до проверяемого пакета: версия должна смениться,
`config.json` — остаться байт в байт, приложение — запуститься с ним.

Каждый шаг ввода открывает окно `kdialog`, нажимает клавиши через QMP (как настоящая
USB-клавиатура) и сравнивает полученный текст с ожидаемым. Скриншот каждого окна
сохраняется в `logs/<шаг>.png`.

| Шаг | Что проверяется |
| --- | --- |
| `boot`, `upload`, `plasma-session`, `install` | Загрузка, автологин в Plasma Wayland, установка штатным менеджером или `install-linux.sh` |
| `installed-files` | `.desktop` (и `desktop-file-validate`), metainfo, иконки hicolor; для пакетов — udev-правило и запись в `/dev/uinput` для сессии через `uaccess` |
| `keyboard-probe` | Какой узел `/dev/input` получает клавиши QEMU `usb-kbd`; он становится `inputDevicePath` |
| `test-config` | Тестовый профиль (`E2E_LAYOUT` в `linux-guest.py`), команды разрешены, раскладки `us,ru`, два рабочих стола, автозапуск |
| `previous-release`, `upgrade-from-previous` | Только с `--previous`: обновление с сохранением конфига |
| `start-without-input`, `tray-item`, `no-mapper-without-input` | Без группы `input` приложение стартует, регистрирует элемент трея (`StatusNotifierWatcher`, Id `lhc-slint-shell`), а mapper не создаёт виртуальную клавиатуру |
| `autostart-entry` | XDG Autostart указывает на установленный бинарник |
| `relogin-with-input`, `autostart-launch`, `tray-after-login`, `mapper-running` | После добавления в `input` и перелогина приложение запускается автозапуском, трей на месте, mapper работает |
| `portal-deny` | Первый `text:` открывает диалог RemoteDesktop; стенд нажимает Deny мышью (`portal_deny_click`). Текст не вводится, следующий `text:` сразу завершается ошибкой «user cancelled» без нового диалога, приложение отвечает |
| `portal-consent` | После перезапуска mapper диалог появляется снова; подтверждение (`portal_keys`) — и текст вводится |
| `mapper-keys-and-text` | `a d c t` → `bXclhc-ok`: remap, chord, passthrough, `text:` |
| `text-unicode` | `text:Привет, ёж 😀` в раскладке us (символов нет в keymap) |
| `layer-tap-hold` | Слой на `F`: tap → `g`, hold + `j` → `k`, быстрый вложенный набор → `k`, «перекат» (F отпущена раньше J) → `gj`, `j` вне слоя → `j` (permissive hold, 200 мс) |
| `macro`, `command` | Макрос `KeyN, pause:300, text:mac, Shift+KeyO` → `nmacO`; `cmd:` создаёт файл-маркер |
| `system-actions` | `sys:switchDesktop2`, `kde:kwin/Switch to Desktop 1`, `sys:switchLayout2/1` — по состоянию KWin и `org.kde.keyboard` |
| `russian-layout` | В раскладке ru физические клавиши дают кириллицу (`a`→KeyB→`и`, `c`→`с`), `text:` вводится без искажений |
| `popups`, `emoji-ipc`, `quick-menu-rule` | IPC show/hide; эмодзи из `show emoji` + `Q` и быстрое действие из `app:showQuickMenu1` + `Q` вставляются в окно, бывшее активным до попапа |
| `emoji-hotkey` | Ctrl+Alt+F11 открывает Emoji, `Q` вставляет эмодзи, консоль не переключается на VT11 |
| `game-mode` | Правило с `gameMode`: Off, On (`game on`), Auto без игры и с процессом из `processMatchers` |
| `screen-lock` | Пароль `qbXc` набирается только через mapper (`q a d c`) и разблокирует экран; набранное на экране блокировки не попадает в окно за ним |
| `suspend-resume` | S3 и пробуждение через QMP, после этого mapper и `text:` работают |
| `cli`, `second-instance` | Повторный запуск передаёт управление работающему экземпляру; `hide`, `ping`, `--version`, `--help`; второй экземпляр со своим `LHC_SOCKET` не захватывает клавиатуру (блокировка mapper) |
| `keyboard-replug` | USB-клавиатура отключается и подключается в тот же порт; mapper должен продолжить работу. Если нет — проверяется нативный ввод, mapper перезапускается |
| `mapper-stop`, `mapper-start` | `toggle-mapper` останавливает и запускает mapper |
| `stuck-keys-on-stop`, `stuck-keys-on-kill` | Остановка mapper и `kill -9` при зажатых Shift и клавише слоя: после этого `abc` вводится без модификатора |
| `upgrade-over-running` | Переустановка пакета поверх запущенного приложения |
| `quit`, `remove` | Выход по IPC, удаление; конфиг пользователя остаётся |

Если шаги с диалогом portal падают, сначала откройте `portal-deny-*.png` и
`portal-consent.png`: у разных версий xdg-desktop-portal-kde размер диалога и кнопка
по умолчанию могут отличаться — подберите `portal_deny_click`/`portal_keys`. Отказ
нажимается мышью: нажатия клавиш во время ожидания `text:` отменяют само задание, и
отказ с клавиатуры не запоминается — следующий `text:` спрашивает снова. Разрешение
KDE запоминает и без токена приложения, поэтому `portal-deny` проверяется только на
чистом образе первым обращением к portal. Визуальный вид трея и попапов, клики по меню
трея остаются на L5; X11-сессия не проверяется.

#### Windows VM

Сценарии mapper на Windows (матрица в [windows-mapper.md](windows-mapper.md))
в стенд ещё не добавлены, поэтому Windows-стенд проверяет оболочку.
По умолчанию запускается `--suite smoke`: нативные тесты `lhc-core --lib`,
debug-сборка, `editor --smoke`,
`interactions`, CLI, второй экземпляр, по три цикла каждого попапа с ожиданием
метрик показа/скрытия, команды темы/языка и штатный выход. Конфиг изолирован
через `LHC_DEV_DIR`. Затем QMP проверяет нативный клавиатурный ввод в
`windows-receiver.ps1` (текст, Shift, Ctrl+Home, Shift+End, down/up). `--suite full` запускает прежний набор: нативные тесты,
release-сборку, `interactions`, IPC lifecycle, 100 циклов попапов, отключение
хоткеев и освобождение ресурсов (`check-windows-automated.ps1`).

Подготовка (один раз) поверх VM из [windows.md](../testing/windows.md) с
установленными MSVC Build Tools и Rust:

1. Создайте диск кэша сборки — он переживает прогоны, поэтому пересборка инкрементальная:
   `qemu-img create -f qcow2 /mnt/disk2/vm/left-hand-control-windows/cache.qcow2 64G`.
2. Создайте `vm.json`:
   ```json
   {
     "name": "windows11",
     "base": "windows11-lhc.qcow2",
     "user": "user",
     "ssh_key": "~/.ssh/lhc-vm",
     "ssh_port": 2241,
     "disk_bus": "ahci",
     "nic": "e1000e",
     "cpu": "host,hv_relaxed,hv_vapic,hv_spinlocks=0x1fff,hv_time",
     "memory": 6144,
     "cpus": 4,
     "cache_disk": "cache.qcow2",
     "uefi": {
       "code": "/usr/share/edk2/x64/OVMF_CODE.secboot.4m.fd",
       "vars": "OVMF_VARS.secboot.4m.fd",
       "tpm": "tpm"
     }
   }
   ```
3. Запустите VM на базовом диске с подключённым `cache.qcow2` (команда из
   windows.md плюс `-drive file=$vm_dir/cache.qcow2,if=none,id=cache,format=qcow2
   -device ide-hd,drive=cache,bus=ahci.1` вместо ISO). В PowerShell от
   администратора выполните:
   ```powershell
   .\prepare-windows-guest.ps1 -User user -Password '<пароль user>' -PublicKey '<содержимое ~/.ssh/lhc-vm.pub>' -CacheDiskNumber 1
   ```
   Номер пустого диска проверьте через `Get-Disk`: скрипт форматирует только явно
   указанный RAW-диск, который не является системным. Если Windows Update
   не загружает OpenSSH, можно явно передать `-OpenSshArchive <OpenSSH-Win64.zip>`
   из официального Win32-OpenSSH; скрипт проверит подпись Microsoft перед установкой. При уже настроенном
   автологоне `-Password` можно опустить.
   Скрипт включает OpenSSH Server с PowerShell по умолчанию, ключ, автологон,
   отключает сон, размечает диск кэша как `D:` и регистрирует интерактивную задачу
   планировщика `LHC-E2E`. Перезагрузите, проверьте автологон, выключите VM.

Прогон:

```sh
apps/slint-shell/scripts/vm/run-windows.py --vm /mnt/disk2/vm/left-hand-control-windows
```

Раннер упаковывает рабочее дерево (`git ls-files` вместе с неотслеживаемыми, но не
игнорируемыми файлами — незакоммиченные правки попадают в прогон), копирует его
в `D:\lhc-e2e\src` (каталог `target\` и Cargo cache на D: сохраняются), запускает задачу `LHC-E2E` в
сессии пользователя и ждёт `D:\lhc-e2e\out\done.txt` с кодом завершения (0 — успех). Первая сборка в VM занимает
десятки минут (`--timeout` в минутах, по умолчанию 150). Логи гостя копируются в
`runs/<время>/logs/guest/`. Трей, DPI, цветные emoji и вставка после хоткея
проверяются вручную (раздел 14 чек-листа).

### L5: ручная приёмка

[manual-acceptance.md](../testing/manual-acceptance.md): smoke-набор для каждой
среды, полная матрица mapper, `text:`, раскладок, действий, команд, жизненного
цикла, игрового режима и оболочки Windows. После L3 на установленном архиве
пройдите хотя бы smoke-набор (раздел 3 чек-листа) и допишите результат в файл L3.

## 5. Выпуск

### Каналы

Flathub и Snap не подходят: из песочницы нельзя поставить udev-правило и дать
доступ к `/dev/uinput` (Flathub не примет `--device=all`, strict Snap закрыт,
classic требует ручного одобрения).

| Канал | Состояние |
| --- | --- |
| GitHub Releases (tar.gz, deb, rpm, vendor, `SHA256SUMS`, attestation) | Workflow готов, первый тег ещё не ставили |
| Fedora COPR | Следующий шаг: SRPM из vendor-архива |
| Свой APT/RPM-репозиторий (GitHub Pages или Cloudsmith) или openSUSE Build Service | Когда появятся пользователи Debian/Ubuntu/openSUSE |
| AUR (`left-hand-control`, `left-hand-control-bin`) | Когда откроют регистрацию |
| nixpkgs | Когда стабилизируется формат |
| AppImage | Отложен: автозапуск должен брать `$APPIMAGE` вместо `current_exe()`, а права на устройства нужно выдавать отдельно |
| Ubuntu PPA | Не планируется (сборка без сети, старый rustc) |

ARM (aarch64) и Windows — следующими этапами, на нативных раннерах.

### Артефакты (Linux x86_64)

```
left-hand-control-<ver>-linux-x86_64.tar.gz   # ставится в ~/.local через install-linux.sh
left-hand-control_<ver>_amd64.deb             # Debian 12+, Ubuntu 22.04+
left-hand-control-<ver>-1.x86_64.rpm          # Fedora, openSUSE
left-hand-control-<ver>-vendor.tar.xz         # cargo vendor: офлайн-сборка и исходники для GPL
SHA256SUMS
```

Бинарник собирается в контейнере **Ubuntu 22.04 (glibc 2.35)**, поэтому подходит
всем актуальным дистрибутивам с Plasma 6. deb и rpm собирает nfpm из
`apps/slint-shell/packaging/linux/nfpm.yaml` и того же бинарника. Общие файлы
упаковки (`.desktop`, metainfo, udev-правило, `modules-load.conf`, иконки) лежат в
`apps/slint-shell/packaging/linux/`; PNG-иконки генерирует
`apps/slint-shell/scripts/render-icons.sh` из `assets/icon.svg`.

Пакеты ставят udev-правило и автозагрузку `uinput`, перезагружают правила udev,
но **не** добавляют пользователя в группу `input`; удаление не трогает конфиг
пользователя. Архив ставит то же явной командой `install-linux.sh permissions`.

### Локальная сборка

```sh
apps/slint-shell/scripts/package-linux.sh   # только tar.gz для этой машины → <target>/packages/
apps/slint-shell/scripts/release-linux.sh   # все артефакты → <target>/release-artifacts/ (нужен nfpm)
```

Переносимые артефакты собирайте в `ubuntu:22.04`, как в workflow:

```sh
docker run --rm -it -v "$PWD":/src -w /src ubuntu:22.04 bash
# внутри: зависимости из раздела 1, rustup stable, nfpm 2.47.0, затем
apps/slint-shell/scripts/release-linux.sh
```

Smoke пакета в чистом контейнере (то же, что `release.yml`):

```sh
docker run --rm -v "$PWD":/src -w /src fedora:latest \
  apps/slint-shell/scripts/smoke-linux-package.sh target/release-artifacts/<файл>.rpm
```

Образы матрицы: `debian:12`, `debian:13`, `ubuntu:22.04`, `ubuntu:24.04`,
`fedora:latest`, `opensuse/tumbleweed`, `archlinux` (tar.gz).

### Workflow релиза

`.github/workflows/release.yml` запускается по тегу `v<версия>`; ручной запуск
собирает и проверяет артефакты без создания релиза.

1. **verify** — вызывает `ci.yml`: fmt, clippy, тесты, сборка без Spell, L1 (если на
   раннере есть `uinput`).
2. **metadata** (`release-metadata.py`) — тег совпадает с `workspace.package.version`,
   в `CHANGELOG.md` есть секция `## [<версия>] - <дата>`, первый `<release>` в metainfo — эта версия.
3. **build-linux** (`ubuntu:22.04`) — `release-linux.sh`, проверка `ldd`, lintian, `SHA256SUMS`.
4. **rpmlint** (`fedora:latest`) и **smoke-packages** — установка и удаление в 7 контейнерах.
5. **draft-release** — attestation сборки и **черновик** релиза с заметками из `CHANGELOG.md`.
   Публикует человек после локальных e2e.

Права: по умолчанию `contents: read`; `contents: write`, `id-token: write` и
`attestations: write` только у job публикации.

### Чек-лист выпуска

1. Перенести `Unreleased` в `CHANGELOG.md` в секцию версии с датой.
2. Поднять `workspace.package.version` в `Cargo.toml`, выполнить `cargo update -w`.
3. Добавить `<release version="…" date="…">` в
   `apps/slint-shell/packaging/linux/dev.bozonx.left-hand-control.metainfo.xml`.
4. Прогнать L3 (`e2e-local.sh`) на текущем `main`.
5. Собрать артефакты (`release-linux.sh` в `ubuntu:22.04`) и прогнать L4: Linux VM
   хотя бы для Fedora KDE (rpm) и Debian 13 KDE (deb); Windows VM — при изменениях
   переносимой оболочки.
6. Поставить тег `vX.Y.Z`, дождаться черновика релиза.
7. Скачать артефакты черновика, проверить `sha256sum -c SHA256SUMS`, при желании
   прогнать `run-linux.py` на скачанном пакете.
8. Опубликовать релиз; затем COPR и другие подключённые каналы.

### Открытые задачи

- Первый запуск `release.yml` на GitHub (сначала тегом в форке) и проверка L1 в CI.
- Подготовка базовых образов Fedora KDE и Debian 13 KDE и первый полный прогон L4;
  затем Kubuntu, openSUSE, Arch.
- Подготовка Windows VM к `run-windows.py` (`prepare-windows-guest.ps1`).
- Выдача прав из приложения через `pkexec` и polkit-политику — для пользователей
  deb/rpm, у которых нет `install-linux.sh permissions`.
- COPR, затем свой репозиторий или OBS; ARM и Windows-сборки.

## 6. Частые проблемы

- **Второй запуск открывает настройки, а не новый экземпляр** — сокет занят:
  `target/debug/left-hand-control quit` или другой `LHC_SOCKET`. Устаревший сокет
  после аварии удаляется при следующем запуске.
- **Первая сборка очень долгая или падает на Skia** — проверьте clang, cmake, ninja, Python.
- **`No readable Ctrl+Alt+F11/F12 evdev devices`** — нет доступа к `/dev/input`:
  права из раздела 1 или `LHC_HOTKEYS=off`.
- **Emoji монохромные или квадратами** — Noto Color Emoji и `SLINT_BACKEND=winit-skia`.
- **`compositor does not advertise zwlr_layer_shell_v1`** — нет layer-shell (GNOME):
  `LHC_POPUPS=auto` или `winit`.
- **Паника `RefCell already borrowed` на странице** — жив `Document::read()`, пока
  callback редактирует; отпустите borrow до вызова UI.
- **VM-стенд: `virtio-vga is not a valid device model name`** — установите модуль
  QEMU `virtio-vga` или уберите `vga` из `vm.json`.
- **VM-стенд: `keyboard-probe` не находит клавиатуру** — в госте нет драйвера USB HID
  или QEMU отправляет клавиши в PS/2; проверьте `qemu.log` и `/proc/bus/input/devices` в госте.
