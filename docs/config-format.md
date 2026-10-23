# Формат config.json и файлов раскладок

Приложение хранит два вида файлов:

- `config.json` — настройки этого компьютера и состояние приложения;
- `layouts/<имя>.json` — раскладки библиотеки. Раскладка переносима: её можно скопировать на другой компьютер.

Пути к каталогам описаны в [руководстве разработчика](development/guide.md). Оба файла редактируются через приложение. Ручные изменения применяются после перезапуска и могут быть перезаписаны следующим сохранением.

## Общие правила

- Каждый файл начинается с `"schemaVersion": 1` — версии схемы файла. Файл без версии или с другой версией не загружается. Если версия больше поддерживаемой, приложение сообщает, что файл создан более новой версией.
- Чтение строгое: неизвестное поле, значение не того типа или неизвестное значение перечисления — ошибка с номером строки и столбца. Файл при этом не перезаписывается. Так приложение никогда не сохраняет файл, молча выбросив непонятые части.
- Необязательные поля можно не писать: берётся значение по умолчанию. Приложение не записывает пустые списки и значения по умолчанию там, где это указано ниже.
- Смысловые проблемы (ссылка на несуществующий макрос, неизвестная клавиша в действии) не мешают загрузке. Их показывает диагностика правил, а mapper не запускается с блокирующими ошибками.
- Новые необязательные поля могут появиться в версии 1. Старая сборка откажется читать такой файл, но не испортит его. Несовместимые изменения повышают `schemaVersion`.

### Идентификаторы

`id` уникален внутри своей коллекции: слои, правила, макросы, команды, страницы
быстрых действий, страницы эмодзи, автоправила и процессы игрового режима.
Сравнение ID регистрозависимое. Разные коллекции могут использовать одинаковый `id`. ID шагов макроса уникальны
внутри одного макроса и служат только редактору; в JSON шаги остаются строками.
Явно заданный ID не может быть пустым или иметь пробелы по краям. Дубликаты —
ошибка загрузки, а не правило «последний побеждает».

У правил, страниц, автоправил и процессов можно пропустить `id`: загрузчик
детерминированно создаёт его, предварительно резервируя все явно заданные ID
этой коллекции. При сохранении созданные ID записываются в файл.

### Контракт таймингов

Все длительности — целые миллисекунды. Дробные, отрицательные и выходящие за
диапазон числовые поля отклоняются при загрузке и редактировании; молчаливого
ограничения значений сохранённого файла нет. Некорректное действие `pause:`
в загруженном макросе показывает смысловая диагностика; mapper с ним не запускается.

| Поля | Допустимый диапазон, мс |
| --- | --- |
| `settings.timing.holdMs`, `doubleTapMs`; `rules[].hold.timeoutMs`, `doubleTap.timeoutMs` | 50–1000 |
| `settings.timing.longHoldMs`; `rules[].longHold.timeoutMs` | 100–60000 |
| `settings.timing.comboMs` | 10–500 |
| `settings.timing.macroStepPauseMs`, `macroModifierDelayMs`; `macros[].stepPauseMs`, `modifierDelayMs` | 0–2000 |
| `pause:<мс>` в шагах макроса | 1–10000 |
| `settings.timing.macroTimeoutMs` | 1000–600000 |

Отсутствующий тайминг правила или макроса (или `null` вместо него) наследует
соответствующую настройку; при сохранении наследуемое поле опускается.
В `settings.timing` значения `null` недопустимы.
Ноль разрешён только для межшаговой задержки и задержки модификаторов: он
означает отсутствие дополнительной задержки. Ноль не означает наследование,
отключение распознавания жеста или неограниченное выполнение макроса.
`macroTimeoutMs` ограничивает выполнение макроса целиком; отдельная допустимая
пауза не отменяет этот предел. Hold и long hold — альтернативные режимы,
поэтому требования `longHoldMs > holdMs` нет.

### Диагностика содержимого

