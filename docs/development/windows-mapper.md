# Windows mapper: состояние реализации и приёмка

Состояние: этапы 1–4 реализованы в коде (`mapper/windows/`, общий
`mapper/output.rs`, `mapper/key_code.rs`, `mapper/scancode.rs`, `layout/windows.rs`)
и проверены сборкой, clippy и модульными тестами; приёмка в VM и на физическом
компьютере по полной матрице ниже ещё не пройдена. `--suite smoke` проверяет оболочку,
нативный ввод и именованные mapper-сценарии в VM; результат каждого сценария
фиксируется отдельно и не заменяет оставшиеся ручные проверки.
Рабочее поведение Linux остаётся эталоном для общих правил движка.
Стенд оболочки и нативного ввода прошёл [Windows 11 smoke 2026-10-18](../testing/results/windows/2026-10-18-vm-windows11-4.txt).
Полный [автоматический прогон Windows 11 от 2026-10-20](../testing/results/windows/2026-10-20-vm-windows11-8.txt)
прошёл 13 mapper-сценариев на release-драйвере, native tests/clippy, изоляцию
release-хранилища и lifecycle со 100 циклами попапов: 18 этапов, 0 ошибок.
Это не закрывает отмеченные ниже ручные проверки и аварийное завершение.

## Стенд перед началом

1. Подготовить Windows VM по [windows.md](../testing/windows.md): MSVC, Rust,
   OpenSSH, автологон, отдельный NTFS-диск `LHC-CACHE`, интерактивная задача
   `LHC-E2E`. Сохранить базовый диск, UEFI variables и TPM после выключения.
2. Запустить `python3 apps/slint-shell/scripts/vm/run-windows.py --vm <dir>`.
   Раннер использует overlay; `target/` остаётся на диске кэша между прогонами.
3. Получить успешные `windows-smoke` и `hardware-keyboard`: QEMU USB-клавиатура
   вводит `abc`, затем Shift+X даёт `abcX`, Ctrl+Home, Shift+End и `ok` заменяют выделение.
   `windows-receiver.ps1` сохраняет текст, Unicode, события down/up и модификаторы.
   Снимки и журнал событий должны быть доступны в каталоге логов прогона.
4. Перед изменениями общего движка получить успешный Linux набор из AGENTS.md
   и сохранить Fedora L4 как регрессионную базу.

SSH здесь служит для управления и чтения результатов. Само приложение и receiver
запускаются задачей планировщика в интерактивной сессии. Ввод через QMP проходит
через эмулируемую USB-клавиатуру; `SendKeys` не заменяет этот входной путь.

## Реализованные этапы и критерии приёмки

### 1. Общий движок без зависимости от evdev

Общий движок использует внутренний `mapper::key_code::KeyCode`; Linux адаптирует
его к evdev, Windows — к scan codes. Строки `KeyboardEvent.code` и формат
конфига сохранены. Общие тесты движка доступны на обеих платформах.
При приёмке физическую клавишу нужно отличать от символа текущей раскладки.

Критерий: общие тесты движка выполняются на Windows и Linux, а Fedora mapper
проходит прежние remap, chord, layer, tap/hold, macro и stuck-key сценарии.

### 2. Минимальный Windows backend в ядре

`mapper/windows/` подключён через `mapper/runtime.rs`; shell получает
состояние через `CoreEvent`. Приёмку начать с session-wide passthrough,
затем одной замены `KeyA -> KeyB` и подавления исходной клавиши. Отдельно
проверить left/right modifiers, extended scan codes и repeat.

Backend использует `WH_KEYBOARD_LL` и `SendInput`. Hook требует message
loop, может подавить событие и снимается системой при превышении timeout;
синхронная часть callback должна быть короткой. Команды, текст и прочие медленные
операции выполнять вне callback. [Контракт Microsoft](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).

На выходе проверять количество событий, принятое `SendInput`, и исключать
повторную обработку собственных injected events. Ввод ограничен integrity level
цели; обычный процесс не должен обещать работу в elevated-приложениях или на
secure desktop. [Контракт SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

Выбор отдельной физической клавиатуры — отдельное решение: текущий Linux API
работает с устройствами, а Windows hook — с потоком сессии. Raw Input умеет
различать устройства, но сам по себе не заменяет подавляющий hook. Не показывать
выбор устройства как работающую функцию до проверки нужной семантики.
[Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input).

### 3. Lifecycle и общие правила

Приёмка должна подтвердить hot reload конфига, слои/tap-hold, chords, макросы,
остановку и восстановление, а также гарантии из [platforms.md](platforms.md#mapper-runtime-guarantees):
отпускание удержанных клавиш, отмена макросов, отсутствие stale side effects,
межпроцессная блокировка. Неподдерживаемые действия отклонять явно.

### 4. Layout, text и системные действия

Проверять по отдельным возможностям: US/RU, Unicode, смена фокуса,
отмена text job, доступные `sys:` действия. Код popup-ввода не является готовым
backend маппера; общую логику размещать в `lhc-core`.

## Windows mapper e2e матрица для приёмки

| Шаг | Ожидаемый результат в нативном receiver |
| --- | --- |
| Native baseline | `abc`, Shift+X, Ctrl+Home, Shift+End; down/up; текущий smoke |
| Remap / swallow / passthrough | ровно заменённый текст; исходная клавиша подавлена |
| Chord / left-right modifiers / repeat | правильные события и текст без дубликатов |
| Tap / hold / layer roll | те же решения, что в тестах общего движка |
| Macro / cancel / live config | порядок и паузы; отмена без остаточного ввода |
| Stop / quit / crash with held keys | следующий обычный ввод без залипших модификаторов |
| Own injected events | нет рекурсии и повторного remap |
| US / RU / Unicode | сравнение точного текста и code points |
| Popup / focus change | ввод только в ожидаемую цель или отмена |
| Replug / suspend / session change | явное состояние, восстановление и доступный native input |
| Elevated target / secure desktop | отдельно зафиксированные ограничения backend |

Эти строки являются планом приёмки реализованного backend; успешные сборка
и модульные тесты не подтверждают прохождение этой матрицы. VM не заменяет
финальную проверку на физическом Windows-компьютере с несколькими клавиатурами.

## Автоматизированный стенд

`run-windows.py --suite smoke` и `--suite full` запускают реальные mapper
сценарии через QMP; full включает shell smoke и release lifecycle.
Сценарии находятся в `scripts/vm/windows_mapper.py`, Rust driver —
`crates/lhc-core/examples/windows-mapper-stand.rs`, native receiver —
`scripts/windows-receiver.ps1` (пути scripts относительно `apps/slint-shell/`).
Driver не читает пользовательский config: принимает runtime JSON через отдельный
mailbox, вызывает публичный `mapper::runtime` и снимает hooks при штатном выходе.
Строки матрицы считаются пройденными только при наличии соответствующего
успешного именованного шага в отчёте конкретного прогона.

Принудительное завершение процесса не выполняет `engine.shutdown`: Windows
сама снимает hooks, но отпускание виртуальных modifiers, отправленных ранее
через `SendInput`, этим не гарантируется. Штатный stop/quit и crash — разные
гарантии; проверка stop не закрывает приёмку crash. До отдельного решения и
проверки восстановления после crash эту строку нельзя отмечать принятой.
