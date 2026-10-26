# Общие проверки и CI

## Что проверяется

Источник актуальных команд CI — [ci.yml](../../.github/workflows/ci.yml).
Linux job проверяет workspace со Spell, clippy и сборку без Spell. Windows и macOS
проверяют portable core/shell, clippy и компиляцию release. Во всех трёх job
проверяется форматирование. Linux job также запускает Python-тесты Windows VM-контроллера;
Windows job проверяет синтаксис PowerShell без запуска GUI.

Unit/integration покрывают модель и валидацию конфигурации, парсер действий,
движок, журнал клавиш, IPC, хранение и переводы в пределах `cfg` текущей ОС.
Тесты с `#[ignore]` обычная Cargo-команда не выполняет.
CI выполняется на push/PR в main/master, вручную и через вызывающий workflow.
Наличие job в YAML само по себе не доказывает успешный прогон конкретного коммита.

## Как запустить

Linux с системными зависимостями из [руководства](../development/guide.md):

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo test --locked --workspace --features slint-shell/spell --all-targets
cargo clippy --locked -p lhc-core -p slint-shell --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo check --locked -p slint-shell --all-targets
python3 -m unittest discover -s apps/slint-shell/scripts/vm -p test_windows_stand.py
```

На Windows/macOS, нативно на соответствующей ОС:

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo test --locked -p lhc-core -p slint-shell --all-targets
cargo clippy --locked -p lhc-core -p slint-shell --all-targets --no-deps -- -D warnings
cargo check --release --locked -p slint-shell
```

Windows дополнительно:

```powershell
powershell -NoProfile -File apps/slint-shell/scripts/windows/check-windows-syntax.ps1
```

При ограниченной памяти допустим `CARGO_BUILD_JOBS=1` (в PowerShell —
`$env:CARGO_BUILD_JOBS = '1'`). Это ограничивает параллелизм сборки, а не объём покрытия.

## Headless Linux mapper e2e

```sh
cargo test --locked -p lhc-core --test mapper_e2e -- --ignored
```

Нужны `/dev/uinput` и доступ к тестовым `/dev/input`: сценарии создают виртуальное
входное устройство и проверяют путь evdev → engine → uinput. Они не подтверждают
работу popup, portal, фокуса, трея или физической клавиатуры.
CI job `mapper-e2e` запускает их только если удалось включить uinput; иначе выдаёт
предупреждение. Успешный остальной CI не закрывает этот пропуск.

## Пакеты и контейнеры

[release.yml](../../.github/workflows/release.yml) собирает Linux-пакеты в Ubuntu 22.04,
выполняет rpmlint и smoke установки/удаления в Debian 12/13, Ubuntu 22.04/24.04,
Fedora, openSUSE Tumbleweed и Arch. Теги `latest` изменяются: в отчёте нужно сохранять
фактическую версию/образ, а не только название job.
Подготовка и локальные команды — [раздел выпуска](../development/guide.md#5-выпуск).

Контейнеры проверяют упаковку, зависимости, файлы и install/remove, но не Wayland,
права реальной сессии, автозапуск после входа, трей, перехват клавиатуры или portal.
Контейнерная проверка Arch/openSUSE не является desktop-приёмкой этих систем.

## Что общие проверки не подтверждают

- Поведение GUI и ОС: глобальные хоткеи, фокус, tray, текст в сторонних приложениях.
- Реальные устройства, hotplug, сон, блокировку, несколько экранов и DPI.
- Другую ОС при сборке на текущем хосте: платформенные ветки проверяются нативно.
- Успех полного workspace, если запускался только отдельный тест или пакет.

Desktop/VM-проверки запускаются локально по [документам сред](README.md).
Результаты и артефакты оформляются по [общим правилам](results/README.md).
