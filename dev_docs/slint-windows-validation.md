# Проверка Slint-прототипа на Windows

Статус: базовый Windows-проход выполнен частично 2026-09-21–23. Оставшаяся
приёмка ограничена особенностями Slint shell и его Windows-адаптеров.

Воспроизводимая настройка стенда описана в
[руководстве по Windows-тестированию](../docs/windows-testing.md).

## Цель

Проверить, подходит ли Slint для desktop shell приложения на Windows:

- native event loop, окна, popup focus и keyboard navigation;
- рендеринг, цветные emoji, прозрачность и DPI;
- tray и `global-hotkey` как способы открыть UI;
- IPC/single-instance и устойчивость show/hide;
- возврат в исходное приложение и вставку после вызова глобальным hotkey.

Полноценный Windows mapper, раскладки, слои, макросы, системные действия,
packaging и широкая продуктовая матрица проверяются позднее по
[кроссплатформенному продуктовому плану](cross-platform-product-validation-plan.md).

Вызов Emoji или Quick из tray не обязан возвращать фокус или вставлять текст.
Tray нужен для открытия интерактивного UI, Settings, переключателя и Quit.

## Стенд

- Windows 11 x64 в интерактивной KVM/QEMU VM.
- Один монитор 1920×1080; DPI 100%, 125% и 150%.
- English (US) и Russian.
- Release-сборка `prototypes/slint-shell` с `winit-skia`.
- Notepad для одного внешнего smoke-сценария вставки.

Зафиксировать Windows build, commit, Rust, Slint, renderer и DPI. Производительность
VM не сравнивать с Linux-стендом.

## Уже подтверждено 2026-09-21–23

- native `cargo test --locked` и release-сборка;
- скрытый старт, tray, loopback IPC и single-instance;
- portable `interactions`: clipboard, keyboard, modal, DnD, cancellation, edge
  scrolling, theme, locale и hidden popups;
- 100/100 IPC show/hide и штатный `quit`;
- Settings, Emoji и Quick интерактивны;
- цветные emoji с `winit-skia`;
- задержанный IPC вернул фокус в Notepad и точно вставил Quick и Emoji;
- текстовый clipboard после вставки восстановлен.
- `Ctrl+Alt+F11` и `Ctrl+Alt+F12` открывают нужный popup; повторное сочетание
  скрывает его без дубликата;
- первая стрелка работает без предварительного клика, Enter выбирает, а Esc и
  потеря фокуса закрывают popup без вставки;
- hotkey-flow вернул фокус в Notepad и по Enter ровно один раз вставил выбранные
  смешанные строки `Действие 01 / Action 01` и `Действие 02 / Action 02`.
- `winit-skia` цветно отрисовал контрольные `☕`, `😀` и `👩‍💻`; каждый из них
  через hotkey-flow вставился в Notepad ровно один раз.
- WinForms receiver сохраняет text, UTF-16, Unicode code points и focus events;
  smoke-проход записал выбранный `☕` как единственную кодовую точку `9749`.

Старый блокер tray focus return снят: вставка после tray-вызова больше не входит в
контракт. Старые одиночные тестовые клавиши заменены на `Ctrl+Alt+F11` для Emoji и
`Ctrl+Alt+F12` для Quick.

## Автоматическая приёмка

### Сборка и lifecycle

- [x] `cargo test --locked --manifest-path prototypes/slint-shell/Cargo.toml --bin slint-shell`.
- [x] Release-сборка без Linux-only feature `spell`.
- [x] Повторный запуск работает как IPC client, а не второй UI server.
- [x] 100 чередующихся `show emoji` / `show quick` / `hide`, затем `quit`.
- [x] Выход с открытым и скрытым окном не оставляет process или listener.
- [ ] После такого выхода tray icon не остаётся визуально в системном трее.
- [x] Неверный `SLINT_SHELL_POPUPS=spell` завершается с понятной ошибкой.

### Slint UI и окна