Перед запуском mapper проверяется вся активная раскладка: действия правил,
клавиши и действия слоёв, шаги и циклы вызовов макросов, действия быстрых меню,
условия окон и поддерживаемые сочетания параметров правил. Ошибка содержит
путь к элементу; для макроса указывается конкретный шаг. Некорректное действие
в слое или меню больше не пропускается молча. Диагностика не переписывает файл:
раскладку со смысловыми ошибками можно открыть и исправить в редакторе.


## Действия

Действие — строка:

| Вид | Пример |
| --- | --- |
| клавиша или сочетание (коды W3C `KeyboardEvent.code`) | `KeyA`, `Ctrl+KeyC`, `Ctrl+Shift+ArrowLeft` |
| макрос | `macro:copyLine` |
| команда | `cmd:<id>` |
| системное действие | `sys:switchDesktop2`, `sys:showDesktop` |
| действие приложения | `app:showQuickMenu1`, `app:gameModeToggle` |
| глобальное сочетание KDE | `kde:kwin/Window Maximize` |
| ввод текста | `text:Привет` |
| пауза (только в макросах) | `pause:200` |

Там, где действие необязательно, отсутствие поля значит «как у физической клавиши», а `null` — «ничего не делать». Пустая строка — ошибка.

Встроенные макросы (`macro:moveLineDown` и др.) используют то же пространство имён `macro:`. Если в раскладке есть макрос с таким же id, используется он: так новые встроенные макросы не меняют существующие раскладки. Редактор не даёт создать макрос с id встроенного.

Действия `app:showQuickMenuN` и `app:showEmojiMenuN` открывают страницу меню по номеру (1–5). При перестановке и удалении страниц в редакторе ссылки перенумеровываются вслед за страницами.

## config.json

```json
{
  "schemaVersion": 1,
  "settings": {
    "launchOnStartup": false,
    "startMapperOnLaunch": true,
    "appearance": "system",
    "highContrast": false,
    "reduceMotion": false,
    "layoutOsd": false,
    "layerHint": false,
    "locale": "auto",
    "keyboardDevice": "/dev/input/by-id/usb-Keyboard-event-kbd",
    "mouseDevice": "/dev/input/by-id/usb-Mouse-event-mouse",
    "timing": {
      "holdMs": 200,
      "longHoldMs": 1000,
      "doubleTapMs": 200,
      "comboMs": 50,
      "tapDecision": "permissiveHold",
      "macroStepPauseMs": 20,
      "macroModifierDelayMs": 5,
      "macroTimeoutMs": 10000
    },
    "layoutMode": "manual",
    "activeLayoutId": "user:Основная",
    "autoRules": [
      {
        "id": "games",
        "layoutId": null,
        "when": { "gameMode": "on" }
      },
      {
        "id": "blender",
        "layoutId": "user:Blender",
        "when": { "windows": ["org.blender.Blender"], "keyboardLayouts": ["us"] }
      }
    ],
    "autoDefaultLayoutId": "user:Основная",
    "commandsEnabled": false,
    "gameMode": {
      "detectors": {
        "gamemoded": true, "fullscreen": false, "steam": true, "lutris": true,
        "heroic": true, "umu": true, "gamescope": true, "appCategory": false, "gameBar": false
      },
      "processes": [
        { "id": "p1", "name": "factorio", "onlyActiveWindow": true },
        { "id": "p2", "name": "obs", "onlyActiveWindow": false, "exclude": true }
      ]
    },
    "linux": {
      "textMode": "libei",
      "pasteShortcut": "shift-insert",
      "ydotoolPath": "",
      "xdotoolPath": ""
    }
  },
  "state": {
    "editedLayoutId": "user:Основная",
    "layoutOrder": ["user:Основная", "user:Blender"],
    "gameMode": "auto"
  }
}
```

### settings

