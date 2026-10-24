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

Keep message contexts and placeholders unchanged. Each catalog declares its own
`Plural-Forms`; the external runtime translator's corresponding rules are in
`src/i18n.rs`. Build-time checks require complete catalogs, matching placeholders,
all Rust and Slint messages, and all plural forms. Words legitimately shared with
English, such as French “Description”, may remain identical to the source.

When adding a message, update every catalog. The build generates both bundled
Slint translations and the catalog used by tray and runtime messages from these
PO files. No additional translation service is used at runtime.
