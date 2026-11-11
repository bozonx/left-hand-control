# Проверка тестов 2026-11-04

Обязательные автоматические проверки завершены. Набор проверенных исходников совпадает с текущими файлами; финальный HEAD: `7097502d` с незакоммиченными исправлениями.

## Исправления

- Windows: атомарное сохранение настроек/layout повторяет rename до 500 мс при временных ошибках доступа/совместного использования. Постоянный отказ сохраняет исходный файл. Добавлены два native-теста с настоящей блокировкой файла.
- Windows-стенд: физическая F отпускается сразу после ответа stop; порядок Ctrl-up перед F-up проверяется по событиям. Чтение snapshot допускает ограниченный разрыв публикации, остальные ошибки остаются ошибками.
- Upgrade-стенд выбирает ZIP текущей версии; создаёт реальный config командой `game on`, исключает только сменяемый IPC-token из проверки постоянных данных и сохраняет диагностическую копию config.
- GNOME-стенд регистрирует XDG autostart-entry перед проверкой нового входа. Обвязка native clipboard-теста очищает тестовый service и повторно создаёт seed перед независимыми проверками приложения.
- Исправлены только локальные fixture-настройки диска Windows и координаты Deny для Kubuntu. Базовые образы не изменялись.

## Финальные host-проверки

Все команды ниже завершились с кодом 0. Cargo запускался с `CARGO_BUILD_JOBS=1`; параллелизм тестов не увеличивался.

| Проверка | Результат |
| --- | --- |
| fmt | passed |
| clippy, все targets, `-D warnings` | passed |
| workspace со Spell | 803 passed, 0 failed, 11 ignored |
| все targets без Spell | passed |
| cargo deny | advisories/bans/licenses/sources passed |
| тесты VM-runner | 49 passed |
| Rust 1.92 MSRV, core + shell со Spell | passed |
| локальные UI + mapper e2e | 20 passed, 0 failed |

```sh
cargo fmt -p tecloid-core -p slint-shell --check
python3 -m unittest discover -s apps/slint-shell/scripts/vm -p 'test_*.py'
cargo clippy --locked -p tecloid-core -p slint-shell --features slint-shell/spell --all-targets --no-deps -- -D warnings
cargo test --locked --workspace --features slint-shell/spell
cargo check --locked -p slint-shell --all-targets
/home/ivan/.cargo/bin/cargo-deny check
```

Все 11 ignored-тестов выполнены отдельно: 8 mapper e2e через evdev/uinput, изолированный clipboard, synthetic input timestamps, native GNOME portal. UI/e2e выполнялись в отдельном KWin; clipboard пользователя хоста не использовался.

## KDE Wayland: строгая L4-матрица

Каждая клетка завершилась `Acceptance: COMPLETE`, без исключений suspend и с нагрузкой минимум 180 секунд. Для winit сбор отсутствующего Spell CSV является неприменимым сбором артефакта, а не пропущенным функциональным тестом.

| Пакет / VM | winit + libei | winit + clipboard | auto + libei | auto + clipboard | spell + libei | spell + clipboard |
| --- | --- | --- | --- | --- | --- | --- |
| Debian 13 KDE / DEB | [53 passed](linux/2026-11-04-vm-debian13-kde.txt) | [54 passed](linux/2026-11-04-vm-debian13-kde-2.txt) | [53 passed](linux/2026-11-03-vm-debian13-kde.txt) | [54 passed](linux/2026-11-04-vm-debian13-kde-3.txt) | [53 passed](linux/2026-11-04-vm-debian13-kde-4.txt) | [54 passed](linux/2026-11-04-vm-debian13-kde-5.txt) |
| Fedora 44 KDE / RPM | [53 passed](linux/2026-11-04-vm-fedora44-kde.txt) | [54 passed](linux/2026-11-04-vm-fedora44-kde-2.txt) | [53 passed](linux/2026-11-04-vm-fedora44-kde-3.txt) | [54 passed](linux/2026-11-04-vm-fedora44-kde-4.txt) | [53 passed](linux/2026-11-04-vm-fedora44-kde-5.txt) | [54 passed](linux/2026-11-04-vm-fedora44-kde-6.txt) |

Kubuntu 26.04 KDE: auto/libei — [53 passed](linux/2026-11-04-vm-kubuntu2604-2.txt); auto/clipboard — [54 passed](linux/2026-11-04-vm-kubuntu2604-3.txt).

GNOME 50.1 Wayland: [32 passed](linux/2026-11-04-vm-ubuntu2604-gnome-4.txt), включая native portal Unicode, text/binary MIME, новый clipboard-owner, отмену paste, tray-host recovery и autostart.

## Windows

