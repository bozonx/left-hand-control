# План подготовки к релизу и e2e-тестированию

Статус: рабочий план, октябрь 2026. Обновляйте его по мере выполнения этапов.

## 1. Рамки первого релиза

| Что              | Решение                                                                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------- |
| Платформы        | Linux x86\_64. ARM (aarch64) и Windows (x64/arm64) — следующими этапами, macOS — не планируется         |
| Где публикуем    | GitHub Releases. Репозитории дистрибутивов — по этапам из раздела 3                                     |
| AUR              | Отложен: регистрация новых аккаунтов сейчас закрыта                                                     |
| Целевая среда    | KDE Plasma 6 на Wayland; остальные среды работают в объёме, описанном в `docs/development/platforms.md` |
| Desktop-сценарии | Только локально на машине разработчика и в локальных VM, не в CI                                        |

## 2. Уже сделано

- Бинарник называется `left-hand-control` (`[[bin]]` в `apps/slint-shell/Cargo.toml`).
  Cargo-пакет остаётся `slint-shell`, поэтому команды `cargo … -p slint-shell` не меняются.
- На Linux окно получает Wayland/X11 app id `dev.bozonx.left-hand-control`
  (`slint::set_xdg_app_id`), поэтому KDE сопоставляет окно с `.desktop`-файлом и иконкой.
- Иконка: исходник `apps/slint-shell/assets/icon.svg`; PNG 16–512 в
  `apps/slint-shell/packaging/linux/icons/` и иконка трея `assets/icon.png` (64 px)
  генерируются скриптом `apps/slint-shell/scripts/render-icons.sh`.
- В `apps/slint-shell/packaging/linux/` лежат общие для всех форматов файлы:
  - `dev.bozonx.left-hand-control.desktop` — запись меню (EN/RU);
  - `dev.bozonx.left-hand-control.metainfo.xml` — AppStream, проходит
    `appstreamcli validate --pedantic`; секцию `<releases>` заполняем при выпуске;
  - `60-left-hand-control.rules` — запись в `/dev/uinput` для активной локальной
    сессии (`uaccess`) и для группы `input`;
  - `modules-load.conf` — автозагрузка `uinput`.
- Архив `package-linux.sh` содержит все эти файлы. `install-linux.sh` ставит иконки,
  metainfo и `.desktop`. Новые команды `permissions` и `remove-permissions` явно
  (через `sudo`) ставят и снимают udev-правило, автозагрузку модуля и группу `input`.
  Подсказки в UI об отсутствии прав ссылаются на эту команду.

## 3. Куда публиковать

Мы не можем ставить udev-правило и выдавать доступ к `/dev/input` из песочницы, поэтому
**Flathub и Snap не подходят**: Flatpak не даёт доступа к uinput без `--device=all`,
Flathub такое не примет; strict-Snap так же закрыт, а classic требует ручного одобрения.

| Канал | Что даёт | Стоимость поддержки | Когда |
| --- | --- | --- | --- |
| **GitHub Releases** | `.tar.gz`, `.deb`, `.rpm`, `SHA256SUMS`, attestation | Низкая: всё собирается одним workflow | Первый релиз |
| **Fedora COPR** | `dnf copr enable …` для Fedora (KDE Spin — основная Plasma 6 аудитория на RPM) | Низкая: SRPM с vendored crates, в Fedora свежий Rust | Сразу после первого релиза |
| **Свой APT/RPM-репозиторий** на GitHub Pages (или Cloudsmith, бесплатный тариф для OSS) | Обновления через `apt`/`dnf`/`zypper` из тех же `.deb`/`.rpm`, без пересборки | Средняя: ключ подписи репозитория, `reprepro`/`createrepo_c` в workflow | Когда появятся пользователи Debian/Ubuntu/openSUSE |
| **openSUSE Build Service (OBS)** | Одна точка сборки для openSUSE, Fedora, Debian, Ubuntu, Arch | Средняя/высокая: офлайн-сборка, нужен Rust ≥ 1.92 в целевом дистрибутиве, а в старых Debian/Ubuntu его нет | Альтернатива своему репозиторию, если нужны нативные сборки |
| **nixpkgs** | NixOS и Nix | Средняя: PR и ревью, `buildRustPackage` с vendored-патчами | Когда стабилизируется формат |
| **AUR** (`left-hand-control`, `left-hand-control-bin`) | Arch, Manjaro, EndeavourOS | Низкая | Когда откроют регистрацию |
| Ubuntu PPA | Ubuntu | Высокая: Launchpad собирает без сети, версии rustc ограничены | Не планируем |

