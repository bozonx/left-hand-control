# Разработка Slint-версии под Linux

Как собрать, запустить и проверить Slint-оболочку (`apps/slint-shell`) на Linux при разработке. Tauri-версия описана в корневом `README.md` и в `AGENTS.md`; обе оболочки собираются из одного Cargo workspace и используют общий крейт `crates/lhc-core`.

Основная среда разработки и проверки — KDE Plasma 6 на Wayland. На других окружениях многое работает, но не принято (см. «Что зависит от окружения»)

## Коротко

```sh
# один раз: системные пакеты (см. ниже), затем из корня репозитория
cargo run -p slint-shell                                   # обычные окна winit
SLINT_SHELL_POPUPS=auto cargo run -p slint-shell --features spell   # попапы как layer-shell
```

Окна при старте скрыты — приложение живёт в трее. Настройки открываются кликом по иконке трея или командой:

```sh
target/debug/slint-shell show settings
```

## Требования

Rust stable не ниже `1.85` (edition 2024; см. `rust-version` в корневом `Cargo.toml`).

Системные пакеты. Skia собирается из исходников при первой сборке, поэтому нужны clang, cmake, ninja и Python.

**Manjaro / Arch:**

```sh
sudo pacman -S --needed base-devel clang cmake ninja python pkgconf \
  fontconfig freetype2 libxkbcommon wayland dbus noto-fonts-emoji \
  wl-clipboard
```

**Debian / Ubuntu:**

```sh
sudo apt install build-essential clang cmake ninja-build python3 pkg-config \
  libfontconfig1-dev libfreetype-dev libxkbcommon-dev libwayland-dev \
  libdbus-1-dev fonts-noto-color-emoji wl-clipboard
```

Для отдельных функций дополнительно:

| Функция | Что нужно |
| --- | --- |
| Mapper (перехват клавиатуры) | Чтение `/dev/input/event*` (группа `input`) и запись в `/dev/uinput` (udev-правило `0660`, группа `input`) |
| Глобальные хоткеи Ctrl+Alt+F11 / F12 | Чтение `/dev/input/event*`; устройство не захватывается |
| Диагностический стенд возврата ввода (`--features probes`, `SLINT_SHELL_INSERT=1`) | KDE Wayland (KWin), `wl-clipboard`, запись в `/dev/uinput` |
| Условия правил по активному окну на KDE Wayland | `kdotool` |
| Ввод литерального текста mapper'ом | `xdg-desktop-portal` и бэкенд окружения (`xdg-desktop-portal-kde` и т. п.) |

Доступ к устройствам ввода (после изменения нужен перезаход в сессию):