- Native/full: 460 core + 186 shell + 5 IPC в smoke; завершающий native-набор — суммарно 666 passed (включая отдельный release storage run), clippy и portable package passed. Числа относятся к запускам команд, а не сумме уникальных тестов.
- Полный run завершил функциональные mapper-сценарии и native/package-проверки; исходный итоговый exit 1 сохранён как история ошибки upgrade-фикстуры. Исправленный upgrade отдельно успешно выполнен на том же ZIP: `windows-upgrade-recovery-2.log`.
- S3-покрытие составное: baseline и lock/unlock из `windows-s3-extended-1.log`; три S3-цикла, stress, Unicode, Notepad и UIPI из `windows-s3-extended-2.log`; повтор UIPI и завершение дерева процессов из `windows-s3-extended-3.log`. Это не единый строгий Windows `--acceptance`.
- [Чистая VM](windows/2026-11-04-vm-windows11-clean-package.txt): prerequisite VC runtime, CLI/lifecycle, 500 rendered popup cycles за минимум 180 секунд, отсутствие Rust и исходников — passed.
- ZIP SHA256: `2bfd072c4e76bb5b9b63949cbff028edb3dc21f1784d57966ad91461aa7f06be`.

**Ограничение upgrade:** config 0.3.0 имеет старый формат без `schemaVersion`. Проверена сохранность его байтов после замены/удаления portable-программы; автоматическая миграция этого неподдерживаемого формата не утверждается и не добавлялась.

## Пакеты и артефакты

Установка/удаление Linux-пакетов прошли в Debian 12/13, Ubuntu 22.04/24.04, Fedora, openSUSE Tumbleweed и Arch контейнерах. Проверены SHA256, обязательные файлы, ELF/PE architecture, vendor archive и одинаковый Linux-бинарник в tar.gz/DEB/RPM.

Linux-артефакты пересобраны после изменения bundled layout. Последующее исправление storage меняет поведение только Windows; на остальных OS `rename_file` — прямой alias прежнего `std::fs::rename`. Это и изменения только тестовых стендов зафиксированы в `source_amendments`; существующие Linux VM-результаты сохранены, финальные host-проверки повторены на текущих файлах.

## Точные команды VM / packaging

Ниже фактически использованные команды последнего успешного этапа или целевого продолжения. Старые неуспешные отчёты оставлены как история; составное Windows-покрытие описано выше. Локальные обвязки и сырые журналы сохранены в `/mnt/disk2/vm/tecloid-validation-20261103`.

