# Debian 13 KDE, Wayland

## Стенд и объём

Отдельная KDE Plasma 6 Wayland VM; пакет **deb**. Общие требования,
полный перечень сценариев и локальные UI/mapper-тесты — [KDE Wayland](linux-kde.md).
Подготовка базового образа и `vm.json` — [руководство VM](../development/guide.md#l4-e2e-в-виртуальных-машинах).
VM, SSH-ключ, кэш и пакеты хранить вне Git. Install/remove/reinstall в `vm.json`
должны соответствовать выбранному формату пакета.

Debian 13 проверяет DEB-путь и совместимость с более консервативным окружением. Собирать пакет на совместимой базе по руководству выпуска; локальная сборка на более новой glibc не является равноценной.

## Запуск

После [общих проверок](common.md) и сборки пакета:

```sh
python3 apps/slint-shell/scripts/vm/run-linux.py --vm /path/to/debian13-kde --package /path/to/left-hand-control_VERSION_amd64.deb
```

Для теста перехода между версиями добавить `--previous /path/to/previous-package`.
Без предыдущего пакета не считать этот переход проверенным. `--show` показывает
экран VM, `--keep` сохраняет overlay. Логи находятся в каталоге запуска VM.

## Зафиксированный результат

[Полный прогон DEB на исправленном стенде](results/linux/2026-10-21-vm-debian13-kde.txt):
**42 PASS, 0 FAIL, без SKIP**. Пройдены suspend/resume, блокировка, hotkey без VT,
Unicode после второго экземпляра и USB-replug, stop/start, освобождение клавиш
при stop/kill, переустановка и удаление.

В `vm.json` закреплён `"machine": "pc-i440fx-11.1"`, поле
`suspend_skip_reason` удалено. Базовый образ, ядро Debian 6.12.113, CPU `host`,
четыре vCPU и xHCI сохранены. Пять контрольных S3/resume/replug без приложения
прошли с неизменным boot ID. [Диагностика и точные команды](results/linux/2026-10-21-debian-suspend-fix.txt).

Исторические неудачи сохранены: [34/8 на q35](results/linux/2026-10-21-vm-debian13-kde-3.txt),
[контроль без приложения](results/linux/2026-10-21-debian-suspend-baseline.txt),
[41 PASS с исключением S3](results/linux/2026-10-21-vm-debian13-kde-4.txt) и
[40/2 до исправления синхронизации stop/start](results/linux/2026-10-21-vm-debian13-kde-5.txt).
На q35 повторный S3 не проходит, затем виртуальный watchdog сбрасывает гостя.
Отключение только watchdog не исправляет S3. Принята конфигурация i440fx;
исходный q35-стенд исправленным не считается.

## Что не принято

VM не принимает
физические устройства, Bluetooth, media keys, реальные sleep/wake и hotplug,
все экраны/DPI, сторонние приложения и полный [ручной чек-лист](manual-acceptance.md).
Конкретный отчёт определяет режим popup, portal, права, пропуски и пакет;
результаты другого дистрибутива не заменяют этот прогон.

Сводку оформлять по [политике результатов](results/README.md).
