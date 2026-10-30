# Автоматическая функциональная и релизная приёмка

Детерминированные требования проверяются программно. Отсутствие сценария,
прошлого пакета, VM или разрешения — открытая задача автоматизации, а не пункт
для ручного тестировщика. Только фактически выполненный PASS закрывает требование;
наличие скрипта, SKIP, короткий smoke или переустановка той же версии его не заменяют.

Выполненные проверки и открытые дефекты на 2026-10-22 собраны в
[отчёте автоматизации](results/windows/2026-10-22-automation-checks.txt).
Debian clipboard/auto: **45 PASS, 0 FAIL**. Новый Windows ZIP с исправлениями
прошёл чистую Windows без toolchain: **500 циклов, ≥180 секунд, 0 FAIL**;
[отчёт пакета](results/windows/2026-10-22-vm-windows11-clean-package.txt).
Нативные Windows-тесты: **401 PASS**, clippy без предупреждений.
Расширенный release-прогон при 4 GiB: [**33 PASS, 0 FAIL**](results/windows/2026-10-22-vm-windows11-s3-7.txt),
включая три S3-пробуждения, Notepad, UIPI и уничтожение дерева владельца.
[Отчёт исправлений и расширенного release-прогона](results/windows/2026-10-22-fixes.txt)
фиксирует точный объём и оставшиеся ограничения. Эти результаты сами по себе
не подтверждают готовность обеих платформ к выпуску.

## Команды и границы

| Область | Автоматическая проверка | Свидетельство / ограничение |
| --- | --- | --- |
| Движок: remap, swallow, chord, tap/hold/double tap, стеки слоёв, timeout, конфликты модификаторов | `cargo test --locked -p lhc-core`; `mapper_e2e -- --ignored` | Unit проверяет семантику; evdev→uinput и VM проверяют реальную доставку. Один слой в VM не означает всю матрицу backend. |
| Макросы, pause, отмена, обновление конфигурации, команды и запрет команд | Workspace tests, UI examples, именованные Linux/Windows VM шаги | Полный каталог действий требует отдельных fixtures для каждого заявленного backend. Недоступный backend — не PASS. |
| Системные раскладки, условия, auto-layout и game mode | Core tests + VM `system-actions`, `russian-layout`, `game-mode`, Windows mapper scenarios | Steam/реестр/D-Bus, whitelist/blacklist, смена источников требуют воспроизводимых fixtures; реальные игры дополнительно оцениваются в ручной пользовательской сессии. |
| Точный Unicode, раскладка US/RU, отмена при смене фокуса | Linux `text-unicode`; Windows `mapper-unicode-us-ru`, `mapper-focus-cancel`; popup BMP/surrogate/ZWJ | Корпус и набор получателей должны быть указаны в отчёте. Общее название теста не означает все приложения и IME. |
| Clipboard backend | Linux `--text-backend clipboard`: `clipboard-preservation` | Нужны уже имеющиеся в KDE-стенде `wl-copy`/`wl-paste`. Проверяются отказ/согласие portal для paste shortcut, выдача Unicode и восстановление прежнего текста clipboard. Отдельные тесты MIME/смены владельца описаны в GNOME-стенде; это не физическая ручная проверка. |
| Popup, фокус, theme/locale, редактор и сохранение | UI examples `editor -- --smoke`, `interactions`, page examples; Windows popup/lifecycle | Метрики, попадание кликов, повторное открытие, точность вставки автоматизируются. Человеку остаётся оценка читаемости и визуального качества. |
| Linux lifecycle: autostart, права, lock, S3, replug, stop/start, авария | `run-linux.py --vm … --package …` | Основные KDE VM: 42/0; расширенный Debian clipboard/auto: 45/0, см. индекс. Физические драйверы и сон оборудования этим не принимаются. |
| Windows lock/unlock и S3 | `run-windows.py --suite release --extended` | Тест вызывает настоящий lock и требует QMP S3 + неизменное время загрузки. Не считается пройденным до отчёта PASS. |
| Авария владельца, guard и дерева владельца | Windows `mapper-crash-held`, `mapper-guard-failure`, `mapper-process-tree-crash` | Проверка дерева владельца выполняется до корректирующего ввода. Ошибка — автоматический дефект, не ручной пункт. |
| Повторная нагрузка и ресурсы | Linux/Windows `--stress-seconds 180` или длительнее; ZIP `-StressCycles`/`-StressSeconds` | Сохраняются точные выходные данные, память, FD/handles, threads. Три минуты — короткая нагрузка, а не многосуточный soak. |
| Чистая Windows без toolchain | `run-windows-package.py --vm … --package … --runtime-installer …` | VM до установки Rust/VS, без cache-диска; ISO/QMP bootstrap; подписанный Microsoft VC runtime при необходимости; ZIP/lifecycle/ресурсы/удаление распаковки. |
| Обновление Linux между версиями | `run-linux.py --previous OLD --package NEW` | Нужны настоящие разные выпущенные версии текущего формата. Без OLD проверка не выполняется; для первого публичного релиза previous-release отсутствует по определению. |
| Обновление Windows portable | `check-windows-upgrade.ps1 -PreviousArchive OLD -Archive NEW` | Проверяет суммы, разные версии, запуск, сохранность профиля и удаление portable-файлов. Две копии той же версии отклоняются. |
| Комплект распространения | `check-release-artifacts.py --directory … --version … --require-vendor` | Суммы, обязательные артефакты, LICENSE/README/installer, ELF/PE x64, offline vendor. Локально `--compare-linux` сверяет бинарники tar/DEB/RPM через имеющийся bsdtar. |
| CI/release pipeline | `.github/workflows/ci.yml`, `.github/workflows/release.yml` | Desktop e2e остаются локальными. `verify-artifacts` выполняется и при workflow_dispatch; перед draft обязателен. Локальный actionlint не заменяет выполненный hosted workflow. |

