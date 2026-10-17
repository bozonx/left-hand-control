# Установка на Linux

Left Hand Control рассчитан на KDE Plasma 6 в сессии Wayland. Релизы публикуются
на странице GitHub Releases: архив `.tar.gz` для установки в домашний каталог,
пакеты `.deb` и `.rpm` и файл `SHA256SUMS`. Как работают функции приложения,
описано в `features.md` (в репозитории — [docs/features.md](features.md)).

## Установка архива

Проверьте контрольную сумму командой `sha256sum -c --ignore-missing SHA256SUMS`, распакуйте архив, перейдите
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
`sudo gpasswd -d "$USER" input`. Для ввода текста (`text:`) нужен `xdg-desktop-portal` с бэкендом окружения (на KDE — `xdg-desktop-portal-kde`); при первом вводе система один раз спросит разрешение на удалённое управление.

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