| Поле | Значения | По умолчанию |
| --- | --- | --- |
| `appearance` | `system`, `light`, `dark`, `eink` | `system` |
| `locale` | `auto`, `en-US`, `ru-RU` | `auto` |
| `keyboardDevice`, `mouseDevice` | путь к устройству ввода; в Linux — стабильная ссылка `/dev/input/by-id/…` или `/dev/input/by-path/…`, которая переживает перезагрузку и переподключение | нет |
| `timing` | значения по умолчанию для правил и макросов, в миллисекундах; правило или макрос может задать своё | см. пример |
| `timing.tapDecision` | `permissiveHold` (нажатие другой клавиши фиксирует удержание только при вложенном отпускании или по таймауту), `holdOnOtherKeyPress` (сразу) | `permissiveHold` |
| `layoutMode` | `manual` — работает `activeLayoutId`; `auto` — первое подходящее правило `autoRules`, иначе `autoDefaultLayoutId` | `manual` |
| `autoRules[].layoutId` | id раскладки или `null` (mapper выключен) | `null` |
| `autoRules[].enabled` | `false` сохраняет правило, но не применяет его | `true` |
| `autoRules[].when` | `gameMode` (`on`/`off`), `keyboardLayouts` (короткие id системной раскладки, например `us`), `windows` (шаблоны окон); пустые условия подходят всегда | — |
| `commandsEnabled` | разрешить действия `cmd:`; без этого команды из раскладок не выполняются | `false` |
| `gameMode.detectors` | источники автоопределения игр; каждая платформа использует только свои | зависит от ОС |
| `gameMode.processes[]` | процесс-признак игры; с `"exclude": true` — процесс, который игровой режим не включает | — |
| `linux.textMode` | `libei`, `libei-pure`, `keycode`, `clipboard`, `ydotool`, `xdotool` | `libei` |
| `linux.pasteShortcut` | `shift-insert`, `ctrl-v`, `ctrl-shift-v` | `shift-insert` |

Id раскладок библиотеки имеют вид `user:<имя файла без .json>`. При переименовании раскладки в приложении все ссылки в `config.json` обновляются.

### state

Состояние, которое приложение запоминает само: раскладка, открытая в редакторе (`editedLayoutId`), порядок библиотеки (`layoutOrder`, он же приоритет `app:activateLayoutN`) и положение переключателя игрового режима (`gameMode`: `auto`, `on`, `off`).

## Файл раскладки

```json
{
  "schemaVersion": 1,
  "description": "Основная раскладка",
  "layers": [
    {
      "id": "nav",
      "name": "Навигация",
      "description": "Стрелки под правой рукой",
      "extraKeyOrder": ["F13"],
      "keys": { "KeyH": "ArrowLeft", "KeyQ": null, "F13": "macro:copyLine" }
    },
    {
      "id": "desktop",
      "keys": { "Tab": "Alt+Tab", "CapsLock": "Alt+Shift+Tab" },
      "holdModifiers": ["Tab", "CapsLock"]
    }
  ],
  "rules": [
    {
      "id": "caps",
      "key": "CapsLock",
      "tap": "Escape",
      "hold": { "layer": "nav", "timeoutMs": 180 }
    },
    {
      "id": "alt",
      "key": "AltLeft",
      "tap": "Enter",
      "hold": { "layer": "desktop" }
    },
    {
      "id": "meta",
      "key": "MetaLeft",
      "tap": "Escape",
      "hold": { "keys": "ControlLeft" },
      "tapDecision": "holdOnOtherKeyPress"
    },
    {
      "id": "shift",
      "key": "ShiftLeft",
      "tap": "Ctrl+KeyC",
      "doubleTap": { "action": "Ctrl+KeyX", "timeoutMs": 250 }
    },
    {
      "id": "chord",
      "key": "ShiftLeft+ControlRight",
      "longHold": { "action": "sys:screenOff", "timeoutMs": 1500 }
    },
    {
      "id": "games",
      "key": "Space",
      "enabled": false,
      "when": { "gameMode": "off", "keyboardLayouts": ["us"], "windows": ["code"], "excludeWindows": ["title:Secret"] },
      "tap": null
    }
  ],
  "macros": [
    { "id": "copyLine", "name": "Copy line", "stepPauseMs": 10, "steps": ["Home", "Shift+End", "Ctrl+KeyC"] }
  ],
  "commands": [
    { "id": "music", "name": "Play/pause", "unix": "playerctl play-pause", "windows": "", "workingDirectory": "/tmp" }
  ],
  "quickActionPages": [
    {
      "id": "system",
      "name": "System",
      "cells": {
        "KeyQ": { "name": "Lock", "action": "kde:ksmserver/Lock Session", "icon": "lock" },
        "KeyW": "sys:showDesktop"
      }
    }
  ],
  "emojiPages": [
    { "id": "faces", "name": "Faces", "cells": { "KeyQ": "😀" } }
  ]
}
```

