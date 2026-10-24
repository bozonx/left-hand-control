# Slint Shell

Основная desktop-оболочка Left Hand Control на Slint и `lhc-core`.
Оболочка входит в Cargo workspace вместе с `lhc-core` и явно включённым
vendored-пакетом `spell-framework`. Патч `i-slint-core` остаётся внешней для
workspace зависимостью. Slint 1.18.1 /
winit 0.30, ksni (D-Bus StatusNotifierItem без GTK), evdev. Редактор настроек
работает с реальными назначениями клавиш, слоями, правилами, макросами,
командами и Quick/Emoji. Библиотека поддерживает раздельные редактирование и
активацию раскладок, ручной и автоматический режимы, приоритеты, условия,
переименование и описание. Автоматический режим обновляет mapper при смене
системного языка, активного окна и Game Mode; при отсутствии совпадения
используется passthrough. Конфигурация читается при запуске; после ручного изменения файла
нужно перезапустить приложение.

Повседневный запуск, зависимости, переменные окружения и проверки описаны в
[руководстве разработчика](../../docs/development/guide.md). Этот файл
содержит подробности стендов и измерений приложения.

Общий Slint UI также компилируется для Windows и macOS. Linux-native код ограничен
target dependencies и `cfg`; переносимый слой использует `global-hotkey`,
`tray-icon` и `enigo`. Это минимальные адаптеры для платформенной проверки, а не
заявление о принятой поддержке Windows/macOS.

## Запуск

Из корня репозитория:

```sh
cargo build --locked -p slint-shell
SLINT_BACKEND=winit-software LHC_METRICS=/tmp/slint-software.csv target/debug/left-hand-control
```

CSV метрик пишется только при заданном `LHC_METRICS`; Spell worker
пишет рядом файл с суффиксом `.spell.csv`.

По очереди замените backend на `winit-femtovg` и `winit-skia`.
Одновременно запускается только один экземпляр на `XDG_RUNTIME_DIR`.
Окна изначально скрыты. Процесс остаётся в трее до команды «Выход».
Индикатор mapper в трее отражает реальный статус. Quick и Emoji используют сохранённые данные; действия выполняются через mapper.
Для release используйте `cargo build --release --locked -p slint-shell`.

На Windows IPC слушает только `127.0.0.1:43176`; порт можно изменить через
`LHC_PORT`; каждый запрос подписан токеном. Без `LHC_DEV_DIR` он хранится в
`%LOCALAPPDATA%\lhc-slint-shell\ipc-token`; при override — в каталоге настроек
этого профиля. Linux и macOS используют Unix socket в приватном
каталоге `0700`: `$XDG_RUNTIME_DIR/lhc-slint-shell/` или, без `XDG_RUNTIME_DIR`,
`<temp>/lhc-slint-shell-<uid>/`. Ctrl+Alt+F11 открывает Emoji, Ctrl+Alt+F12 — Quick. После выбора на
Windows/macOS окно скрывается, ранее активное окно восстанавливается через Win32
или macOS System Events, затем текст отправляется через системный native input.
На macOS для этого требуется Accessibility permission. Фактический tray, возврат
фокуса и ввод должны быть проверены в нативной сессии до принятия платформы.

Нужны рабочая графическая сессия, session D-Bus, Noto Color Emoji, системные
библиотеки Wayland/X11/fontconfig/GL и зависимости сборки Skia (clang, cmake,
ninja, Python). Возможность сборки не означает наличие цветных эмодзи во всех
рендерерах — это отдельная визуальная проверка.

## Управление

```sh
target/debug/left-hand-control show settings
target/debug/left-hand-control show emoji
target/debug/left-hand-control show quick
target/debug/left-hand-control toggle-mapper
target/debug/left-hand-control game toggle   # auto|on|off|toggle
target/debug/left-hand-control hide
target/debug/left-hand-control quit
```

- Трей: клик переключает настройки; меню открывает окна, меняет цвет иконки
  (зелёный/серый) и завершает процесс.
- Emoji: страницы активной раскладки по 15 ячеек (Q W E R T · A S D F G · Z X C V B),
  стрелки, Enter, Esc, цифры переключают страницы.
- Quick: быстрые действия активной раскладки, поиск (включая кириллицу), стрелки,
  Enter, Esc, Alt+1…9.
- Выбранное действие выполняет mapper после того, как попап отдал фокус; при
  остановленном mapper показывается сообщение.
- Потеря фокуса скрывает попап.
- Ctrl+Alt+F11 открывает Emoji, Ctrl+Alt+F12 — Quick. Слушаются доступные
  evdev-устройства, поддерживающие эти клавиши. Можно задать
  `LHC_INPUT=/dev/input/eventN`. Устройство не захватывается.
  Права на input должны быть настроены заранее. Новые устройства, в том числе
  виртуальная клавиатура mapper, подхватываются автоматически в течение 2 с.

