# Slint Shell: принятое UI и результаты прототипа

Принятая оболочка будущего приложения; Tauri продолжает работать параллельно.
Обе оболочки входят в Cargo workspace и используют общий `lhc-core`. Slint 1.17.1 /
winit 0.30, ksni (D-Bus StatusNotifierItem без GTK), evdev. Редактор настроек
содержит демонстрационный каталог действий и Quick/Emoji. Простые назначения
клавиш загружаются из `config.json` и сохраняются с проверкой через `lhc-core`;
условные и послойные правила этот редактор не изменяет. Можно выбрать клавиатуру
и запустить Linux mapper через общий runtime. Если Tauri изменил конфиг во время
работы Slint, сохранение отклоняется до повторного запуска Slint.

Общий Slint UI также компилируется для Windows и macOS. Linux-native код ограничен
target dependencies и `cfg`; переносимый слой использует `global-hotkey`,
`tray-icon` и `enigo`. Это минимальные адаптеры для платформенной проверки, а не
заявление о принятой поддержке Windows/macOS.

## Запуск

Из корня репозитория:

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml
SLINT_BACKEND=winit-software SLINT_SHELL_METRICS=/tmp/slint-software.csv target/debug/slint-shell
```

По очереди замените backend на `winit-femtovg` и `winit-skia`.
Одновременно запускается только один экземпляр на `XDG_RUNTIME_DIR`.
Окна изначально скрыты. Процесс остаётся в трее до команды «Выход».
Индикатор mapper в трее отражает реальный статус. Выбор Quick пока остаётся
демонстрационным; реальный каталог и выполнение действий ещё не подключены.
Для release используйте `cargo build --release --locked --manifest-path prototypes/slint-shell/Cargo.toml`.

На Windows IPC слушает только `127.0.0.1:43176`; порт можно изменить через
`SLINT_SHELL_PORT`. macOS использует Unix socket в `XDG_RUNTIME_DIR` или системной
временной директории. Ctrl+Alt+F11 открывает Emoji, Ctrl+Alt+F12 — Quick. После выбора на
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
target/debug/slint-shell show settings
target/debug/slint-shell show emoji
target/debug/slint-shell show quick
target/debug/slint-shell toggle-mapper
target/debug/slint-shell hide
target/debug/slint-shell quit
```

- Трей: клик переключает настройки; меню открывает окна, меняет цвет иконки
  (зелёный/серый) и завершает процесс.
- Emoji: 240 символов, 5 страниц по 48, стрелки, Enter, Esc, цифры 1–5.
  Кнопка Stress: 1500 ячеек, прокрутка и навигация.
- Quick: 30 заглушек, поиск (включая кириллицу), стрелки, Enter, Esc.
- Потеря фокуса скрывает попап.
- Ctrl+Alt+F11 открывает Emoji, Ctrl+Alt+F12 — Quick. Слушаются доступные
  evdev-устройства, поддерживающие эти клавиши. Можно задать
  `SLINT_SHELL_INPUT=/dev/input/eventN`. Устройство не захватывается.
  Права на input должны быть настроены заранее. Переподключение устройств
  требует перезапуска прототипа.

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

## Проверка этапа 3

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
prototypes/slint-shell/scripts/bench-open.sh emoji 20
python3 prototypes/slint-shell/scripts/summarize.py /tmp/slint-software.csv
```

Скрипт проверяет только IPC без activation token при обычном запуске из shell.
Он не синтезирует ввод и не доказывает готовность к вводу: t5 и navigation_handled
появятся только после реальной клавиши. Пропущенные события не заменяются нулями.
CSV перезаписывается при старте; используйте отдельные пути для разных запусков.

Для реальной вставки выбранного emoji или текста в KDE Wayland запустите Spell с
`SLINT_SHELL_INSERT=1`. Прототип запоминает исходное окно через KWin, после выбора
скрывает слой, ждёт отпускания клавиатуры, возвращает фокус и вставляет значение
через `wl-copy` + uinput Ctrl+V. Нужны `wl-copy`, доступ к `/dev/uinput` и KWin.
Исходный clipboard восстанавливается после вставки. Без переменной выбор только
пишется в лог. `SLINT_SHELL_TEST_INJECT=1` сохраняет диагностическую вставку `A`.

Проверка поведения при аварийном завершении Spell worker:

```sh
python3 prototypes/slint-shell/scripts/check-worker-crash.py \
  target/debug/slint-shell