### Слои

`id` обязателен, `name` по умолчанию равен `id`. В `keys` код клавиши указывает на действие; `null` глушит клавишу в слое, а клавиша, которой нет в `keys`, прозрачна (работает правило базового слоя). `extraKeyOrder` хранит порядок показа дополнительных клавиш.

`holdModifiers` — клавиши слоя, у сочетания которых модификаторы остаются нажатыми до отпускания клавиши, удерживающей слой; основная клавиша нажимается и отпускается вместе с физической. Так работает переключение окон: в примере удержание `AltLeft` включает `desktop`, Tab выдаёт Alt+Tab, повторные Tab листают окна при всё ещё зажатом Alt, CapsLock (Alt+Shift+Tab) листает назад, а отпускание `AltLeft` отпускает Alt и выбирает окно. Модификаторы, которые не нужны следующей такой клавише, отпускаются (Shift после Alt+Shift+Tab → Alt+Tab), Alt остаётся. Каждая клавиша из списка должна иметь действие в `keys` (не `null`).

### Правила

| Поле | Смысл |
| --- | --- |
| `key` | физическая клавиша или сочетание клавиш-участников через `+` (обязательно) |
| `id` | идентификатор; если его нет, генерируется при загрузке |
| `enabled` | `false` — правило сохранено, но не работает |
| `when` | условия: `gameMode` (`on`/`off`), `keyboardLayouts`, `windows`, `excludeWindows` (шаблоны окон: id приложения или `title:…`); должны выполняться все |
| `tap` | действие при нажатии; нет поля — сама клавиша, `null` — ничего |
| `doubleTap` | `{ "action", "timeoutMs"? }` — действие при двойном нажатии |
| `hold` | что делает удержание (см. ниже) |
| `longHold` | `{ "action", "timeoutMs"? }` — действие при долгом удержании; нельзя сочетать с `hold` |
| `tapDecision` | как решать нажатие/удержание для этого правила; по умолчанию `settings.timing.tapDecision` |

Поля `hold`:

| Поле | Смысл |
| --- | --- |
| `layer` | слой, который действует, пока клавиша удерживается |
| `keys` | клавиша или сочетание, зажатые во время удержания (например `ControlLeft`). Нет поля — сама клавиша (или ничего, если задан `layer`), `null` — ничего |
| `timeoutMs` | через сколько нажатие считается удержанием; по умолчанию `settings.timing.holdMs` |

### Макросы и команды

У макроса обязателен `id` (латиница, цифры, `_`, `-`). `steps` — список действий, включая `pause:<мс>`. `stepPauseMs` и `modifierDelayMs` переопределяют значения из `settings.timing`.

У команды обязателен `id`. `unix` — сценарий POSIX `sh` для Linux и macOS, `windows` — сценарий PowerShell. Команды из раскладки выполняются только при `settings.commandsEnabled`.

### Страницы меню

`quickActionPages` и `emojiPages` — страницы меню быстрых действий и эмодзи. Ключи `cells` — клавиши сетки `KeyQ KeyW KeyE KeyR KeyT / KeyA KeyS KeyD KeyF KeyG / KeyZ KeyX KeyC KeyV KeyB`. Ячейка быстрого действия — строка с действием или объект `{ "name"?, "action", "icon"? }`, ячейка эмодзи — вставляемый текст. Если списка страниц нет, приложение показывает одну пустую страницу.