## Примеры запуска

```sh
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/kde-vm --package /path/to/package.deb --text-backend clipboard --stress-seconds 180
python3 apps/slint-shell/scripts/vm/run-windows.py --vm /path/to/windows-vm --suite mapper --extended --stress-seconds 180
python3 apps/slint-shell/scripts/vm/run-windows-package.py --vm /path/to/clean-windows-vm --package /path/to/package.zip --runtime-installer /path/to/vc_redist.x64.exe
python3 apps/slint-shell/scripts/release/check-release-artifacts.py --directory /path/to/artifacts --version 0.3.0 --require-vendor
python3 -m unittest discover -s apps/slint-shell/scripts/vm -p 'test_*.py'
```

Чистый Windows base берётся из снимка **до** toolchain; удаление PATH или
отключение cache-диска у подготовленного developer base не доказывает чистую ОС.
`run-windows-package.py` требует `xorriso`, ISO виден как D: без cache-диска;
гость должен входить на рабочий стол тестового пользователя. Installer VC runtime
проверяется по Authenticode Microsoft; UAC подтверждается QMP только в disposable VM.

## Оставшиеся автоматические проверки и ограничения

- Настоящий previous-release upgrade: на момент подготовки этого набора GitHub
  releases пуст, локальных release tags нет. Не создавать искусственный «старый
  релиз» из той же сборки. Для первого публичного выпуска это N/A; перед заявлением обновления существующей версии повторить оба сценария с настоящими артефактами.
- UAC/повышенный получатель: добавлен fixture `mapper-elevated-uipi` с реальным RunAs и проверкой
  уровня привилегий receiver, отсутствия маркера в повышенном/постороннем окне
  и восстановления обычного ввода. Выполнен PASS в Windows 11 VM (дважды); это не принимает другие повышенные приложения. Отдельная попытка mapper-ввода
  в сам UAC secure desktop этим сценарием не проверяется.
- `mapper-process-tree-crash`: guard вынесен за живое дерево владельца; целевой
  прогон завершения дерева прошёл. Явное уничтожение одновременно владельца и
  независимого guard остаётся ограничением, без обещания восстановления.
  [Отчёт исправлений](results/windows/2026-10-22-fixes.txt) отделяет debug от release.
- Notepad с чтением через UI Automation прошёл целевой прогон точного Unicode,
  включая длинный ввод >15 секунд; см. [отчёт исправлений](results/windows/2026-10-22-fixes.txt).
  Расширение до браузера, терминала и IDE и полный IME-корпус: нужны
  наблюдаемые fixtures и проверка реального текста/фокуса, а не только SendInput.
- Все дополнительные text/popup backend, полный каталог системных действий и
  заявленные дополнительные desktop окружения принимаются отдельными прогонами.
- Многосуточная нагрузка с теми же assertions и сохранением временного ряда
  ресурсов; короткий прогон не подменяет её.
- Hosted release workflow на конкретной ревизии, подпись/attestation и проверка
  именно его артефактов. Публикация релиза остаётся отдельным действием владельца.

Текущий запуск и его FAIL/SKIP/ограничения записываются в results/;
[ручной чек-лист](manual-acceptance.md) не служит запасным списком для этих задач.

При больших `--stress-seconds` в Windows увеличить общий `--timeout` (минуты)
так, чтобы он покрывал запуск, нагрузку и сбор логов; гостевой срок ожидания
учитывает длительность нагрузки плюс час. Для ISO package runner `--timeout`
задаётся в секундах. Короткий выполненный прогон не считать многосуточным soak.
Исходный BasicDisplay Windows base не предоставляет S3. Отдельный стенд с подписанным
VirtIO GPU DOD при 4 GiB прошёл 3 S3/wakeup с работающим mapper и вводом после каждого.
[Целевой debug-повтор при 6 GiB](results/windows/2026-10-23-vm-windows11-s3.txt)
также прошёл 3 цикла (2026-10-23, 6 PASS / 0 FAIL с учётом подготовки).
Ранее 6 GiB зависал на втором пробуждении; причина того сбоя не установлена,
поэтому один повтор не доказывает длительную стабильность. Release-прогон принят
при 4 GiB; целевой debug PASS не расширяет его объём до 6 GiB.
`powercfg /a`, QMP events, boot time и ограничения сохраняются; hibernation не считается S3.

