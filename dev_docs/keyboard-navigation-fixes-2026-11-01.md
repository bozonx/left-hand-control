# Исправления клавиатурной навигации — 2026-11-01

Продолжение [аудита](keyboard-navigation-audit-2026-11-01.md). Изменения относятся к Slint shell; формат конфигурации, IPC и зависимости не меняются.

## Правила взаимодействия

- Клавиатурное управление доступно постоянно. Tab/Shift+Tab, стрелки, Home/End/PageUp/PageDown и Enter/Space/F1 включают показ фокуса. Обычный ввод текста не меняет modality; нажатие указателя отключает рамки. Текстовые поля, HC и E-ink сохраняют собственные правила показа фокуса.
- Вкладки используют ручную активацию: стрелки перемещают реальный фокус внутри группы, Enter/Space открывают вкладку; Tab выходит из группы. Home/End выбирают первую/последнюю цель фокуса. В группу входят через активную вкладку; вертикальные вкладки используют Up/Down, горизонтальные — Left/Right.
- Списки устройств ограничивают стрелки границами списка. Меню и radio groups поддерживают циклический обход. Сетки используют пространственную навигацию без перехода через противоположный край.
- Escape сначала обрабатывает поле/внутренний контрол, затем верхний диалог. Tab в диалоге остаётся внутри его границы; модифицированные Ctrl/Alt/Meta+Tab не подменяются обычным обходом.
- В ComboBox движение по открытому списку не сохраняет значение; Enter/Space/Tab подтверждают его, Escape отменяет предварительный выбор. Поддерживаются Home/End и поиск по префиксу.
- Popup Emoji/Quick actions: стрелки/Home/End перемещают фокус между ячейками, Enter/Space выбирают ячейку, Tab/Shift+Tab переводят фокус между сеткой и Close, PgUp/PgDn переключают страницы. Буквенные и цифровые hotkeys сохранены в winit и Spell.