Рекомендованный порядок: GitHub Releases → COPR → свой подписанный APT/RPM-репозиторий
(или OBS) → AUR → nixpkgs.

Для любых сборок в дистрибутивах без сети выкладываем в релиз архив
`left-hand-control-<ver>-vendor.tar.xz` (`cargo vendor --locked`). Он же закрывает
требование GPL о предоставлении исходников вместе с vendor-патчами.

## 4. Артефакты GitHub-релиза (Linux x86_64)

```
left-hand-control-<ver>-linux-x86_64.tar.gz      # универсальный, ставится в ~/.local
left-hand-control_<ver>_amd64.deb                # Debian 12+/Ubuntu 22.04+
left-hand-control-<ver>-1.x86_64.rpm             # Fedora, openSUSE
left-hand-control-<ver>-vendor.tar.xz            # исходники зависимостей
SHA256SUMS
```

### 4.1. Базовая система сборки

Бинарник зависит от glibc, libstdc++, libxkbcommon, freetype и fontconfig.
Wayland загружается через dlopen. Собираем в контейнере **Ubuntu 22.04 (glibc 2.35)**:
так один бинарник подходит всем актуальным дистрибутивам с Plasma 6 (Debian 13,
Fedora 41+, openSUSE Tumbleweed, Kubuntu 25.04+, Arch). Rust ставится через
`rustup` до версии из `rust-version`; системный rustc образа не используем.

### 4.2. deb и rpm

Собираем **nfpm** (внешний инструмент сборки, не зависимость Cargo) из одного
`apps/slint-shell/packaging/linux/nfpm.yaml` и того же бинарника, что в tar.gz.

| Файл в пакете | Источник |
| --- | --- |
| `/usr/bin/left-hand-control` | release-бинарник |
| `/usr/share/applications/dev.bozonx.left-hand-control.desktop` | `packaging/linux/` |
| `/usr/share/metainfo/dev.bozonx.left-hand-control.metainfo.xml` | `packaging/linux/` |
| `/usr/share/icons/hicolor/*/apps/dev.bozonx.left-hand-control.{png,svg}` | `packaging/linux/icons/`, `assets/icon.svg` |
| `/usr/lib/udev/rules.d/60-left-hand-control.rules` | `packaging/linux/` |
| `/usr/lib/modules-load.d/left-hand-control.conf` | `packaging/linux/modules-load.conf` |
| `/usr/share/licenses/left-hand-control/LICENSE` (rpm), `/usr/share/doc/left-hand-control/copyright` (deb) | `LICENSE` |

- Зависимости: deb — `libc6 (>= 2.35)`, `libstdc++6`, `libxkbcommon0`, `libfontconfig1`,
  `libfreetype6`, `libwayland-client0`; rpm — аналоги по sonames. Recommends:
  `xdg-desktop-portal`, `wl-clipboard`, emoji-шрифт Noto Color Emoji.
- `postinst`: `udevadm control --reload-rules`, `udevadm trigger` для `uinput`,
  `modprobe uinput || true`. **Не добавляет** пользователя в группу `input`:
  пакет только печатает подсказку.
