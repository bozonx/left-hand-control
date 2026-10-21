# Linux: KDE Plasma 6, Wayland

Это основное принятое окружение продукта. Локальный рабочий стол, VM и физические
устройства проверяются разными наборами; один результат не заменяет остальные.
X11 в приёмку не входит.

## Проверяемые системы

| Система | Пакет проверенного прогона | Исторический отчёт (PASS/FAIL) |
| --- | --- | --- |
| [Fedora 44 KDE](fedora-kde.md) | rpm | [2026-10-17: 39/2](results/linux/2026-10-17-vm-fedora44-kde-4.txt) |
| [Debian 13 KDE](debian-kde.md) | deb | [2026-10-18: 39/2](results/linux/2026-10-18-vm-debian13-kde-4.txt) |
| [Kubuntu 26.04](kubuntu-kde.md) | tar.gz | [2026-10-19: 39/2](results/linux/2026-10-19-vm-kubuntu2604-3.txt) |

Эти VM-прогоны не полностью зелёные: в каждом записаны сбои Emoji hotkey
(Ctrl+Alt+F11 переключает VT) и восстановления mapper после USB-replug
(в Debian шаг завершился SSH-тайм-аутом). Они не подтверждают закрытие этих
двух областей; для текущего HEAD нужен повторный прогон.

Это свидетельства для записанных коммитов/пакетов. Тип пакета нельзя подменять:
прогон tar.gz не подтверждает установку deb на том же дистрибутиве.
Arch KDE и openSUSE KDE перечислены как будущие VM в руководстве разработчика,
но desktop-отчётов для них в текущей матрице нет.

## Предварительные условия

- Rust и системные зависимости — [руководство разработчика](../development/guide.md).
- Для локального desktop-набора — действующая KDE Wayland-сессия, трей и поддержка
  layer-shell для Spell; для mapper e2e — доступ к `/dev/uinput` и `/dev/input`.
- Для VM — Linux-хост с KVM/QEMU, SSH/SCP, отдельный базовый образ с автологином,
  тестовым пользователем и `vm.json`. Подготовка образов и поля конфигурации —
  [VM-руководство](../development/guide.md#l4-e2e-в-виртуальных-машинах).
- Использовать временные тестовые профили. Прогон VM устанавливает/удаляет пакет,
  меняет права и выполняет перелогин только в одноразовом overlay.

## Локальные проверки

Сначала [общие Cargo-проверки](common.md), затем в графической сессии:

```sh
apps/slint-shell/scripts/e2e-local.sh
```

Набор собирает оболочку со Spell и выполняет editor smoke, interactions, library,
action-picker, macros, menus, undo-redo, inline-edit, drag-scroll, notifications;
проверяет восстановление, настройки, падение Spell worker, Linux install-скрипты
и headless mapper e2e. `--no-mapper` явно исключает последний набор.
Полные логи сохраняются во временный каталог, который выводится при запуске.

Для точечного UI-изменения:

```sh
cargo run --locked -p slint-shell --example editor -- --smoke
cargo run --locked -p slint-shell --example interactions
```

Эти два примера не заменяют весь desktop-набор и VM-проверку установленного пакета.
Не используйте клавиатуру для другой работы во время сценариев ввода.

## Пакет в VM

Сначала собрать совместимый release-пакет по [инструкции выпуска](../development/guide.md#5-выпуск).
Команды для подготовленных стендов (заменить пути и версию пакета):

```sh
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/fedora44-kde --package /path/to/left-hand-control-VERSION.x86_64.rpm
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/debian13-kde --package /path/to/left-hand-control_VERSION_amd64.deb
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/kubuntu2604 --package /path/to/left-hand-control-VERSION-linux-x86_64.tar.gz
```

Runner использует команды install/remove/reinstall из `vm.json`, а не угадывает
пакетный менеджер по имени каталога. `--show` показывает VM, `--keep` оставляет overlay.
Для обновления с предыдущего выпуска добавить `--previous /path/to/previous-package`;
без него переход между двумя версиями не проверяется.

## Что проверяет VM

Источник перечня — [run-linux.py](../../apps/slint-shell/scripts/vm/run-linux.py):

- Установка и файлы пакета, отсутствие grab без прав, выдача прав с перелогином,
  автозапуск и присутствие tray item.
- Клавиши QEMU через evdev/uinput: remap, tap/hold-слой, макрос, команда,
  системные действия, RU-раскладка, остановка/запуск и освобождение клавиш при stop/kill.
- Portal deny/consent, ASCII/Unicode `text:`, popup через IPC, правило Quick actions
  и Emoji hotkey, игровой режим.
- Блокировка, suspend/resume и USB-replug на возможностях конкретной VM.
- CLI, второй экземпляр, обновление поверх работающего приложения при наличии
  `reinstall`, завершение и удаление. Upgrade без настроенного шага явно пропускается.

Отчёт каждого запуска определяет, какие из этих шагов действительно прошли.
Наличие шага в runner не означает его успех на каждом госте.

## Что не покрыто полностью

VM не принимает физический USB/Bluetooth hotplug, несколько клавиатур, media keys,
драйверы, аппаратные задержки, реальный сон, несколько экранов и HiDPI.
Регистрация tray item не подтверждает внешний вид всех меню/иконок.
Проверки в тестовом редакторе не заменяют браузер, терминал и IDE.
Не каждый пункт [ручной матрицы](manual-acceptance.md) автоматизирован.
Отдельно фиксировать popup backend (`winit`, `auto`, `spell`), способ ввода текста
и решения portal: результаты одного режима не переносить на остальные.

## Результаты и диагностика

`e2e-local.sh` и VM runner сейчас пишут короткую сводку в `docs/testing/results/linux/`.
VM хранит логи/снимки в `<vm>/runs/<timestamp>/logs`; overlay не заменяет отчёт.
Правила отбора файлов для Git, описание неуспешных шагов и шаблон отчёта —
[хранение результатов](results/README.md).
