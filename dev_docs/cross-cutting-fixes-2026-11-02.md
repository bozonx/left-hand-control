# Исправления сквозного аудита — 2026-11-02

Исправлены все четыре пункта из `cross-cutting-audit-2026-11-02.md`.

## Изменения

1. **Порядок конфигураций и действий.** Linux control queue объединяет только последовательные сообщения `Config`. Сообщение выполнения служит барьером: `Config(C1) → Execute → Config(C2)` сохраняет C1 перед выполнением. Добавлен регрессионный тест очереди.
2. **Единый снимок правил и меню.** `RuntimeConfig` содержит исходную раскладку. `Document` сохраняет весь успешно переданный снимок; winit и Spell получают меню из него. Ошибка переключения сохраняет прежние меню вместе с прежними правилами. Последующая перезапись файла библиотеки не подменяет меню работающего mapper. При остановленном mapper меню следуют документу. Добавлен тест отказа переключения и изменения файла библиотеки.
3. **Сбор заголовков.** Shell обновляет свою потребность при смене состояния mapper, включая остановку, аварийное завершение и неудачный запуск. При успешном запуске снимок регистрируется до обновления состояния UI. Независимый флаг потребности game-mode не затрагивается. Проверены потребность работающего снимка и её отсутствие после остановки/замены снимка.
4. **Ограниченная доставка событий.** Очередь подписчика ограничена 256 событиями; состояния и диагностика объединяются по типу. При переполнении вытесняется старый запрос. Действия приложения сохраняют порядок, истекают через секунду и отзываются при остановке/новом запуске mapper. Исходные время и поколение передаются до UI и повторно проверяются перед выполнением, включая уже опубликованный callback. Добавлены тесты перегрузки, отзыва поколения и истечения срока.

Обновлены `docs/development/api.md` и запись в `CHANGELOG.md` под `[Unreleased]`. Зависимости, Cargo.lock, форматы файлов, CLI и IPC не менялись.

## Команды и результаты

| Команда | Результат |
| --- | --- |
| `cargo fmt -p lhc-core -p slint-shell` | Успешно |
| `cargo fmt -p lhc-core -p slint-shell --check` | Успешно |
| `git diff --check` | Успешно |
| `cargo test --locked -p lhc-core --lib` | Промежуточный прогон: 582 passed, 3 ignored |
| `cargo test --locked -p slint-shell --lib document::tests` | 10 passed |
| `cargo clippy --locked -p lhc-core -p slint-shell --features slint-shell/spell --all-targets --no-deps -- -D warnings` | Успешно |
| `cargo test --locked --workspace --features slint-shell/spell` | 801 passed, 0 failed, 11 ignored; из них 8 mapper e2e затем запущены отдельно |
| `cargo check --locked -p slint-shell --all-targets` | Успешно |
| `cargo run --locked -p slint-shell --example editor -- --smoke` | Успешно |
| `cargo run --locked -p slint-shell --example interactions` | Успешно |
| `cargo run --locked -p slint-shell --example menus` | Успешно |
| `cargo test --locked -p lhc-core --test mapper_e2e -- --ignored` | 8 passed, 0 failed |
| `cargo deny check` | Ошибка advisories: RUSTSEC-2026-0433, pulldown-cmark 0.13.5; bans, licenses, sources — ok |
| `cargo deny --format json check > /tmp/lhc-cross-cutting-deny.jsonl 2>&1` | Та же ошибка; структурированный вывод использован для диагностики |

`pulldown-cmark 0.13.5` уже присутствовал в неизменённом lockfile. Обновление зависимостей не входит в эти четыре исправления; advisory не подавлялся.

## Окружение и пределы проверки

Linux, KDE, Wayland (`XDG_CURRENT_DESKTOP=KDE`, `XDG_SESSION_TYPE=wayland`). UI-сценарии выполнены в графической сессии с временными конфигурациями. Mapper e2e прошёл через синтетические устройства evdev/uinput. Полный workspace проверен со Spell, отдельная сборка всех targets — без Spell.

Windows, macOS и другие композиторы не запускались. Сценарий отказа автопереключения покрыт регрессионным unit-тестом; отдельная ручная приёмка этого сценария в Spell worker не проводилась.

Логи финальных проверок:
- `/tmp/lhc-cross-cutting-workspace.log`
- `/tmp/lhc-cross-cutting-without-spell.log`
- `/tmp/lhc-cross-cutting-editor.log`
- `/tmp/lhc-cross-cutting-interactions.log`
- `/tmp/lhc-cross-cutting-menus.log`
- `/tmp/lhc-cross-cutting-mapper-e2e.log`
- `/tmp/lhc-cross-cutting-deny.jsonl`