- `postrm`: перезагрузка udev-правил. Конфигурацию пользователя не трогаем.
- Проверки в CI: `lintian` для deb, `rpmlint` для rpm, `desktop-file-validate`,
  `appstreamcli validate`.

### 4.3. AppImage — позже

Отложен, пока не решены два вопроса:
- автозапуск пишет `current_exe()` (`crates/lhc-core/src/autostart.rs`), а внутри
  AppImage это временный путь `/tmp/.mount_*`; нужно брать `$APPIMAGE`;
- AppImage не может установить udev-правило: пользователю нужна отдельная
  инструкция или встроенная команда `permissions`.

## 5. Workflow релиза

Новый файл `.github/workflows/release.yml`, запуск по тегу `v*` и вручную.

1. **verify**: те же проверки, что в `ci.yml` (fmt, clippy `-D warnings`, тесты,
   сборка без Spell).
2. **metadata**: тег совпадает с `workspace.package.version`; в `CHANGELOG.md`
   есть секция версии; в metainfo есть `<release version="…" date="…">`.
3. **build-linux** (контейнер `ubuntu:22.04`): `package-linux.sh` → tar.gz;
   nfpm → deb и rpm; `cargo vendor` → vendor-архив; проверка `ldd` на `not found`;
   lintian/rpmlint; `SHA256SUMS`.
4. **smoke-packages** (матрица контейнеров): `debian:12`, `debian:13`,
   `ubuntu:22.04`, `ubuntu:24.04`, `fedora:latest`, `opensuse/tumbleweed`,
   `archlinux` (последний — tar.gz). Установить пакет, проверить, что все
   библиотеки находятся, файлы на месте, `.desktop` и metainfo валидны, бинарник
   запускается в неграфическом режиме (нужен `--version`, см. раздел 8), удаление
   чистое.
5. **attest**: `actions/attest-build-provenance` для всех артефактов.
6. **draft-release**: черновик релиза с артефактами и заметками из `CHANGELOG.md`.
   **Публикует человек** после локального e2e (раздел 7).

Права workflow: `contents: write` только у job публикации, `id-token: write` и
`attestations: write` только у attest. Секреты для COPR и своего репозитория
добавятся на соответствующих этапах.

## 6. Чек-лист выпуска

1. Обновить `CHANGELOG.md` (перенести `Unreleased` в версию с датой).
2. Поднять `workspace.package.version` в `Cargo.toml`, `cargo update -w`.
3. Добавить `<release>` в metainfo.
4. Прогнать локальный e2e (раздел 7.3) на текущем `main`; при изменениях упаковки
   или прав — ещё VM-матрицу (раздел 7.4). Результат записать в
   `docs/testing/results/linux/<дата>.txt`.
5. Поставить тег `vX.Y.Z`, дождаться черновика релиза.
6. Скачать артефакты из черновика, проверить `sha256sum -c SHA256SUMS` и
   установку `.deb`/`.rpm` хотя бы в одной VM.
7. Опубликовать релиз; затем COPR и другие каналы, когда они подключены.

## 7. E2E-тестирование

### 7.1. Уровни

| Уровень | Что проверяет | Где | Когда |
| --- | --- | --- | --- |
| L0. Unit/integration | Ядро, конфиг, движок, IPC, i18n | GitHub CI | Каждый push/PR |
| L1. Headless e2e маппера | Реальный путь evdev → engine → uinput без рабочего стола | GitHub CI (если доступен `uinput`) и локально | Каждый PR |
| L2. Smoke пакетов | Зависимости, файлы, установка и удаление | GitHub CI, контейнеры | Тег релиза |
| L3. Desktop-сценарии | UI, попапы, трей, восстановление worker, настройки | Машина разработчика (KDE Wayland) | Перед релизом и после изменений UI |
| L4. VM-матрица | Установка пакета с нуля, права, сессия, автозапуск, обновление | Локальные KVM/QEMU VM | Перед релизом, при изменениях упаковки и прав |
| L5. Ручная приёмка | Полный список из `docs/testing/cross-platform-product-validation-plan.md` | Реальное железо | Мажорные релизы |

