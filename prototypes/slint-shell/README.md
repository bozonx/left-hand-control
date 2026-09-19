# Slint Shell: этапы 0–3 и проверка 3а

Изолированный Linux-прототип, не входит в сборку Tauri. Slint 1.17.1 / winit 0.30,
ksni (D-Bus StatusNotifierItem без GTK), evdev. Настройки — только пульт вызова
попапов; действия и выбор эмодзи пишутся в лог.

## Запуск

Из корня репозитория:

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml
SLINT_BACKEND=winit-software SLINT_SHELL_METRICS=/tmp/slint-software.csv prototypes/slint-shell/target/debug/slint-shell
```

По очереди замените backend на `winit-femtovg` и `winit-skia`.
Одновременно запускается только один экземпляр на `XDG_RUNTIME_DIR`.
Окна изначально скрыты. Процесс остаётся в трее до команды «Выход».
Для release используйте `cargo build --release --locked --manifest-path prototypes/slint-shell/Cargo.toml`.

Нужны рабочая графическая сессия, session D-Bus, Noto Color Emoji, системные
библиотеки Wayland/X11/fontconfig/GL и зависимости сборки Skia (clang, cmake,
ninja, Python). Возможность сборки не означает наличие цветных эмодзи во всех
рендерерах — это отдельная визуальная проверка.

## Управление

```sh
prototypes/slint-shell/target/debug/slint-shell show settings
prototypes/slint-shell/target/debug/slint-shell show emoji
prototypes/slint-shell/target/debug/slint-shell show quick
prototypes/slint-shell/target/debug/slint-shell toggle-mapper
prototypes/slint-shell/target/debug/slint-shell hide
prototypes/slint-shell/target/debug/slint-shell quit
```

- Трей: клик переключает настройки; меню открывает окна, меняет цвет иконки
  (зелёный/серый) и завершает процесс.
- Emoji: 240 символов, 5 страниц по 48, стрелки, Enter, Esc, цифры 1–5.
  Кнопка Stress: 1500 ячеек, прокрутка и навигация.
- Quick: 30 заглушек, поиск (включая кириллицу), стрелки, Enter, Esc.
- Потеря фокуса скрывает попап.
- F13 открывает Emoji, ScrollLock — Quick. Слушаются доступные evdev-устройства,
  поддерживающие эти клавиши. Можно задать `SLINT_SHELL_INPUT=/dev/input/eventN`.
  Устройство не захватывается: ScrollLock также получает исходное приложение.
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

Этапы 4–7 (позиционирование, возврат фокуса/инжект, полный UI настроек,
сравнительные замеры) здесь не реализованы.

## Автопроверка evdev

Остановите прототип. При уже настроенных правах на uinput/input:

```sh
cargo build --locked --manifest-path prototypes/slint-shell/Cargo.toml --examples
SLINT_BACKEND=winit-software prototypes/slint-shell/target/debug/examples/bench-evdev \
  prototypes/slint-shell/target/debug/slint-shell /tmp/slint-evdev.csv
