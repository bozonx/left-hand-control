# Left Hand Control

Desktop keyboard mapper на Rust и Slint. Перехватывает физическую клавиатуру,
применяет слои, правила, макросы и команды и выдаёт результат через виртуальное
устройство. Настройки доступны из системного трея; Emoji и Quick actions
открываются отдельными попапами.

Основная платформа — Linux, KDE Plasma 6 на Wayland. Оболочка компилируется
на Windows и macOS, но перехват клавиатуры на этих платформах пока не реализован.

## Сборка и запуск

Нужен Rust stable ≥ 1.85. На Linux установите clang, cmake, ninja, Python,
библиотеки D-Bus, Fontconfig, Wayland и xkbcommon. Подробные команды и настройка
доступа к `/dev/input` и `/dev/uinput` — в [руководстве Linux](docs/slint-dev-linux.md).

Все команды выполняются из корня репозитория:

```sh
cargo run --locked -p slint-shell
# Layer-shell попапы с автоматическим переходом на winit:
SLINT_SHELL_POPUPS=auto cargo run --locked -p slint-shell --features spell
```

Приложение запускается со скрытыми окнами и остаётся в трее. Открыть настройки:

```sh
target/debug/slint-shell show settings
```

В одном runtime-каталоге работает один экземпляр. Команды `show emoji`,
`show quick`, `hide`, `toggle-mapper`, `ping` и `quit` отправляются ему по IPC.

## Конфигурация

Debug-сборки используют `dev-files/<os>/` внутри репозитория. `LHC_DEV_DIR`
задаёт другой базовый каталог с той же структурой по ОС. На Linux конфиг находится
в `dev-files/linux/.config/dev.bozonx.left-hand-control/config.json`, данные —
в `dev-files/linux/.local/share/dev.bozonx.left-hand-control/`.

Release-сборки используют системные config/data-каталоги. На Linux это
`~/.config/dev.bozonx.left-hand-control/` и
`~/.local/share/dev.bozonx.left-hand-control/`. Настройки сохраняются автоматически;
раскладки записываются в библиотеку только при явном сохранении. Внешние изменения
конфига обнаруживаются автоматически.

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

CI проверяет core и оболочку на Linux, Windows и macOS. Ручной workflow Desktop E2E
запускает Slint-сценарии на стендах KDE Wayland и Windows; это не означает приёмку
перехвата клавиатуры на Windows.

## Структура

- `crates/lhc-core/` — конфигурация, хранение, mapper и системные интеграции;
  `assets/` содержит встроенную раскладку, `tests/fixtures/` — совместимые образцы.
- `apps/slint-shell/` — UI, IPC, трей, платформенные адаптеры и Spell worker;
  `assets/` содержит иконку приложения.
- `docs/` — настройка разработки и тестовых стендов.
- `dev_docs/` — планы функций и платформенной приёмки.
- `scripts/` — диагностика оболочки и экспериментальные XKB-проверки.

## Документация

- [Инструкции для агентов](AGENTS.md)
- [Разработка на Linux](docs/slint-dev-linux.md)
- [Оболочка, сценарии и диагностика](apps/slint-shell/README.md)
- [Windows-стенд](docs/windows-testing.md)
- [Кроссплатформенная приёмка](dev_docs/cross-platform-product-validation-plan.md)
- [Политика безопасности](SECURITY.md)

Лицензия: [GPL-3.0-or-later](LICENSE).