Полные desktop-e2e (L3–L4) в GitHub CI **не запускаем**:
- на облачных раннерах нет KDE Plasma Wayland сессии, трея и portal;
- VM внутри раннера без KVM слишком медленная и нестабильная;
- self-hosted раннер в публичном репозитории опасен: PR из форка может выполнить
  код на машине разработчика. Существующий `e2e-desktop.yml` остаётся только
  с ручным запуском.

### 7.2. L1: headless e2e маппера

Это самый ценный автоматический тест: проверяет продукт, а не оболочку.

- Тест в `lhc-core` (рядом с уже существующим `#[ignore]` тестом в
  `crates/lhc-core/src/mapper/linux/io.rs`): создаёт виртуальную «физическую»
  клавиатуру через uinput, запускает маппер с тестовым профилем на этом
  устройстве, читает выход виртуального устройства маппера через evdev и сравнивает
  последовательность событий.
- Сценарии: passthrough, remap `KeyA → KeyB`, chord, tap/hold, слой, double tap,
  макрос с паузой, остановка без залипших клавиш, отключение устройства во время работы.
- Включение: `cargo test -p lhc-core -- --ignored mapper_e2e` или переменная окружения;
  в обычном `cargo test` тест пропускается.
- В CI: `sudo modprobe uinput && sudo chmod 0666 /dev/uinput` перед тестом.
  Сначала проверить, что на `ubuntu-latest` модуль доступен. Если нет — L1 остаётся
  локальным уровнем.
- `text:` через portal здесь не проверяется — это L3/L4.

### 7.3. L3: локальный прогон на машине разработчика

Один скрипт `apps/slint-shell/scripts/e2e-local.sh`, который в графической сессии
по очереди запускает уже существующие сценарии с временным конфигом и печатает сводку:

```sh
cargo run --locked -p slint-shell --features spell --example editor -- --smoke
cargo run --locked -p slint-shell --features spell --example interactions
cargo run --locked -p slint-shell --features spell --example library
# action-picker, macros, menus, undo-redo, inline-edit, drag-scroll, notifications
python3 apps/slint-shell/scripts/check-recovery.py --popups spell
python3 apps/slint-shell/scripts/check-preferences.py
python3 apps/slint-shell/scripts/check-worker-crash.py
python3 apps/slint-shell/scripts/check-linux-install.py
cargo test -p lhc-core -- --ignored mapper_e2e          # L1
```

Затем — короткий ручной smoke из раздела 3 плана кроссплатформенной проверки на
установленном архиве (не на `cargo run`). Результат: файл в `docs/testing/results/linux/`
с датой, коммитом, версией Plasma и списком пройденных шагов.

### 7.4. L4: VM-матрица

Гипервизор — KVM/QEMU, как в `docs/testing/windows.md`. Образы лежат вне репозитория
(например `/mnt/disk2/vm/lhc-<distro>/`).

| VM | Зачем |
| --- | --- |
| Fedora KDE (Plasma 6, Wayland) | rpm, SELinux, основная RPM-аудитория |
| Debian 13 KDE | deb, базовый glibc, консервативные пакеты |
| Kubuntu (актуальный релиз с Plasma 6) | deb, Ubuntu-специфика |
| openSUSE Tumbleweed KDE | rpm через zypper, другие имена пакетов |
| Arch KDE | tar.gz и будущий AUR |
| Fedora Workstation (GNOME) | fallback на winit, сообщения о недоступных функциях |

Подготовка образа (один раз): установка, пользователь с автологином SDDM в Plasma
Wayland, ssh-ключ, `qemu-guest-agent`, без установленного приложения и без прав
на `/dev/input`. Затем — базовый `base.qcow2`, который больше не меняется.