python3 prototypes/slint-shell/scripts/summarize.py /tmp/slint-evdev.csv
```

Пример создаёт временную виртуальную клавиатуру и запускает отдельный сервер,
слушающий только её. Он действительно нажимает F13 / ScrollLock в сессии:
проводите проверку, когда эти клавиши не привязаны к другим действиям.
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
  prototypes/slint-shell/target/debug/slint-shell
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
Не считайте сам факт передачи имени подтверждением корректного выбора: результаты
и ограничения приведены в отчёте. Margin относительно work area, fullscreen,
второй монитор и fractional scaling требуют проверки соответствующего композитора.

Fallback GNOME использует те же окна без рамки и размеры. Центрирование и активация
определяются Mutter; код не утверждает, что они обеспечены. В этой сессии GNOME не
проверен, поэтому требование центрирования fallback **не принято**.

### Два режима жизненного цикла

`SLINT_SHELL_SPELL_LIFECYCLE=unmap` использует исходные `SpellWin::hide/show_again`.
На KDE показ предварительно скрытых поверхностей не привёл к кадру/фокусу: 0/20
для каждого попапа. Режим сохранён для воспроизведения upstream-проблемы.

По умолчанию используется экспериментальный `transparent`: слой остаётся mapped,
содержимое скрывается, область мышиного ввода очищается, keyboard interactivity
переключается в None. При показе возвращаются содержимое, область ввода и Exclusive.
Это обход unmapped lifecycle, с постоянно существующими прозрачными поверхностями
и дополнительной работой event loop. **Стабильность 20/20 также не достигнута.**

`SLINT_SHELL_SPELL_INITIAL=emoji` или `quick` — диагностический показ при старте,
обходящий начальное скрытие. Для измерения обычного запуска переменную не задавайте.

### Метрики и воспроизведение

Родитель пишет `/tmp/slint-stage3a.csv`, worker — `/tmp/slint-stage3a.csv.spell.csv`.
В worker t4 берётся из событий enter/leave клавиатуры, которые логирует закреплённая
версия Spell. Прототип пересылает их в Slint как WindowActiveChanged и фиксирует
доставку. Winit accessor не используется. Этот адаптер зависит от формата сообщений
данной ревизии Spell. t5/navigation_handled возникают только при обработке клавиши.
`t2_shown` — отправка запроса показа. Rendering notifier подключён, но в измерениях
Spell не выдавал AfterRendering: отсутствие t3 не заменяется значением t2.

```sh
# Остановите предыдущий экземпляр перед evdev-тестом.
SLINT_SHELL_POPUPS=spell SLINT_BACKEND=winit-software \
  prototypes/slint-shell/target/debug/examples/bench-evdev \
  prototypes/slint-shell/target/debug/slint-shell /tmp/spell-evdev.csv
python3 prototypes/slint-shell/scripts/summarize.py /tmp/spell-evdev.csv.spell.csv

# Исходный lifecycle Spell, отдельная серия:
SLINT_SHELL_SPELL_LIFECYCLE=unmap SLINT_SHELL_POPUPS=spell SLINT_BACKEND=winit-software \
  prototypes/slint-shell/target/debug/examples/bench-evdev \
  prototypes/slint-shell/target/debug/slint-shell /tmp/spell-unmap.csv

# 500 чередующихся циклов, отдельный экземпляр и сокет, результаты + дерево памяти:
prototypes/slint-shell/scripts/bench-lifecycle.py /tmp/spell-500 500
python3 prototypes/slint-shell/scripts/summarize.py /tmp/spell-500/parent.csv.spell.csv

# Для уже работающего экземпляра:
prototypes/slint-shell/scripts/mem.sh <PID-родителя> idle
```

Evdev-тест теперь ждёт новый trial и t4 с опросом каждые 5 мс, максимум 2 с, затем
сразу посылает стрелку. При отсутствующем фокусе или уже скрытом окне стрелка не
посылается. Потеря фокуса между проверкой и синтетической клавишей всё ещё возможна;
проверяйте на тестовом исходном окне. Любой неуспешный вызов даёт ненулевой exit code.
Метрики t5 включают задержку опроса и эмуляции. Старые серии с ожиданием 400 мс
не эквивалентны новому тесту.

Lifecycle-скрипт доказывает прохождение команд, но сам не доказывает отображение
каждого кадра, ввод или отсутствие утечек в полном рабочем сценарии. Проверяйте
число t4/t5/t3 перед интерпретацией памяти. Оба теста меняют фокус в текущей сессии;
не запускайте их одновременно.

Выбор по Enter остаётся заглушкой. Возврат фокуса с инжектом в исходное приложение,
автоповтор, модификаторы при уже зажатой клавише, полный набор DE/мониторов и
визуальная приёмка пока не приняты; этап 3а целиком не закрыт.