```

Стенд завершает только дочерний процесс, ждёт новый worker, вызывает попап и
проверяет, что родитель остаётся доступен, а ошибка записана в лог. Во время восстановления ошибка
показывается в настройках.
Родитель автоматически перезапускает worker не чаще трёх раз за минуту и
восстанавливает тему и язык. После исчерпания лимита ошибка остаётся в настройках.

## Автопроверка evdev

Остановите прототип. При уже настроенных правах на uinput/input:

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --examples
SLINT_BACKEND=winit-software target/debug/examples/bench-evdev \
  target/debug/slint-shell /tmp/slint-evdev.csv
python3 prototypes/slint-shell/scripts/summarize.py /tmp/slint-evdev.csv
```

Пример создаёт временную виртуальную клавиатуру и запускает отдельный сервер,
слушающий только её. Он действительно нажимает Ctrl+Alt+F11 / Ctrl+Alt+F12 в сессии.
Первая стрелка отправляется после нового t4, если окно ещё не скрыто (опрос 5 мс,
таймаут 2 с). t5 включает задержку опроса и эмуляции. 20 вызовов каждого попапа; завершение через IPC.
При аварии тест останавливает свой дочерний процесс; может остаться stale socket
в `XDG_RUNTIME_DIR`, который сервер удалит при следующем обычном запуске.

## Этап 3а: Spell / layer-shell

Прототип теперь фиксирует **Slint 1.17.1** и Spell **1.0.6**, Git revision
`c3139561519cbd78fc5ac8721e1b5a6eb6412f42`. Spell из этой ревизии не собирается
со Slint 1.18 (`i_slint_core::items::MouseCursor` удалён из прежнего места).
Старые CSV этапов 0–3 остаются результатами Slint 1.18, сравнивайте версии явно.

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --features spell --examples --bin slint-shell
SLINT_SHELL_POPUPS=auto SLINT_BACKEND=winit-software \
  SLINT_SHELL_METRICS=/tmp/slint-stage3a.csv \
  target/debug/slint-shell