```sh
# msrv-toolchain
docker exec tecloid-validation-build /root/.cargo/bin/rustup toolchain install 1.92.0 --profile minimal
# msrv
docker exec -e PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin -e RUSTUP_TOOLCHAIN=1.92.0 -e CARGO_TARGET_DIR=/src/target/msrv-1.92 tecloid-validation-build cargo check --locked -p tecloid-core -p slint-shell --features slint-shell/spell
# local-e2e-final
/usr/bin/python3 /mnt/disk2/vm/tecloid-validation-20261031/isolated-desktop.py /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/e2e-local.sh
# isolated-clipboard
/usr/bin/python3 /mnt/disk2/vm/tecloid-validation-20261031/isolated-desktop.py cargo test --locked -p tecloid-core --lib text_input::clipboard::tests::isolated_clipboard_preserves_formats_cancellation_and_new_copies -- --ignored
# synthetic-input
cargo test --locked -p tecloid-core --lib mapper::linux::io::tests::synthetic_devices_use_monotonic_timestamps_and_preserve_cross_device_order -- --ignored
# native-core-build
docker exec -e PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin tecloid-validation-build cargo test --release --locked -p tecloid-core --lib --no-run --message-format=json
# kde-deb-auto-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode auto --text-backend libei --stress-seconds 180 --acceptance
# windows-upgrade-recovery
python3 /mnt/disk2/vm/tecloid-validation-20261103/windows-disk.py /mnt/disk2/vm/tecloid-validation-20261103/windows-upgrade-recovery.py
# windows-s3-extended
/usr/bin/python3 /mnt/disk2/vm/tecloid-validation-20261103/windows-disk.py /mnt/disk2/vm/tecloid-validation-20261103/windows-resume.py --vm /mnt/disk2/vm/tecloid-windows-s3 --suite release --extended --stress-seconds 180 --timeout 60
# windows-clean-package
/usr/bin/python3 /mnt/disk2/vm/tecloid-validation-20261103/windows-disk.py /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-windows-package.py --vm /mnt/disk2/vm/tecloid-windows-clean-package --package /mnt/disk2/vm/tecloid-windows/runs/20261103-212003/logs/guest/packages/tecloid-0.4.0-windows-x86_64.zip --runtime-installer /mnt/disk2/vm/tecloid-windows/vc_redist.x64.exe --cycles 500 --stress-seconds 180 --timeout 1800
# gnome-native
/usr/bin/python3 /mnt/disk2/vm/tecloid-validation-20261103/run-gnome-native.py /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release/deps/tecloid_core-ba8da88e3645a2b8 --vm /mnt/disk2/vm/tecloid-ubuntu2604-gnome --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-linux-x86_64.tar.gz
# kubuntu-auto-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-kubuntu2604 --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode auto --text-backend libei --stress-seconds 180 --acceptance
# kubuntu-auto-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-kubuntu2604 --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode auto --text-backend clipboard --stress-seconds 180 --acceptance
# kde-deb-winit-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode winit --text-backend libei --stress-seconds 180 --acceptance
# kde-deb-winit-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode winit --text-backend clipboard --stress-seconds 180 --acceptance
# kde-deb-auto-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode auto --text-backend clipboard --stress-seconds 180 --acceptance
# kde-deb-spell-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode spell --text-backend libei --stress-seconds 180 --acceptance
# kde-deb-spell-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-debian13-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid_0.3.0_amd64.deb --popup-mode spell --text-backend clipboard --stress-seconds 180 --acceptance
# kde-rpm-winit-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode winit --text-backend libei --stress-seconds 180 --acceptance
# kde-rpm-winit-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode winit --text-backend clipboard --stress-seconds 180 --acceptance
# kde-rpm-auto-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode auto --text-backend libei --stress-seconds 180 --acceptance
# kde-rpm-auto-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode auto --text-backend clipboard --stress-seconds 180 --acceptance
# kde-rpm-spell-libei
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode spell --text-backend libei --stress-seconds 180 --acceptance
# kde-rpm-spell-clipboard
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/vm/run-linux.py --vm /mnt/disk2/vm/tecloid-fedora-kde --package /mnt/disk2/codeWorkspace/tecloid/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm --previous /mnt/disk2/vm/tecloid-release-20261021/prior-linux-packages/tecloid-0.3.0-1.x86_64.rpm --popup-mode spell --text-backend clipboard --stress-seconds 180 --acceptance
# package-debian-12
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src debian:12 sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb
# package-debian-13
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src debian:13 sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb
# package-ubuntu-22.04
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src ubuntu:22.04 sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb
# package-ubuntu-24.04
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src ubuntu:24.04 sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid_0.4.0_amd64.deb
# package-fedora-latest
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src fedora:latest sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm
# package-opensuse-tumbleweed-latest
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src opensuse/tumbleweed:latest sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-1.x86_64.rpm
# package-archlinux-latest
docker run --rm -v /mnt/disk2/codeWorkspace/tecloid:/src:ro -w /src archlinux:latest sh apps/slint-shell/scripts/release/smoke-linux-package.sh /src/target/ubuntu-22.04/release-artifacts/tecloid-0.4.0-linux-x86_64.tar.gz
# artifact-integrity
/usr/bin/python3 /mnt/disk2/codeWorkspace/tecloid/apps/slint-shell/scripts/release/check-release-artifacts.py --directory /mnt/disk2/vm/tecloid-validation-20261103/assembled-artifacts --version 0.4.0 --require-vendor --compare-linux
```

Linux release build:

```sh
docker exec -e PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin tecloid-validation-build sh apps/slint-shell/scripts/release/release-linux.sh
```

Native Windows-команды внутри guest:

```powershell
cargo build --locked -p tecloid-core -p slint-shell --bin tecloid --example windows-mapper-stand --example editor --example interactions
cargo test --locked -p tecloid-core -p slint-shell --lib --test ipc
cargo build --release --locked -p tecloid-core -p slint-shell --bin tecloid --example windows-mapper-stand
cargo test --locked -p tecloid-core -p slint-shell --all-targets
cargo clippy --locked -p tecloid-core -p slint-shell --all-targets --no-deps -- -D warnings
cargo test --release --locked -p tecloid-core -p slint-shell --test storage_override
apps/slint-shell/scripts/release/package-windows.ps1
```

## Проверяемость и оставшиеся пределы

- Источник команд, exit codes и журналов: `/mnt/disk2/vm/tecloid-validation-20261103/vm-suite-state.json` и `/mnt/disk2/vm/tecloid-validation-20261103/final-host-state.json`.
- SHA256 манифеста проверенных файлов: `74e01f3373f9b78cbed141c96550e61c15517633419fd12f916697e4f5ef954e`.
- Ошибки доступа Windows проверены с настоящими файловыми блокировками. Основание для учёта share-доступа: [Microsoft — Moving and Replacing Files](https://learn.microsoft.com/en-us/windows/win32/fileio/moving-and-replacing-files).
- Физическое Windows-оборудование, ручная приёмка и macOS не проверялись: соответствующего оборудования/сессии в этом наборе нет. Основной принимаемый desktop — KDE Wayland — проверен в VM.
- Коммиты, теги и публикация релиза не выполнялись; индекс пользователя сохранён.