Ориентиры: [WAI Keyboard Interface](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/), [Tabs](https://www.w3.org/WAI/ARIA/apg/patterns/tabs/), [Grid](https://www.w3.org/WAI/ARIA/apg/patterns/grid/), [Combobox](https://www.w3.org/WAI/ARIA/apg/patterns/combobox/). Используется модель взаимодействия этих рекомендаций; native роли остаются в пределах доступных Slint/AccessKit ролей.

## Связь с аудитом

| Пункты | Изменение |
| --- | --- |
| K01 | Карточки picker имеют настоящий фокус. Стрелки ограничены каталогом; Enter применяет сфокусированную карточку. Модель сохраняет делегаты и ограничивает индекс входа диапазоном. |
| K02 | Убрано глобальное перехватывание Escape по списку флагов; DialogFrame обрабатывает его после дочерних контролов. |
| K03–K04 | Общий NavigationFocus для верхних и редакторских вкладок, разделов настроек, слоёв, страниц меню и категорий picker. |
| K05 | Общий обход пунктов меню с Up/Down/Home/End; Tab закрывает popup и продолжает обход от инициатора. |
| K06 | Одна остановка Tab на клавиатурную сетку; стрелки между клавишами обеих рук. Ctrl+X/Ctrl+V обменивают назначения; Escape снимает отметку источника. |
| K07 | Отдельные составные группы палитры Emoji и текстовых примеров с реальным фокусом и пространственной навигацией. |
| K08 | Стрелки списка устройств не переходят к поиску, scrollbar и кнопкам диалога. |
| K09 | Modality учитывает активацию Enter/Space/F1, включая символьный пробел; тот же observer применяется к winit popup. |
| K10 | Нажатие кнопок, checkbox, вкладок и picker-кнопок сначала фокусирует инициатора. |
| K11 | ComboBox разделяет предварительный и сохранённый выбор, потребляет событие открытия и поддерживает отмену, подтверждение, поиск и реальный фокус опций. |
| K12 | Начальный фокус разрушительных подтверждений — Cancel. |
| K13 | Popup имеют фокусируемые ячейки и Close; изменены клавиши переключения страниц, подсказки и все PO-каталоги. |
| K14 | Clear — обычная доступная IconButton с Tab/Enter/Space и рамкой фокуса. |

Дополнительно: ButtonGroup и HotkeyGrid переводят настоящий фокус между доступными дочерними элементами; редактор меню не пересоздаёт неизменившиеся делегаты ячеек. Возврат из диалога допускает инициатор, скрытый только Flickable, и показывает его. Рамка различает настоящий внешний инициатор и дочерний автофокус во время создания диалога; удалённый, но ещё живой в кэше диалог не удерживает модальность. После смены страницы предусмотрено отложенное восстановление фокуса. Клавиатурная перестановка строк/вкладок восстанавливает фокус через устойчивые accessibility IDs.

В vendored i-slint-core добавлен modal-aware `focus_outside` и открыт существующий предикат видимости с учётом Flickable; изменения и ограниченная история слабых ссылок фокуса записаны в PATCHES.md. Новых зависимостей и обновления Slint нет.

## Регрессии

`keyboard-navigation` дополнен проверками границ списка устройств, стрелок/Home/End/выхода из вкладок, перестановки страниц с сохранением фокуса, отмены ComboBox, Escape в поиске и Enter после перемещения фокуса в picker. Сохранены 1440 событий Tab/Shift+Tab во вложенных диалогах и прежние проверки активации/возврата фокуса.

`popup-grid` использует те же bindings навигации, что приложение и Spell, и проверяет реальную цель стрелок, Enter, Tab/Shift+Tab и края сетки. `layer-keys` проверяет также стрелки и обмен назначений с клавиатуры. Unit-тесты проверяют набор клавиш modality и геометрию направленного перехода.

## Итоговая проверка

Среда: Linux, изолированный KWin 6.7.5 Wayland, временные XDG/LHC каталоги и отдельная D-Bus session. Реальная пользовательская конфигурация не использовалась. Целевые keyboard-navigation/popup-grid/layer-keys ранее прошли с winit-software; финальный полный GUI-прогон выполнен с winit-skia.

| Команда | Результат |
| --- | --- |
| `cargo fmt -p lhc-core -p slint-shell --check` | PASS |
| `CARGO_BUILD_JOBS=1 cargo clippy --locked -p lhc-core -p slint-shell --features slint-shell/spell --all-targets --no-deps -- -D warnings` | PASS |
| `CARGO_BUILD_JOBS=1 cargo test --locked --workspace --features slint-shell/spell` | PASS: 779 passed, 11 ignored, 0 failed |
| `CARGO_BUILD_JOBS=1 cargo check --locked -p slint-shell --all-targets` | PASS, без Spell |
| `cargo deny check` | PASS: advisories, bans, licenses, sources |
| `CARGO_BUILD_JOBS=1 cargo build --locked -p slint-shell --features spell --example inline-edit` | PASS после обновления ожидания безопасного начального фокуса |
| `CARGO_BUILD_JOBS=1 cargo clippy --locked -p slint-shell --features spell --example inline-edit --no-deps -- -D warnings` | PASS после последнего изменения сценария |
| `git diff --check` | PASS |

Полный `cargo test` также собрал GUI examples. Финальные команды GUI:

```sh
SLINT_BACKEND=winit-skia LHC_AUDIT_OUT=/tmp/lhc-keyboard-final \
  LHC_AUDIT_COMMAND=/tmp/lhc-keyboard-final/run-ui.sh \
  bash dev_docs/accessibility-audit-2026-10-29/tools/run-isolated.sh
SLINT_BACKEND=winit-skia LHC_AUDIT_OUT=/tmp/lhc-keyboard-final \
  LHC_AUDIT_SUFFIX=-atspi \
  LHC_AUDIT_COMMAND='/usr/bin/python3 /tmp/lhc-keyboard-final/watch-atspi.py' \
  bash dev_docs/accessibility-audit-2026-10-29/tools/run-isolated.sh
```

Копии использованных скриптов: [run-ui.sh](keyboard-navigation-2026-11-01/tools/run-ui.sh), [watch-atspi.py](keyboard-navigation-2026-11-01/tools/watch-atspi.py). Перед повторением нужно создать каталог `LHC_AUDIT_OUT` и скопировать туда скрипты либо указать их актуальные пути в `LHC_AUDIT_COMMAND`.

`run-ui.sh` последовательно выполнил `timeout 90 target/debug/examples/editor --smoke`, затем `timeout 90 target/debug/examples/<scenario>` для interactions, keyboard-navigation, popup-grid, layer-keys, menus, library, small-window, macros, inline-edit, undo-redo, drag-scroll, notifications. Все 13 — PASS. Первое выполнение inline-edit обнаружило устаревшее ожидание немедленного подтверждения удаления; сценарий исправлен: Enter на Cancel отменяет, повторное открытие и Tab → Enter подтверждают. Повторный полный прогон прошёл.

AT-SPI: PASS, 132 внешних события focused; проверены роли page tab и дочерних контролов. Это проверка настоящего accessibility bridge, но не полная ручная сессия со screen reader.

Ограничения: физический ввод через evdev/uinput и реальные устройства этим прогоном не проверялись; ignored mapper e2e не запускались. Spell собран и покрыт общими тестами модели/клавиш, отдельный runtime Spell worker не запускался. Windows/macOS и другие DE не проверялись. Это исправление выявленных пунктов аудита, а не сертификация полного соответствия WCAG.
