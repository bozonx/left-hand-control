# Interface translations

English is the source language. The PO catalogs cover Russian (`ru`), German
(`de`), French (`fr`), international Spanish (`es`), Italian (`it`), Portuguese
(`pt`), Polish (`pl`), Turkish (`tr`), Ukrainian (`uk`), Czech (`cs`), Hungarian
(`hu`), Romanian (`ro`), Greek (`el`), Croatian (`hr`), Georgian (`ka`), Kazakh
(`kk`), Simplified Chinese (`zh-Hans`), Traditional Chinese (`zh-Hant`), Japanese
(`ja`) and Korean (`ko`). Language names in the settings selector are autonyms.

The catalogs added in October 2026, other than the existing Russian catalog,
start with machine translations (Google Translate) and corrections to common
interface labels and keyboard mapping terms. They need native-speaker review, particularly
terminology and messages with plural forms. Spanish uses the language code `es`
without a country-specific variant.

Keep message contexts unchanged. Positional arguments may be reordered with
`{0}`, `{1}`, etc., including when the English source uses implicit `{}`
placeholders. Preserve the arguments and their occurrence counts; `{n}` is the
plural count. Translate complete messages with their arguments rather than
concatenating translated prefixes with paths or status values. Do not depend on
leading or trailing spaces in translations.

Multiline translations start with `msgstr ""` (or `msgstr[N] ""`) followed by
quoted continuation lines, as produced by gettext. When replacing a translation,
replace all its continuation lines too; the build rejects continuations after a
nonempty initial `msgstr` to catch accidentally retained old instructions.

Each catalog declares its own
`Plural-Forms`; the external runtime translator's corresponding rules are in
`src/i18n.rs`. Build-time checks require complete catalogs, matching placeholders,
all Rust and Slint messages, and all plural forms. Words legitimately shared with
English, such as French “Description”, may remain identical to the source.

When adding a message, update every catalog. The build generates both bundled
Slint translations and the catalog used by tray and runtime messages from these
PO files. No additional translation service is used at runtime.

Missing runtime keys are logged and display the English source so controls and
errors remain readable. This fallback does not relax the build-time completeness
checks. Automatic language selection respects the effective locale and the
`LANGUAGE` preference list; C/POSIX locales use English.

Terminology used in the reviewed Russian and Spanish interface labels:

| English | Russian | Spanish |
| --- | --- | --- |
| Key (physical keyboard key) | Клавиша | Tecla |
| Rule | Правило | Regla |
| Layer | Слой | Capa |
| Mapper | Маппер | Mapeador |
| Tap action | Действие при нажатии | Acción al pulsar |
| Hold action | Действие при удержании | Acción al mantener pulsado |
| Release a key | Отпустить клавишу | Soltar una tecla |

Review new translations in context, especially capture instructions, tap/hold
behavior and the distinction between a mapper layout and the system keyboard
layout. Automated checks cover structure, not linguistic correctness; the
machine-translated catalogs still require native-speaker review.