```sh
sudo usermod -aG input "$USER"
echo 'KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"' \
  | sudo tee /etc/udev/rules.d/99-lhc-uinput.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Сборка и запуск

Все команды выполняются из корня репозитория. Артефакты лежат в общем `target/` workspace.

```sh
cargo build -p slint-shell                    # без Spell: попапы — обычные окна winit
cargo build -p slint-shell --features spell   # с поддержкой layer-shell попапов
cargo build -p slint-shell --release --features spell
```

Режим попапов выбирается при запуске переменной `SLINT_SHELL_POPUPS`:

| Значение | Поведение |
| --- | --- |
| `winit` (по умолчанию) | Попапы — окна без рамки в том же процессе |
| `auto` | Если композитор поддерживает `zwlr_layer_shell_v1` (KDE, Hyprland, Sway), попапы запускаются в отдельном процессе Spell; иначе (например GNOME) — fallback на winit |
| `spell` | Только layer-shell; без протокола запуск завершается ошибкой |

`auto` и `spell` требуют сборки с `--features spell`.

Рендерер окна настроек задаётся стандартной переменной Slint `SLINT_BACKEND`: `winit-software`, `winit-femtovg` или `winit-skia`. На Spell-попапы она не влияет, у них всегда Skia software. Цветные emoji в окнах winit рисует только `winit-skia`.

Одновременно работает один экземпляр на `XDG_RUNTIME_DIR`. Второй запуск без аргументов завершится с ошибкой `slint-shell is already running`.

## Hot reload интерфейса (Live Preview)

Slint поддерживает обновление `.slint` без перезапуска приложения через [Live Preview](https://docs.slint.dev/latest/docs/slint/guide/tooling/live-preview/). Для запуска из корня репозитория:

```sh
SLINT_LIVE_PREVIEW=1 cargo run -p slint-shell --features slint/live-preview
```

Перед запуском завершите уже работающий экземпляр. Откройте настройки через трей или командой `target/debug/slint-shell show settings` в другом терминале. После сохранения `.slint` интерфейс перезагружается; связь с Rust, свойства, callbacks и модели сохраняются. При синтаксической ошибке остаётся предыдущая версия интерфейса до исправления.

Изменения Rust-кода и интерфейса между Slint и Rust (например, переименование используемого свойства или callback) требуют пересборки и перезапуска. Live Preview предназначен только для разработки: включайте `slint/live-preview` через командную строку, не добавляя его в обычные или release-сборки.

Этот режим пока не проверен в приложении; совместимость со Spell-попапами также требует отдельной проверки.

## Где лежит конфигурация

Debug-сборки никогда не трогают реальный конфиг пользователя:

- по умолчанию — `<repo>/.dev-files/config/config.json` (данные — в `.dev-files/data/`);
- `LHC_DEV_DIR=/путь` задаёт другой каталог (относительный путь считается от текущего каталога).

Это тот же каталог, что использует `pnpm tauri:dev`. Обе оболочки видят один конфиг. Изменения, сделанные в Tauri, Slint перечитывает автоматически раз в секунду.

Release-сборки используют `~/.config/dev.bozonx.left-hand-control/` и `~/.local/share/dev.bozonx.left-hand-control/`, те же каталоги, что и Tauri.

Пути разрешает `lhc_core::storage::StoragePaths::resolve()`. Не вычисляйте их в оболочке самостоятельно.

## Управление запущенным экземпляром

Бинарник с аргументами работает как клиент: отправляет команду запущенному экземпляру через Unix-сокет и завершается.

```sh
target/debug/slint-shell show settings
target/debug/slint-shell show emoji
target/debug/slint-shell show quick
target/debug/slint-shell hide
target/debug/slint-shell toggle-mapper
target/debug/slint-shell preferences dark ru    # dark|light, ru|en
target/debug/slint-shell ping
target/debug/slint-shell quit
```

Ответ `queued` означает, что команда поставлена в очередь UI, а не что окно уже готово к вводу.

## Mapper

1. Откройте настройки, выберите «Устройство клавиатуры». Путь сохраняется в `settings.inputDevicePath` конфига.
2. Нажмите «Mapper вкл / выкл» (или `slint-shell toggle-mapper`, или пункт меню трея).

Mapper, раскладка, game mode и активное окно работают через `lhc-core` так же, как в Tauri. Linux-бэкенд держит межпроцессную блокировку: одновременно mapper может работать только в одной оболочке. Сохранение назначения клавиши в редакторе сразу передаётся запущенному mapper.

Редактор пока меняет только безусловные правила базового слоя (`tapAction`). Для клавиш, у которых есть только условные или послойные правила, он откажет и попросит воспользоваться полным редактором.

## Макросы

В настройках откройте вкладку «Макросы». Здесь можно создавать и удалять макросы, менять их порядок, редактировать шаги и копировать встроенные макросы из каталога «Системные макросы».

Шаг принимает клавишу или сочетание (`Ctrl+KeyC`), текст (`text:Привет`), ссылку на действие (`macro:id`, `cmd:id`, `sys:id`, `app:id`) или паузу (`pause:100`, от 0 до 10000 мс). Ссылки также доступны в списке «Выбрать действие». Пустые шаги пропускаются. Индивидуальные задержки допускают 0–2000 мс; пустое поле использует значение из настроек.

Кнопка «Сохранить» проверяет ID, действия и циклические ссылки, сохраняет макрос в `current-layout.yaml` и обновляет работающий mapper. «Отмена» отбрасывает черновик. Изменение ID не переписывает существующие назначения; места использования видны в редакторе. Если раскладка изменилась во время редактирования, редактор попросит открыть макрос заново.

Проверка страницы на временной конфигурации, без изменения пользовательских макросов:

```sh
cargo run --locked -p slint-shell --example macros
cargo test --locked --workspace --features slint-shell/spell
cargo clippy --locked -p lhc-core -p slint-shell -p left-hand-control --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo run -p slint-shell --example editor -- --smoke
cargo run -p slint-shell --example interactions
```

## Эмоджи, быстрые действия и команды

Во втором верхнем тулбаре доступны «Эмоджи», «Быстрые действия» и «Команды». Изменения редактируются в черновике: «Сохранить» записывает их в `current-layout.yaml`, «Отмена» перечитывает сохранённое состояние. Переключение ячеек и страниц внутри редактора сохраняет ввод в черновике. Перед переходом в другой раздел нажмите «Сохранить».

Эмоджи и быстрые действия разбиты на страницы по 15 ячеек (`Q W E R T / A S D F G / Z X C V B`). Можно добавлять, переименовывать, удалять и переставлять страницы, менять местами ячейки, очищать назначения. Для эмоджи доступны каталог и собственный текст до 100 UTF-16 единиц. Быстрое действие принимает сочетание клавиш, `text:…`, `macro:id`, `cmd:id`, `sys:id` или `app:id`; ссылки можно выбрать из списка.

У команд редактируются имя, ID и текст команды Linux; доступны места использования и копирование ссылки `cmd:id`. После сохранения команды необходимо разрешить кнопкой «Разрешить сохранённые команды». Изменение ID или текста сбрасывает разрешение; его можно отозвать вручную. Разрешения хранятся отдельно в `config.json`. Если сохранённая конфигурация не может быть применена, работающий mapper останавливается, чтобы не продолжать выполнение старых команд.

Обычные и Spell-меню используют сохранённые данные. Поиск быстрых действий работает по всем страницам; без поискового запроса отображается выбранная страница. Выбор выполняется через запущенный mapper, как в Tauri. Эмоджи выбираются мышью, стрелками с Enter или буквами ячеек; цифры переключают страницы. В быстрых действиях доступны мышь, стрелки с Enter и Alt+1…9.

Проверка редакторов на временной конфигурации:

```sh
cargo run --locked -p slint-shell --example menus
LHC_MENUS_NAV=1 cargo run --locked -p slint-shell --example menus
```



## Переменные окружения

| Переменная | Назначение |
| --- | --- |
| `RUST_LOG` | Фильтр логов `env_logger`; по умолчанию `info,zbus=warn,tracing=warn` |
| `LHC_DEV_DIR` | Каталог конфигурации для debug-сборок |
| `SLINT_BACKEND` | Рендерер окна настроек |
| `SLINT_SHELL_POPUPS` | `winit` / `auto` / `spell` |
| `SLINT_SHELL_HOTKEYS=off` | Не слушать evdev-хоткеи Ctrl+Alt+F11 / F12 |
| `SLINT_SHELL_INPUT=/dev/input/eventN` | Слушать хоткеи только с одного устройства |
| `SLINT_SHELL_INSERT=1` | Только сборка с `--features probes`: диагностический Spell worker на KDE (возврат фокуса и вставка); в обычной сборке меню выполняют назначения через mapper |
| `SLINT_SHELL_OUTPUT=<имя>` | Монитор для Spell-попапов |
| `SLINT_SHELL_METRICS=/путь.csv` | Писать CSV задержек для скриптов измерений; без переменной метрики выключены |
| `SLINT_SHELL_SOCKET` | Имя IPC-сокета в `XDG_RUNTIME_DIR`; удобно для параллельного тестового экземпляра |

Диагностические переменные стендов (`SLINT_SHELL_TEST_INJECT`, `SLINT_SHELL_ISOLATED_INPUT`, `SLINT_SHELL_SPELL_LIFECYCLE`, `SLINT_SHELL_SPELL_INITIAL`) описаны в `apps/slint-shell/README.md`.

Второй экземпляр рядом с основным, например для экспериментов:

```sh
LHC_DEV_DIR=/tmp/lhc-dev SLINT_SHELL_SOCKET=lhc-dev.sock SLINT_SHELL_HOTKEYS=off \
  cargo run -p slint-shell