## Строгий профиль VM-приёмки

Обычный запуск остаётся диагностическим: нулевой exit code подтверждает только
выполненные шаги. `--acceptance` дополнительно требует PASS всех обязательных
сценариев. SKIP, BLOCKED и NOT RUN обязательного шага дают ненулевой exit code;
сводка явно пишет `Acceptance: INCOMPLETE`. Непройденные предусловия блокируют
зависимый сценарий, а независимые проверки могут продолжаться. При ошибке Linux
сохраняет overlay для разбора, как Windows.

Одна ячейка KDE:

```sh
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/kde-vm --package /path/to/current.deb --previous /path/to/previous.deb --popup-mode spell --text-backend clipboard --stress-seconds 180 --acceptance
```

Профиль требует suspend без исключения в `vm.json`, нагрузку минимум 180 секунд,
переустановку поверх работающего приложения и обновление с предыдущей версии.
Только для первого публичного выпуска вместо `--previous` явно передать
`--first-release`: обновление будет записано как N/A, а не PASS.

Полная обязательная Linux-матрица состоит из 12 ячеек: два разных KDE-стенда
(DEB и RPM), `winit`/`auto`/`spell`, `libei`/`clipboard`. Каждый запуск создаёт
новый overlay, поэтому consent и состояние предыдущей ячейки не переиспользуются.
Контроллер выполняет ячейки последовательно и требует успеха каждой:

```sh
python3 apps/slint-shell/scripts/vm/run-linux-matrix.py --deb-vm /path/to/debian-kde --deb-package /path/to/current.deb --rpm-vm /path/to/fedora-kde --rpm-package /path/to/current.rpm --previous-deb /path/to/previous.deb --previous-rpm /path/to/previous.rpm
```

`--list` печатает точные команды без запуска и без отчёта о приёмке. Для первого
выпуска обе опции previous заменяются одной `--first-release`. Это длительная
предрелизная проверка; во время разработки достаточно затронутой ячейки.
Матрица принимает Linux desktop в указанном объёме, не публикацию релиза.

Строгая Windows-приёмка developer VM:

```sh
python3 apps/slint-shell/scripts/vm/run-windows.py --vm /path/to/windows-s3-vm --suite full --extended --stress-seconds 180 --previous-package /path/to/previous.zip --acceptance
```

Рядом с предыдущим ZIP нужен его `WINDOWS-SHA256SUMS`. Upgrade выполняется в
интерактивной сессии против нового ZIP, собранного полным набором; одинаковые
версии отклоняются. Для первого выпуска явно указать `--first-release`.
Диагностические фильтры и smoke/release без полного набора не могут получить
строгий PASS. Проверка через настоящий executable охватывает профиль → mapper →
WinForms receiver, сохранение game mode через штатную команду/Document и
повторный ввод после перезапуска того же профиля. Специальный core stand остаётся
для расширенной матрицы backend. Это не проверка редактирования правил мышью.

Чистый Windows ZIP по-прежнему проверяется отдельно командой
`run-windows-package.py`: developer VM не заменяет чистую ОС. Аналогично зелёная
Linux-матрица не подтверждает Windows, GNOME, физические клавиатуры или hosted
release workflow. Отчёты новых профилей до их фактического запуска не считаются
свидетельством приёмки.

Linux receiver передаёт атомарный JSON-результат с exit status диалога и точным
текстом. Пустая подтверждённая строка отличается от таймаута/отмены/отсутствующего
файла/ошибки SSH; из данных удаляется только один завершающий newline протокола
kdialog/zenity. Popup smoke требует новый первый кадр, событие получения фокуса
и скрытие; отдельный сценарий проверяет возврат ввода в receiver после hide.

CI job mapper e2e теперь завершается ошибкой, если `/dev/uinput` недоступен:
неисполненная обязательная проверка не становится зелёной сборкой.

Быстрая отдельная проверка VM receiver без установки приложения:

```sh
python3 apps/slint-shell/scripts/vm/check-linux-receiver.py --vm /path/to/kde-vm
```

Это компонентная проверка инфраструктуры в настоящем desktop: обычный и пустой
подтверждённый ввод, отмена, таймаут и следующий ввод после cleanup. Ошибка SSH не
засчитывается за ожидаемую отмену/таймаут. Она полезна при изменениях fixture и
не заменяет ни mapper e2e, ни приёмку установленного приложения.
