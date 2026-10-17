# Linux: выпуск, установка и удаление

Для сборки из репозитория нужны Rust ≥ 1.92 и системные зависимости из
[руководства разработчика](linux.md). Из корня репозитория:

```sh
apps/slint-shell/scripts/package-linux.sh
```

Скрипт использует `Cargo.lock`, release-профиль и feature `spell`. Архив и SHA-256
появляются в `<cargo-target>/packages/`. В архив входят бинарник `left-hand-control`,
иконки (PNG 16–512 и SVG), `.desktop`-файл, AppStream metainfo, udev-правило для
`/dev/uinput`, автозагрузка модуля `uinput`, лицензия, инсталлятор, эта инструкция
и руководство Linux. Исходники этих файлов лежат в `apps/slint-shell/packaging/linux/`
и `apps/slint-shell/assets/icon.svg`. Сборка предназначена для архитектуры и дистрибутива
сборочной машины: это не универсальный статический бинарник. Перед выпуском
проверьте его в целевой KDE Plasma 6 Wayland среде и проверьте `ldd left-hand-control`
на отсутствующие библиотеки. Одинаковый lockfile не гарантирует побайтовую
воспроизводимость при разных компиляторах и системных библиотеках.

Для проверки инсталлятора во временном каталоге (нужен `desktop-file-validate`
из пакета `desktop-file-utils`):

```sh
python3 apps/slint-shell/scripts/check-linux-install.py
```

## Установка архива

Проверьте SHA-256 командой `sha256sum -c имя-архива.tar.gz.sha256`, распакуйте архив, перейдите
в распакованный каталог и выполните:

```sh
./install-linux.sh
```

Установка выполняется без `sudo` в `~/.local`: бинарник получает стабильный путь
`~/.local/lib/dev.bozonx.left-hand-control/left-hand-control`, команда
`~/.local/bin/left-hand-control` указывает на него. Также устанавливаются иконки
в `share/icons/hicolor`, запись меню приложений и AppStream metainfo. Добавьте `~/.local/bin` в `PATH`, если его там нет.
Другой абсолютный префикс задаётся через `LHC_INSTALL_PREFIX`.

Запуск с поддержкой layer-shell и переходом на winit при его отсутствии:

```sh
LHC_POPUPS=auto ~/.local/bin/left-hand-control
```

При запуске из меню используется стандартный режим winit. Для цветных emoji
в winit задайте `SLINT_BACKEND=winit-skia`; нужны установленные Noto Color Emoji.
## Права на устройства ввода

Для mapper нужны права чтения `/dev/input/event*` и записи `/dev/uinput`.
Обычная установка их не меняет. Выдайте права один раз явной командой
(она запросит пароль `sudo`):

```sh
~/.local/lib/dev.bozonx.left-hand-control/install-linux.sh permissions
```

Команда устанавливает `/etc/udev/rules.d/60-left-hand-control.rules` (запись в
`/dev/uinput` для активной локальной сессии через `uaccess` и для группы `input`),
`/etc/modules-load.d/left-hand-control.conf` (загрузка `uinput` при старте),
загружает модуль и добавляет пользователя в группу `input`. После добавления
в группу перезайдите в сессию. Группа `input` даёт процессам пользователя чтение
всех клавиатур и мышей; выдавайте её осознанно. Отменить правило и автозагрузку
можно командой `install-linux.sh remove-permissions`; выход из группы:
`sudo gpasswd -d "$USER" input`. Portal и системные библиотеки описаны в руководстве Linux.

Включайте «Запускать вместе с системой» из установленного приложения, чтобы
автозапуск ссылался на стабильный путь. Если он раньше был включён из другого
каталога, выключите и включите его снова. Перед обновлением завершите приложение;
повторный запуск инсталлятора заменяет бинарник, сохраняя путь и настройки.

## Удаление

Выключите автозапуск в настройках и завершите приложение. Затем:

```sh
~/.local/lib/dev.bozonx.left-hand-control/install-linux.sh uninstall
```

При нестандартном префиксе передайте то же значение `LHC_INSTALL_PREFIX`.
Инсталлятор удаляет установленные файлы приложения; config и раскладки остаются.
Права на устройства не трогаются: если они больше не нужны, выполните
`install-linux.sh remove-permissions` до удаления.

## Пакеты deb и rpm

Ставятся штатным менеджером пакетов, `sudo` нужен только ему:

```sh
sudo apt install ./left-hand-control_<версия>_amd64.deb          # Debian 12+, Ubuntu 22.04+
sudo dnf install ./left-hand-control-<версия>-1.x86_64.rpm      # Fedora
sudo zypper install ./left-hand-control-<версия>-1.x86_64.rpm   # openSUSE
```

Пакет ставит бинарник в `/usr/bin`, запись меню, metainfo, иконки, udev-правило
в `/usr/lib/udev/rules.d/` и автозагрузку `uinput` в `/usr/lib/modules-load.d/`,
затем перезагружает правила udev. В группу `input` пакет пользователя **не**
добавляет: выполните `sudo usermod -aG input "$USER"` и перезайдите в сессию.
Удаление — `sudo apt remove left-hand-control` / `sudo dnf remove left-hand-control`;
конфигурация пользователя остаётся.

## Выпуск релиза

Релиз собирает `.github/workflows/release.yml` по тегу `v<версия>`; ручной запуск
собирает и проверяет артефакты без создания релиза. Workflow:

1. прогоняет CI (`ci.yml`);
2. проверяет, что тег совпадает с `workspace.package.version`, в `CHANGELOG.md` есть
   секция `## [<версия>] - <дата>`, а первый `<release>` в metainfo — эта версия
   (`apps/slint-shell/scripts/release-metadata.py`);
3. в контейнере Ubuntu 22.04 (glibc 2.35) выполняет
   `apps/slint-shell/scripts/release-linux.sh`: tar.gz, deb и rpm (nfpm,
   `packaging/linux/nfpm.yaml`), архив vendored-зависимостей и `SHA256SUMS`;
   проверяет deb через lintian, rpm через rpmlint;
4. ставит и удаляет пакеты в Debian 12/13, Ubuntu 22.04/24.04, Fedora, openSUSE
   Tumbleweed и Arch (`apps/slint-shell/scripts/smoke-linux-package.sh`);
5. добавляет attestation сборки и создаёт **черновик** релиза с заметками из
   `CHANGELOG.md`. Публикует его человек после локального e2e
   (`apps/slint-shell/scripts/e2e-local.sh`).

Локально те же артефакты собираются `release-linux.sh` (нужен `nfpm` в `PATH`);
для переносимого бинарника запускайте его в контейнере `ubuntu:22.04`.

Архив `left-hand-control-<версия>-vendor.tar.xz` содержит все crates из
`Cargo.lock` и `.cargo/config.toml` для офлайн-сборки: распакуйте его поверх
исходников версии (`tar -xJf … --strip-components=1`) и собирайте с `--offline`.
При распространении GPL-бинарника предоставляйте соответствующие исходники,
включая vendor-патчи, и инструкции сборки именно выпущенной версии; архив
исходников тега и vendor-архив вместе закрывают это требование.