SLINT_SHELL_SOCKET=lhc-dev.sock target/debug/slint-shell quit
```

Учтите, что трей у второго экземпляра тоже появится, а mapper запустится только в одном из них.

## Проверки перед коммитом

```sh
cargo fmt -p lhc-core -p slint-shell -p left-hand-control --check
cargo clippy --locked -p lhc-core -p slint-shell -p left-hand-control \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets        # сборка без Spell
```

UI-сценарии запускаются в графической сессии и сами закрываются по завершении:

```sh
cargo run -p slint-shell --example editor -- --smoke     # редактор клавиш
cargo run -p slint-shell --example interactions          # настройки, сохранение, навигация, тема, язык
```

С `LHC_EDITOR_SNAPSHOTS=<каталог>` пример `interactions` сохраняет снимки экранов (`.ppm`).

Стенды задержек, lifecycle и возврата ввода (`bench-evdev`, `bench-return`, `scripts/bench-virtual.sh` и др.) описаны в `apps/slint-shell/README.md`.

## Структура кода

```
crates/lhc-core/src/        # общий код обеих оболочек, без UI
├── config_document.rs      # редактируемый config.json: валидация, защита от внешних изменений
├── storage.rs              # StoragePaths::resolve() — пути dev/release
├── events.rs               # шина CoreEvent: единственный канал «ядро → оболочка»
├── layout/ gamemode/ active_window/   # наблюдатели состояния системы
├── mapper/                 # движок, Linux evdev/uinput, portal, runtime
└── platform/               # определение ОС / DE / сессии

