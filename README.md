# Left Hand Control

Desktop keyboard mapper на Rust и Slint. Перехватывает физическую клавиатуру,
применяет слои, правила, макросы и команды и выдаёт результат через виртуальное
устройство. Настройки доступны из системного трея; Emoji и Quick actions
открываются отдельными попапами.

Основная платформа — Linux, KDE Plasma 6 на Wayland. Оболочка компилируется
на Windows и macOS. Windows backend перехвата через хуки сессии и `SendInput`
реализован, но ещё не принят; на macOS перехват не реализован.

## Сборка и запуск

Нужен Rust stable ≥ 1.92. На Linux установите clang, cmake, ninja, Python,
библиотеки D-Bus, Fontconfig, Wayland и xkbcommon. Подробные команды и настройка
доступа к `/dev/input` и `/dev/uinput` — в [руководстве разработчика](docs/development/guide.md).

Все команды выполняются из корня репозитория:

```sh
cargo run --locked -p slint-shell
# Layer-shell попапы с автоматическим переходом на winit:
LHC_POPUPS=auto cargo run --locked -p slint-shell --features spell
```

Приложение запускается со скрытыми окнами и остаётся в трее. Открыть настройки:

```sh
target/debug/left-hand-control show settings
```

В одном runtime-каталоге работает один экземпляр; повторный запуск без аргументов
открывает его настройки. Команды `show emoji`, `show quick`, `hide`, `toggle-mapper`,
`ping` и `quit` отправляются ему по IPC; `--version` и `--help` работают без него.

## Установка Linux release

Установка архива без `sudo`, пакеты deb/rpm, права и удаление описаны в
[инструкции по установке](docs/install-linux.md); сборка релиза — в
[руководстве разработчика](docs/development/guide.md#5-выпуск). Бинарник устанавливается
по стабильному пути; пользовательские настройки сохраняются при обновлении.

Для Windows 11 x64 подготовлена portable ZIP-поставка: [запуск, обновление и удаление](docs/install-windows.md).

## Конфигурация

Debug-сборки используют `dev-files/<os>/` внутри репозитория. `LHC_DEV_DIR`
задаёт другой базовый каталог с той же структурой по ОС. На Linux конфиг находится
в `dev-files/linux/.config/dev.bozonx.left-hand-control/config.json`, данные —
в `dev-files/linux/.local/share/dev.bozonx.left-hand-control/`.

Release-сборки используют системные config/data-каталоги. На Linux это
`~/.config/dev.bozonx.left-hand-control/` и
`~/.local/share/dev.bozonx.left-hand-control/`. Настройки сохраняются автоматически;
раскладки записываются в библиотеку только при явном сохранении. Внешние изменения
конфига обнаруживаются автоматически; сохранение проверяет уже произошедшие
изменения, но не блокирует параллельную запись внешним редактором. Поддерживается текущий формат конфигурации
и раскладок; автоматической миграции устаревших полей нет.

## Проверки

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo clippy --locked -p lhc-core -p slint-shell \
  --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets
```

UI-сценарии требуют графической сессии и используют временную конфигурацию:

```sh
cargo run --locked -p slint-shell --example editor -- --smoke
cargo run --locked -p slint-shell --example interactions
cargo run --locked -p slint-shell --example library
```

CI проверяет core и оболочку на Linux, Windows и macOS. Desktop e2e выполняются
локально в KDE Wayland и Windows VM; [отчёты и границы приёмки](docs/testing/README.md)
относятся к указанным исходникам и пакетам. Зелёный CI не заменяет desktop-приёмку.

## Структура

- `crates/lhc-core/` — конфигурация, хранение, mapper и системные интеграции;
  `assets/` содержит встроенную раскладку, `tests/fixtures/` — образцы для тестирования парсера.
- `apps/slint-shell/` — UI, IPC, трей, платформенные адаптеры и Spell worker;
  `assets/` содержит иконку приложения и каталог emoji.
- `docs/` — [индекс документации](docs/README.md): разработка, проектирование и тестирование.
- `apps/slint-shell/scripts/` — диагностика и проверки оболочки.
- `scripts/experimental/` — экспериментальные XKB-проверки.

## Документация

- [Инструкции для агентов](AGENTS.md)
- [Руководство разработчика: запуск, проверки, e2e, выпуск](docs/development/guide.md)
- [Поведение приложения](docs/features.md)
- [Установка на Linux](docs/install-linux.md)
- [Portable Windows ZIP](docs/install-windows.md)
- [Оболочка, сценарии и диагностика](apps/slint-shell/README.md)
- [Windows-стенд](docs/testing/windows.md)
- [Ручная приёмка](docs/testing/manual-acceptance.md)
- [Политика безопасности](SECURITY.md)

Лицензия: [GPL-3.0-or-later](LICENSE).