```

`SLINT_SHELL_POPUPS`:

- `winit` — прежний режим, по умолчанию; сборка без feature `spell` сохраняется.
- `auto` — проверяет registry Wayland: при наличии `zwlr_layer_shell_v1` запускает
  Spell; без протокола использует обычные окна winit (в частности, GNOME Wayland).
- `spell` — требует layer-shell, иначе завершает запуск с ошибкой. Ошибка Spell
  не маскируется автоматическим переходом на winit.

Настройки и трей работают в родительском процессе winit. Оба попапа живут в одном
дочернем процессе Spell; backend Slint внутри процесса не переключается.
Кнопка, трей, evdev и внешний IPC передают команды этому процессу. Источник и время
триггера сохраняются через IPC; activation token для layer-shell не используется.
При выходе родителя worker завершается; EOF управляющего stdin также завершает
worker при аварийном выходе родителя. `quit` и `ping` доступны через прежний CLI.
Ответ `queued` означает постановку команды в очередь, не готовность окна к вводу.

Spell использует **Skia software surface + Wayland SHM**, а не winit-skia GPU.
`SLINT_BACKEND=winit-*` выбирает только рендерер родителя. Других рендереров у
зафиксированного Spell backend нет. Суммарную память измеряйте вместе с потомками.

Параметры слоёв: Emoji 520×460, Quick 520×500, overlay, anchor bottom, margin bottom
24, exclusive zone 0. `SLINT_SHELL_OUTPUT=<имя>` передаёт выбранный монитор в Spell.
В приватном KWin проверены второй монитор, масштабы 1/1,25/1,5, отключение
выбранного выхода, панель и fullscreen. Для других композиторов проверка остаётся открытой.

Fallback GNOME использует те же окна без рамки и размеры. Центрирование и активация
определяются Mutter; код не утверждает, что они обеспечены. В этой сессии GNOME не
проверен, поэтому требование центрирования fallback **не принято**.

### Исправленный lifecycle (2026-09-20)

Используется локальная копия Spell из `vendor/spell-framework`; исходная ревизия,
лицензия и изменения перечислены в `vendor/spell-framework/PATCHES.md`.
По умолчанию `SLINT_SHELL_SPELL_LIFECYCLE=unmap`: скрытие немедленно коммитит пустой
буфер, повторный показ запускает кадр независимо от остановленного frame callback.
Число ожидающих frame callbacks ограничено одним; скрытое окно не делает поздних
коммитов. Закрытый композитором слой пересоздаётся, например при отключении монитора.
`transparent` сохранён только как диагностический режим.

Фокус передаётся напрямую через callback конкретного окна, без разбора логов.
Исправлены автоповтор, отпускание клавиш при потере фокуса и модификаторы,
зажатые до открытия. Размеры при смене масштаба вычисляются из логических размеров.
Клавиша `6` открывает тяжёлую страницу из 1500 эмодзи.

### Метрики и воспроизведение

Родитель пишет CSV по `SLINT_SHELL_METRICS`, worker — тот же путь с `.spell.csv`.
`t3_first_frame` означает отрисовку и commit буфера, **не presentation композитора**.
`t4_focused` — доставленный keyboard enter, `t5_first_key` — обработанная клавиша.
Evdev-стенд ждёт событие навигации до 500 мс; t5 включает опрос и эмуляцию.

Из корня репозитория:

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --features spell --examples --bin slint-shell
cargo test --locked --manifest-path prototypes/slint-shell/Cargo.toml --features spell
cargo check --locked --manifest-path prototypes/slint-shell/Cargo.toml --no-default-features
cargo clippy --locked --manifest-path prototypes/slint-shell/Cargo.toml --features spell --all-targets --no-deps -- -D warnings

# Изолированный KWin: не переключает фокус рабочего стола пользователя.
prototypes/slint-shell/scripts/bench-virtual.sh /tmp/slint-geometry geometry
prototypes/slint-shell/scripts/bench-virtual.sh /tmp/slint-input input
prototypes/slint-shell/scripts/bench-virtual.sh /tmp/slint-lifecycle lifecycle
python3 prototypes/slint-shell/scripts/summarize.py /tmp/slint-lifecycle/parent.csv.spell.csv

# Реальная KDE Wayland-сессия: на время теста не пользоваться клавиатурой.
SLINT_SHELL_POPUPS=spell SLINT_BACKEND=winit-software \
  target/debug/examples/bench-evdev \
  target/debug/slint-shell /tmp/slint-evdev.csv
SLINT_BACKEND=winit-software \
  target/debug/examples/bench-return \
  target/debug/slint-shell /tmp/slint-return.csv
```

Виртуальные проверки требуют `kwin_wayland`, `kscreen-doctor`, `dbus-run-session`
и Python 3. Они создают отдельные D-Bus, runtime/config/cache/data-каталоги и два
выхода 1920×1080. Только режим `input` включает `KWIN_WAYLAND_NO_PERMISSION_CHECKS=1`
в приватном композиторе для тестового fake-input. Рабочая сессия эту настройку не
получает. Открытие в этом режиме идёт через IPC, клавиатура — через fake-input;
это не заменяет проверку evdev на реальном рабочем столе.

`bench-return` создаёт собственное окно-получатель и включает только для тестового
процесса `SLINT_SHELL_TEST_INJECT=1`. KDE-проба запоминает исходное окно, после выбора
ждёт освобождения клавиатуры, восстанавливает и подтверждает фокус, затем отправляет
одну клавишу A (а не выбранный эмодзи). В реальной сессии ждёт также отпускания
физических модификаторов. Esc ничего не вводит. Это проверка возврата ввода, а не
готовая реализация вставки в продукте; без переменной инжект отключён.

Итог: 500 циклов плюс два прогрева получили кадр и фокус каждый; 16 проверок
геометрии и расширенная серия ввода прошли в приватном KWin. Тяжёлая страница
задерживает ввод примерно на 2,6 с в debug. Полная матрица приложений/способов
вызова, GNOME fallback и Hyprland ещё требуют приёмки. Подробности и исходные
CSV — в [отчёте](../../dev_docs/slint-prototype-report.md#результаты-доработки-3а-2026-09-20).