apps/slint-shell/
├── src/lib.rs              # роли процесса: настройки / Spell worker / CLI-клиент
├── src/command.rs          # типизированные команды и формат IPC
├── src/app/                # процесс настроек: состояние, попапы, mapper, надзор за worker
├── src/document.rs         # единственный путь записи конфига: сохранение → mapper → обновление страниц
├── src/pages/              # страницы окна настроек, по одному Slint-глобалу на страницу
├── src/keyboard.rs         # сетки клавиатуры и подписи клавиш
├── src/popup_model.rs      # данные и клавиши попапов (общие для winit и Spell)
├── src/spell.rs            # процесс Spell-попапов (layer-shell)
├── src/i18n.rs             # идентификаторы сообщений для Locale.text()
├── src/platform/linux/     # evdev-хоткеи, ksni-трей, Wayland activation, возврат ввода
├── src/platform/portable/  # Windows/macOS: global-hotkey, tray-icon, native input
├── ui/*.slint              # страницы и компоненты; ui/types.slint — перечисления; ui/i18n.slint — переводы сообщений
├── translations/ru/        # PO-перевод (исходные строки на английском)
└── vendor/spell-framework/ # локальная копия Spell с патчами (см. PATCHES.md)
```

Правила:

- Доменная логика (конфиг, mapper, наблюдатели, пути) живёт в `lhc-core`. Оболочка только отображает её и вызывает операции.
- Ядро уведомляет оболочки только через `lhc_core::events::bus()`. Подписка выполняется один раз при старте в `app/mapper.rs`.
- Страницы меняют конфиг только через `Document::edit`; остальные страницы обновляются подпиской `Document::subscribe`. Не держите `Document::read()` во время вызовов UI, которые могут редактировать.
- Значения между Rust и Slint передаются перечислениями из `ui/types.slint`, а не числовыми кодами.
- Весь видимый пользователю текст идёт через `@tr`. Rust передаёт `Msg` → `Message { id, arg, count }`, перевод делает `Locale.text()`. Добавляя сообщение, обновите `src/i18n.rs`, `ui/i18n.slint` и `translations/ru/LC_MESSAGES/slint-shell.po`; тест `every_slint_string_has_a_russian_translation` проверяет, что каждая строка `@tr` переведена.
- Платформенные различия держатся в `src/platform/`, общий UI от ОС не зависит.

## Что зависит от окружения

| Окружение | Состояние |
| --- | --- |
| KDE Plasma 6, Wayland | Основная среда: Spell-попапы, фокус, вставка, раскладка, системные действия |
| Hyprland, Sway | Layer-shell есть, Spell должен работать; не принято, вставка (`SLINT_SHELL_INSERT`) использует KWin и там недоступна |
| GNOME Wayland | Нет layer-shell: `auto` уходит в winit-fallback; трею нужно расширение AppIndicator |
| X11 | Окна winit; тип окна Utility, поведение зависит от WM |

## Частые проблемы

- **`slint-shell is already running`** — уже запущен экземпляр с тем же сокетом. Выполните `target/debug/slint-shell quit` или задайте другой `SLINT_SHELL_SOCKET`. Устаревший сокет после аварии удаляется при следующем запуске.
- **Первая сборка очень долгая или падает на Skia** — проверьте clang, cmake, ninja и python.
- **`No readable Ctrl+Alt+F11/F12 evdev devices`** — нет доступа к `/dev/input`; см. раздел про группу `input`, либо отключите хоткеи `SLINT_SHELL_HOTKEYS=off`.
- **Emoji монохромные или квадратами** — установите Noto Color Emoji; для окон winit используйте `SLINT_BACKEND=winit-skia`.
- **`compositor does not advertise zwlr_layer_shell_v1`** — композитор без layer-shell (GNOME); используйте `SLINT_SHELL_POPUPS=auto` или `winit`.
- **Сохранение отклонено: «конфигурация изменена другим приложением»** — файл изменили параллельно (например, в Tauri). Документ уже перечитан, повторите изменение.

## Основное окно приложения

Окно использует системную декорацию и два отдельных горизонтальных тулбара. Первый содержит переход к библиотеке, сохранение текущей раскладки, «Сохранить как…», запуск/остановку маппера, активную раскладку, язык клавиатуры, игровой режим и настройки. Второй содержит библиотеку, имя редактируемой раскладки и разделы редактора. Точка около имени означает отличие от сохранённого файла библиотеки; около команд — необходимость подтверждения выполнения.

Тема, язык, устройства, поведение клавиш, задержки макросов, определение игрового режима, способы ввода текста Linux и пути конфигурации собраны на странице «Настройки». Кнопка «Сохранить настройки» записывает параметры. Маппер запускается с сохранёнными настройками. Системная тема следует теме оконной системы. Выбор устройства недоступен во время работы маппера.

В разделе «Клавиатура» доступны базовые назначения и слои. Вымышленный каталог из 500 действий, несохраняемая демонстрационная сетка, нижние кнопки открытия попапов и переключатели темы/языка вне настроек удалены. Каталог базовой клавиатуры содержит реальные клавиши, встроенные действия, макросы и команды конфигурации.

Проверка интерфейса использует временную конфигурацию, не меняя пользовательские файлы:

```bash
cargo run --locked -p slint-shell --example editor -- --smoke
cargo run --locked -p slint-shell --example interactions
cargo run --locked -p slint-shell --example menus
```

## Библиотека и автоматическая активация

«Создать пустую» и «На основе Ivan K» сразу создают раскладку и открывают её правила.
Имя подбирается автоматически; при совпадении добавляется номер. Переименование,
описание, копирование и удаление доступны в выпадающем меню «…» на карточке.

«Редактировать» на карточке открывает раскладку для редактирования. «Активировать»
меняет раскладку mapper в ручном режиме независимо от редактора. В автоматическом
режиме применяется первая подходящая раскладка в порядке библиотеки; кнопки
перемещения в меню «…» меняют приоритет. Режим переключения выбирается
в настройках и применяется сразу. Диалог «Условия…» на карточке позволяет сохранить включение в Auto,
белый и чёрный списки по системному языку, приложению и игровому режиму. Пустой
список не ограничивает выбор; чёрный список имеет приоритет. Без совпадения
mapper пропускает ввод без переназначений.

События языка, активного окна и Game Mode обновляют работающий mapper. Повторное
событие без смены выбранной раскладки не перезапускает её конфигурацию. При ошибке
применения mapper останавливается. Переименование сохраняет ссылки в условиях,
порядке и настройках активации; удаление убирает эти ссылки.

Вызов конкретной страницы работает и с winit, и со Spell:

```sh
target/debug/slint-shell show emoji 3
target/debug/slint-shell show quick 2
cargo run --locked -p slint-shell --example library
cargo run --locked -p slint-shell --example macros
```

Эти UI-примеры используют временные конфигурации. Приёмка физического ввода,
Windows mapper и матрица прочих DE остаются отдельными платформенными задачами.

## Выбор клавиш и действий

Триггеры правил и дополнительные клавиши слоёв используют режим выбора одной
клавиши. Назначения клавиатуры, действия правил и слоёв, шаги макросов и быстрое
меню используют тот же диалог с каталогами макросов, команд, системных действий
и вводом текста. Паузы доступны только для шагов макросов. Поиск работает по
названию и коду во всех доступных категориях. Выбор остаётся черновиком до
нажатия «Применить»; «Отмена» его отбрасывает.

«Захватить клавишу» считывает физический код в активном окне, независимо от
раскладки, включая Escape, numpad и левые/правые модификаторы. В режиме действий
можно захватить сочетание; отдельный модификатор выбирается при отпускании.
«Остановить захват» или потеря фокуса прекращают ожидание. Системные сочетания,
которые композитор не передаёт приложению, можно выбрать в каталоге или ввести
вручную.

Проверка диалога на временной конфигурации:

```sh
cargo run --locked -p slint-shell --example action-picker
LHC_PICKER_SNAPSHOT=/tmp/lhc-picker.ppm cargo run --locked -p slint-shell --example action-picker
```