Прогон (`scripts/vm/run-e2e.py --vm fedora-kde --package <файл>`):
1. Создать overlay `run.qcow2` поверх `base.qcow2`; после прогона удалить —
   каждый запуск с чистого состояния.
2. Загрузить VM с QMP-сокетом и `usb-kbd` (события доходят до гостя как
   настоящее evdev-устройство), дождаться ssh.
3. Скопировать пакет, установить штатным менеджером (`dnf`/`apt`/`zypper`) или
   `install-linux.sh`.
4. Проверить: запись меню и иконка видны (`.desktop`, hicolor), metainfo
   находится `appstreamcli`, udev-правило применилось, `/dev/uinput` доступен сессии.
5. Без группы `input`: запуск показывает подсказку о правах. Затем выдать права,
   перелогиниться (перезапуск SDDM-сессии), запустить приложение.
6. Подать клавиши через QMP `input-send-event`, проверить вывод маппера
   (через evdev-лог виртуального устройства и окно-приёмник для `text:`).
7. Проверить трей, попапы (`SLINT_SHELL_POPUPS=auto`), автозапуск после перезагрузки,
   обновление поверх запущенной версии с сохранением конфига, удаление.
8. Собрать логи, `screendump` через QMP и сводку в каталог результатов.

aarch64-VM на x86-хосте работают только в эмуляции (TCG) и очень медленные — добавим их
вместе с ARM-сборками, только для smoke.

## 8. Открытые вопросы (нужно решение до релиза)

Эти пункты меняют CLI, IPC или поведение, поэтому по AGENTS.md требуют согласования:

1. **`--version` / `--help`**: нужны для smoke пакетов и для пользователей. Сейчас
   любой аргумент считается IPC-командой.
2. **Повторный запуск из меню**, когда приложение уже в трее: сейчас процесс
   завершается с ошибкой `left-hand-control is already running`. Для пользователя
   ожидаемо, что откроются настройки (`show settings` через IPC).
3. **Переменные окружения** `SLINT_SHELL_*` (`SOCKET`, `POPUPS`, `METRICS` …)
   видны пользователям релиза. Переименовать в `LHC_*` до первого релиза или оставить.
4. **Выдача прав из приложения**: кнопка в подсказке о правах, запускающая
   `pkexec` с polkit-политикой. Для пользователей пакетов это удобнее, чем
   `install-linux.sh permissions`, которого в deb/rpm нет.
5. **Идентификатор трея** `lhc-slint-shell` — внутренний; KDE запоминает по нему
   настройки видимости. Можно оставить.

## 9. Этапы

| # | Этап | Критерий готовности |
| --- | --- | --- |
| 1 | Решения по разделу 8 и их реализация | Пункты 1–3 реализованы или явно отклонены |
| 2 | `nfpm.yaml` и локальная сборка deb/rpm | Пакеты ставятся и удаляются в контейнерах Debian и Fedora, lintian/rpmlint без ошибок |
| 3 | `release.yml` (раздел 5) | Тег в форке создаёт черновик релиза со всеми артефактами |
| 4 | L1 headless e2e маппера | Тест проходит локально; в CI, если доступен `uinput` |
| 5 | `e2e-local.sh` (L3) | Один запуск даёт сводку и файл результатов |
| 6 | VM-стенд (L4) для Fedora KDE и Debian 13 KDE, затем остальные | `run-e2e.py` проходит сценарий раздела 7.4 без ручных шагов |
| 7 | Первый релиз `v0.x` на GitHub | Чек-лист раздела 6 выполнен |
| 8 | COPR, затем свой APT/RPM-репозиторий или OBS | Установка через `dnf`/`apt` в VM |
| 9 | ARM (aarch64 Linux), затем Windows x64/arm64 | Отдельные планы: нативные раннеры `ubuntu-24.04-arm`, `windows-11-arm`; для Windows arm64 проверить готовые сборки Skia |
