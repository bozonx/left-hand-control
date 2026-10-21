# macOS

На macOS сейчас проверяется переносимость core и Slint-оболочки на уровне CI.
Mapper interception не реализован. Desktop-e2e и полная приёмка macOS не заявлены.

## Что проверяется

Job `Slint / macos-latest` в [ci.yml](../../.github/workflows/ci.yml):
форматирование, нативные unit/integration-тесты core/shell со всеми Cargo targets,
clippy с `-D warnings` и release check оболочки.
`macos-latest` — изменяемый образ: версию macOS и архитектуру нужно брать из
конкретного CI run. Успех сборки не является проверкой запуска GUI.

## Как повторить

На Mac установить Rust stable из требований проекта и Xcode Command Line Tools,
затем выполнить из корня репозитория:

```sh
cargo fmt -p lhc-core -p slint-shell --check
cargo test --locked -p lhc-core -p slint-shell --all-targets
cargo clippy --locked -p lhc-core -p slint-shell --all-targets --no-deps -- -D warnings
cargo check --release --locked -p slint-shell
```

Для исследования UI в графической сессии можно запустить:

```sh
cargo run --locked -p slint-shell --example editor -- --smoke
cargo run --locked -p slint-shell --example interactions
```

Эти команды — способ ручного дополнительного запуска; регулярного подтверждённого
macOS desktop-набора и отчёта о его приёмке сейчас нет. Не переносить Linux/Windows
результаты UI на Mac.

## Что не тестируется и не принято

- Перехват клавиатуры и mapper-сценарии: backend отсутствует.
- Реальные глобальные хоткеи, tray, возврат фокуса, Unicode-вставка в приложения.
- Системные разрешения Accessibility/Input Monitoring, раскладки и системные действия.
- Сон/пробуждение, lock/unlock, hotplug, Retina/несколько экранов.
- Установка/обновление/подпись/notarization готового macOS-пакета.
- Матрица версий macOS и обеих архитектур за пределами конкретного CI run.

Ручной план — [manual-acceptance.md](manual-acceptance.md); наличие Mac в этом плане
не означает, что проверки уже выполнены. Для новых результатов записывать ОС,
архитектуру, коммит, команды и пропуски по [общим правилам](results/README.md).