## Три пути вызова

1. Кнопка настроек запрашивает activation token у winit, затем показывает попап.
   При отсутствии поддержки/ответа используется показ без токена (таймаут 500 мс
   включён в метрику).
2. evdev передаёт событие на UI-поток через `invoke_from_event_loop`, без токена.
3. IPC-клиент передаёт `XDG_ACTIVATION_TOKEN` из своего окружения. Назначьте
   абсолютный путь к бинарнику с аргументами `show emoji` или `show quick`
   на глобальный шорткат DE. Для запуска через desktop entry задайте
   `StartupNotify=true`. Наличие токена зависит от способа запуска и DE;
   лог сервера явно показывает `activation_token=true/false`.

Токен применяется через Wayland `xdg_activation_v1.activate` к готовой
поверхности winit, без изменения окружения многопоточного процесса.
Подключение заимствует display у winit и живёт внутри UI-потока.
Начальная привязка протокола включает roundtrip и входит в метрику.
Hook атрибутов используется только для типа Utility на X11. На Wayland Slint уничтожает нативную
поверхность при hide и создаёт её при следующем show; сами три UI-компонента
живут с момента старта до выхода. На X11 сохранённое окно получает
`focus_window()`; токен создания уже существующего X11-окна не применяется.

Для обоих попапов запрошен always-on-top. На X11 задан тип Utility, но отсутствие
кнопки в панели зависит от WM. На Wayland такой гарантии нет. Для KDE можно
создать правило по точному заголовку `Slint Shell — Emoji` / `Slint Shell — Quick`,
включить «Поверх остальных окон» и «Пропускать панель задач». Результаты с правилом
нужно записывать отдельно от результатов без него. Тень не запрашивается;
фон прозрачный, внутренний прямоугольник со скруглениями.

## Проверка задержек

Для каждого backend / DE / исходного окна (терминал, браузер, IDE) выполните
20 вызовов каждым из трёх способов. После каждого вызова нажмите первую стрелку,
проверьте перемещение выделения и закройте Esc. Запишите отдельно: перекрытие
исходного окна, мигание панели, наличие кнопки панели, цвет эмодзи, трей.
Не кликайте по попапу перед первой стрелкой: это исказит проверку фокуса.

CSV содержит trial ID, окно, источник, событие, время от trigger и main.
Метки: t0 trigger, t1 UI callback, t2 show, t3 AfterRendering, t4 Focused(true),
t5 первая обработанная клавиша, navigation_handled, hidden. Для IPC t0 —
получение запроса сервером, не запуск клиентского процесса. Метка ready означает
создание UI-компонента, а не готовый к вводу нативный surface. Если рендерер
не поддерживает notifier, t3 отсутствует и в лог выводится предупреждение.

```sh
apps/slint-shell/scripts/bench-open.sh emoji 20
python3 apps/slint-shell/scripts/summarize.py /tmp/slint-software.csv
```

Скрипт проверяет только IPC без activation token при обычном запуске из shell.
Он не синтезирует ввод и не доказывает готовность к вводу: t5 и navigation_handled
появятся только после реальной клавиши. Пропущенные события не заменяются нулями.
CSV перезаписывается при старте; используйте отдельные пути для разных запусков.

Для диагностической вставки выбранного emoji или текста в KDE Wayland соберите
с `--features probes` и запустите Spell с `LHC_INSERT=1`. Прототип запоминает исходное окно через KWin, после выбора
скрывает слой, ждёт отпускания клавиатуры, возвращает фокус и вставляет значение
через Wayland data-control + uinput Shift+Insert с сохранением MIME-форматов. Нужны data-control, доступ к `/dev/uinput` и KWin.
Исходный clipboard восстанавливается после вставки. Без переменной выбор только
пишется в лог. `LHC_TEST_INJECT=1` сохраняет диагностическую вставку `A`.

Проверка поведения при аварийном завершении Spell worker:

```sh
python3 apps/slint-shell/scripts/check-worker-crash.py \
  target/debug/left-hand-control
```

Стенд завершает только дочерний процесс, ждёт новый worker, вызывает попап и
проверяет, что родитель остаётся доступен, а ошибка записана в лог. Во время восстановления ошибка
показывается в настройках.
Родитель автоматически перезапускает worker не чаще трёх раз за минуту и
восстанавливает тему и язык. После исчерпания лимита ошибка остаётся в настройках.

## Автопроверка evdev

Остановите приложение. При уже настроенных правах на uinput/input:

```sh
cargo build --locked -p slint-shell --features probes --examples
SLINT_BACKEND=winit-software target/debug/examples/bench-evdev \
  target/debug/left-hand-control /tmp/slint-evdev.csv
python3 apps/slint-shell/scripts/summarize.py /tmp/slint-evdev.csv
```

Пример создаёт временную виртуальную клавиатуру и запускает отдельный сервер,
слушающий только её. Он действительно нажимает Ctrl+Alt+F11 / Ctrl+Alt+F12 в сессии.
Первая стрелка отправляется после нового t4, если окно ещё не скрыто (опрос 5 мс,
таймаут 2 с). t5 включает задержку опроса и эмуляции. 20 вызовов каждого попапа; завершение через IPC.
При аварии тест останавливает свой дочерний процесс; может остаться stale socket
в `XDG_RUNTIME_DIR`, который сервер удалит при следующем обычном запуске.

## Spell / layer-shell

Приложение использует **Slint 1.18.1** и Spell **1.0.6**, Git revision
`b37604eb46edfec5861720915db87e646b645bd1` (первая ревизия с поддержкой Slint 1.18).

```sh
cargo build --locked -p slint-shell --features probes --examples --bin left-hand-control
LHC_POPUPS=auto SLINT_BACKEND=winit-software \
  LHC_METRICS=/tmp/slint-stage3a.csv \
  target/debug/left-hand-control
```

`LHC_POPUPS`:

- `winit` — режим по умолчанию; сборка без feature `spell` сохраняется.
- `auto` — проверяет registry Wayland: при наличии `zwlr_layer_shell_v1` запускает
  Spell; без протокола использует обычные окна winit (в частности, GNOME Wayland).
- `spell` — требует layer-shell, иначе завершает запуск с ошибкой. Ошибка Spell
  не маскируется автоматическим переходом на winit.

Настройки и трей работают в родительском процессе winit. Оба попапа живут в одном
дочернем процессе Spell; backend Slint внутри процесса не переключается.
Кнопка, трей, evdev и внешний IPC передают команды этому процессу. Источник и время
триггера сохраняются через IPC; activation token для layer-shell не используется.
При выходе родителя worker завершается; EOF управляющего stdin также завершает
worker при аварийном выходе родителя. `quit` и `ping` доступны через CLI.
Ответ `queued` означает постановку команды в очередь, не готовность окна к вводу.

Spell использует **Skia software surface + Wayland SHM**, а не winit-skia GPU.
`SLINT_BACKEND=winit-*` выбирает только рендерер родителя. Других рендереров у
зафиксированного Spell backend нет. Суммарную память измеряйте вместе с потомками.

Параметры слоёв: Emoji 520×460, Quick 520×500, overlay, anchor bottom, margin bottom
24, exclusive zone 0. `LHC_OUTPUT=<имя>` передаёт выбранный монитор в Spell.
В приватном KWin проверены второй монитор, масштабы 1/1,25/1,5, отключение
выбранного выхода, панель и fullscreen. Для других композиторов проверка остаётся открытой.

Fallback GNOME использует те же окна без рамки и размеры. Центрирование и активация
определяются Mutter; код не утверждает, что они обеспечены. В этой сессии GNOME не
проверен, поэтому требование центрирования fallback **не принято**.

### Исправленный lifecycle (2026-09-20)

Используется локальная копия Spell из `vendor/spell-framework`; исходная ревизия,
лицензия и изменения перечислены в `vendor/spell-framework/PATCHES.md`.
По умолчанию `LHC_SPELL_LIFECYCLE=unmap`: скрытие немедленно коммитит пустой
буфер, повторный показ запускает кадр независимо от остановленного frame callback.
Число ожидающих frame callbacks ограничено одним; скрытое окно не делает поздних
коммитов. Закрытый композитором слой пересоздаётся, например при отключении монитора.
`transparent` сохранён только как диагностический режим.

Фокус передаётся напрямую через callback конкретного окна, без разбора логов.
Исправлены автоповтор, отпускание клавиш при потере фокуса и модификаторы,
зажатые до открытия. Размеры при смене масштаба вычисляются из логических размеров.

### Метрики и воспроизведение

Родитель пишет CSV по `LHC_METRICS`, worker — тот же путь с `.spell.csv`.
`t3_first_frame` означает отрисовку и commit буфера, **не presentation композитора**.
`t4_focused` — доставленный keyboard enter, `t5_first_key` — обработанная клавиша.
Evdev-стенд ждёт событие навигации до 500 мс; t5 включает опрос и эмуляцию.

Из корня репозитория:

```sh
cargo build --locked -p slint-shell --features probes --examples --bin left-hand-control
cargo test --locked -p slint-shell --features spell
cargo check --locked -p slint-shell --no-default-features
cargo clippy --locked -p slint-shell --features probes --all-targets --no-deps -- -D warnings

# Изолированный KWin: не переключает фокус рабочего стола пользователя.
apps/slint-shell/scripts/bench-virtual.sh /tmp/slint-geometry geometry
apps/slint-shell/scripts/bench-virtual.sh /tmp/slint-input input
apps/slint-shell/scripts/bench-virtual.sh /tmp/slint-lifecycle lifecycle
python3 apps/slint-shell/scripts/summarize.py /tmp/slint-lifecycle/parent.csv.spell.csv

# Реальная KDE Wayland-сессия: на время теста не пользоваться клавиатурой.
LHC_POPUPS=spell SLINT_BACKEND=winit-software \
  target/debug/examples/bench-evdev \
  target/debug/left-hand-control /tmp/slint-evdev.csv
SLINT_BACKEND=winit-software \
  target/debug/examples/bench-return \
  target/debug/left-hand-control /tmp/slint-return.csv
```

Виртуальные проверки требуют `kwin_wayland`, `kscreen-doctor`, `dbus-run-session`
и Python 3. Они создают отдельные D-Bus, runtime/config/cache/data-каталоги и два
выхода 1920×1080. Только режим `input` включает `KWIN_WAYLAND_NO_PERMISSION_CHECKS=1`
в приватном композиторе для тестового fake-input. Рабочая сессия эту настройку не
получает. Открытие в этом режиме идёт через IPC, клавиатура — через fake-input;
это не заменяет проверку evdev на реальном рабочем столе.

`bench-return` создаёт собственное окно-получатель и включает только для тестового
процесса `LHC_TEST_INJECT=1`. KDE-проба запоминает исходное окно, после выбора
ждёт освобождения клавиатуры, восстанавливает и подтверждает фокус, затем отправляет
одну клавишу A (а не выбранный эмодзи). В реальной сессии ждёт также отпускания
физических модификаторов. Esc ничего не вводит. Это проверка возврата ввода, а не
готовая реализация вставки в продукте; без переменной инжект отключён.

Итог: 500 циклов плюс два прогрева получили кадр и фокус каждый; 16 проверок
геометрии и расширенная серия ввода прошли в приватном KWin. Тяжёлая страница
задерживает ввод примерно на 2,6 с в debug. Полная матрица приложений/способов
вызова, GNOME fallback и Hyprland ещё требуют приёмки. CSV для новых запусков
сохраняйте вне исходников, например в `/tmp/lhc-bench/`.

## Каталог сценариев и инструментов

Все команды выполняются из корня репозитория. UI-сценарии запускаются через
`cargo run --locked -p slint-shell --example <name>` и используют временный конфиг.

| Назначение | Примеры |
| --- | --- |
| Общая проверка UI | `editor -- --smoke`, `interactions` |
| Страницы и диалоги | `action-picker`, `library`, `macros`, `menus`, `conditions`, `layer-keys` |
| Общие контролы | `undo-redo`, `inline-edit`, `drag-scroll`, `notifications` |
| Измерение ввода и возврата фокуса | `bench-evdev` (feature `spell`), `bench-return` (feature `probes`) |
| Вспомогательные окна и диагностика | `window-fixture`, `panel-fixture` (feature `spell`), `focus-window`, `inspect-geometry` (feature `probes`) |

`conditions` дополнительно требует путь для снимка:
`SNAP=/tmp/lhc-conditions.ppm cargo run --locked -p slint-shell --example conditions`.

`bench-*` остаются исполняемыми примерами: они работают с реальными окнами и
устройствами и не используют стандартный Cargo benchmark harness.

| Скрипты | Назначение |
| --- | --- |
| `check-recovery.py` | Восстановление конфигурации, worker и Live Preview |
| `check-worker-crash.py`, `check-preferences.py` | Проверки worker и настроек |
| `check-windows-automated.ps1`, `check-windows-lifecycle.ps1`, `windows-receiver.ps1` | Windows-приёмка и окно получения ввода |
| `bench-open.sh`, `bench-virtual.sh`, `bench-geometry.py`, `bench-lifecycle.py` | Измерительные прогоны |
| `mem.sh`, `summarize.py` | Сбор памяти и сводка результатов |
| `e2e-local.sh` | Локальный прогон L3 со сводкой в `docs/testing/results/linux/` |
| `vm/run-linux.py`, `vm/run-windows.py` и гостевые скрипты `vm/` | E2E в одноразовых KVM/QEMU VM (L4), см. [руководство разработчика](../../docs/development/guide.md#l4-e2e-в-виртуальных-машинах) |

Общие экспериментальные XKB-инструменты описаны в [scripts/README.md](../../scripts/README.md).
