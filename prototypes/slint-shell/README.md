# Slint Shell: этапы 0–3

Изолированный Linux-прототип, не входит в сборку Tauri. Slint 1.18 / winit 0.30,
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
Первая стрелка отправляется через 400 мс только при наличии t4 и отсутствии hidden.
Поэтому t5 этого теста включает искусственную задержку и не измеряет минимальную
готовность к вводу. 20 вызовов каждого попапа; завершение через IPC.
При аварии тест останавливает свой дочерний процесс; может остаться stale socket
в `XDG_RUNTIME_DIR`, который сервер удалит при следующем обычном запуске.