- [ ] Settings, Emoji и Quick открываются без дубликатов и невидимого focus trap.
- [x] Первая стрелка обрабатывается popup без предварительного клика.
- [x] Enter выбирает; Esc и потеря фокуса закрывают без действия.
- [ ] Tab/Shift+Tab, modal focus return, Save и Cancel имеют правильную семантику.
- [ ] DnD, edge scrolling, dropdown у края и каталог из 500 записей работают.
- [ ] Валидный каталог из 1500 emoji прокручивается и остаётся управляемым.
- [ ] Скрытые popup корректно получают новые theme и locale перед повторным показом.

### Windows adapters

- [ ] Tray открывает Settings, Emoji и Quick; Mapper не зависает; Quit завершает процесс.
- [ ] Выбор в popup, открытом из tray, не выполняет автоматическую вставку.
- [x] `Ctrl+Alt+F11` открывает Emoji, `Ctrl+Alt+F12` — Quick.
- [x] Повтор hotkey скрывает уже открытый соответствующий popup без дубликата.
- [ ] Конфликт регистрации даёт диагностическую ошибку; `SLINT_SHELL_HOTKEYS=off`
  отключает регистрацию.
- [ ] После hotkey-вызова из Notepad выбор возвращает подтверждённый foreground и
  вставляет Latin, Cyrillic и composed emoji точно один раз.
- [ ] Если foreground изменился или его нельзя подтвердить, ввод отменяется и не
  попадает в другое окно.
- [ ] Esc и focus loss ничего не вставляют; Ctrl/Alt/Shift/Win не остаются зажатыми.

Для повторяемой матрицы использовать небольшой Win32 receiver, который записывает
focus events и точный Unicode. Notepad оставить одним внешним smoke-тестом.

### Renderer и DPI

- [x] `winit-skia` показывает цветные BMP, surrogate-pair и composed emoji.
- [ ] На 100%, 125% и 150% окно не обрезано, click areas совпадают с отрисовкой,
  modal/dropdown/popup остаются видимыми.
- [ ] Popup находится в доступной рабочей области текущего монитора и не создаёт
  постоянную кнопку taskbar.
- [ ] Dark/light и ru/en меняются без пересоздания процесса и без критического clipping.

### Минимальная устойчивость

- [ ] 20 автоматических циклов для каждого popup через hotkey: focus, первая стрелка,
  Enter, возврат/результат; отдельный цикл Esc.
- [ ] После прогрева скрытый процесс не создаёт постоянную CPU-нагрузку.
- [ ] Working set и handle count после 100 show/hide не растут без ограничений.

## Короткий ручной проход

Автоматизация не заменяет визуальную оценку следующих пунктов:

- цвет, прозрачность, скругления и отсутствие артефактов;
- читаемость и геометрия при 100%, 125% и 150%;
- фактическое tray menu и отсутствие stale icon после Quit;
- первая hotkey-активация из Notepad.

Достаточно одного прохода на DPI; полную комбинацию всех состояний не повторять.

## Отложено до продуктовой Windows-приёмки

- key interception и remapping engine;
- раскладки, слои, macros, system actions и configurable bindings;
- Windows Terminal и браузеры как полная матрица получателей;
- сохранение всех нетекстовых clipboard formats;
- taskbar во всех положениях и auto-hide;
- fullscreen, hot-plug второго монитора, lock/unlock и sleep/wake;
- installer, update, permissions и Accessibility/security compatibility;
- длительные performance и soak tests.

## Критерий решения по Slint

Slint shell на Windows принят, если:

- release build, native tests, single-instance, IPC и tray lifecycle успешны;
- Settings и оба popup интерактивны мышью и клавиатурой;
- Skia, theme, locale и 100/125/150% не имеют блокирующих дефектов;
- оба глобальных сочетания стабильно открывают нужный popup;
- hotkey-flow безопасно возвращает и вставляет текст либо отменяет ввод при
  неподтверждённом target;
- 20/20 автоматических циклов каждого popup проходят без неверного адресата,
  дубликатов, утечки клавиш и постоянной фоновой нагрузки.

Этот результат подтверждает только выбор Slint и минимальную архитектуру Windows
shell. Он не означает готовность Windows mapper или продукта целиком.
