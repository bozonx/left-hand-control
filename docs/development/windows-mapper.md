# Windows mapper: подготовка и порядок реализации

Состояние: Windows backend перехвата **не реализован**. `--suite smoke`
проверяет оболочку и нативный ввод в VM, но не подтверждает работу mapper.
Рабочее поведение Linux остаётся эталоном для общих правил движка.

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

## Этапы реализации

### 1. Общий движок без зависимости от evdev

Сейчас `mapper/{engine,action,keys,validation,commands,system}` ограничены Linux,
а движок и действия используют `evdev::KeyCode`. Сначала выделить внутренний
тип физической клавиши и перехода down/up/repeat в `lhc-core` и адаптер evdev.
Сохранить текущие строки `KeyboardEvent.code` в конфиге; формат файлов не менять.
Перенести существующие тесты движка на общий тип, сохранив Linux проверки
конвертации клавиш. Не подменять физическую клавишу символом текущей раскладки.

Критерий: общие тесты движка выполняются на Windows и Linux, а Fedora mapper
проходит прежние remap, chord, layer, tap/hold, macro и stuck-key сценарии.

### 2. Минимальный Windows backend в ядре

Добавить `mapper/windows/`, подключить через `mapper/runtime.rs`; shell получает
состояние через существующий `CoreEvent`. Начать с session-wide passthrough,
затем одной замены `KeyA -> KeyB` и подавления исходной клавиши. Отдельно
проверить left/right modifiers, extended scan codes и repeat.

Кандидат первого backend — `WH_KEYBOARD_LL` и `SendInput`. Hook требует message
loop, может подавить событие и снимается системой при превышении timeout;
синхронная часть callback должна быть короткой. Команды, текст и прочие медленные
операции выполнять вне callback. [Контракт Microsoft](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).

На выходе проверять количество событий, принятое `SendInput`, и исключать
повторную обработку собственных injected events. Ввод ограничен integrity level
цели; обычный процесс не должен обещать работу в elevated-приложениях или на
secure desktop. [Контракт SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput).

Выбор отдельной физической клавиатуры — отдельное решение: текущий Linux API
работает с устройствами, а hook-кандидат — с потоком сессии. Raw Input умеет
различать устройства, но сам по себе не заменяет подавляющий hook. Не показывать
выбор устройства как работающую функцию до проверки нужной семантики.
[Raw Input](https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input).

### 3. Lifecycle и общие правила

Подключить hot reload конфига, слои/tap-hold, chords, макросы, остановку и
восстановление. Сохранять гарантии из [platforms.md](platforms.md#mapper-runtime-guarantees):
отпускание удержанных клавиш, отмена макросов, отсутствие stale side effects,
межпроцессная блокировка. Неподдерживаемые действия отклонять явно.

### 4. Layout, text и системные действия

Добавлять по отдельным проверяемым возможностям: US/RU, Unicode, смена фокуса,
отмена text job, доступные `sys:` действия. Код popup-ввода не является готовым
backend маппера; общую логику размещать в `lhc-core`.

## Будущая Windows mapper e2e матрица

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

До реализации backend эти строки являются планом приёмки, а не пропущенными
тестами с зелёным результатом. VM не заменяет финальную проверку на физическом
Windows-компьютере с несколькими клавиатурами.
