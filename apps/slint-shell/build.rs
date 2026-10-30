#[path = "build-support/i18n.rs"]
mod i18n_checks;
#[path = "build-support/rust_messages.rs"]
mod rust_messages;

fn main() {
    // The Slint compiler recurses deeply on this UI tree; the default 1 MiB
    // main-thread stack on Windows overflows (STATUS_STACK_OVERFLOW).
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(build)
        .unwrap()
        .join()
        .unwrap();
}

fn build() {
    println!("cargo:rerun-if-changed=translations");
    println!("cargo:rerun-if-changed=src/i18n.rs");
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=../../crates/lhc-core/src/mapper/builtin_macros.rs");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let languages = [
        ("Russian", "ru"),
        ("German", "de"),
        ("French", "fr"),
        ("Spanish", "es"),
        ("Italian", "it"),
        ("Portuguese", "pt"),
        ("Polish", "pl"),
        ("Turkish", "tr"),
        ("Ukrainian", "uk"),
        ("Czech", "cs"),
        ("Hungarian", "hu"),
        ("Romanian", "ro"),
        ("Greek", "el"),
        ("Croatian", "hr"),
        ("Georgian", "ka"),
        ("Kazakh", "kk"),
        ("ChineseSimplified", "zh-Hans"),
        ("ChineseTraditional", "zh-Hant"),
        ("Japanese", "ja"),
        ("Korean", "ko"),
    ];
    let catalogs: Vec<_> = languages
        .iter()
        .map(|&(name, code)| {
            let path = format!("translations/{code}/LC_MESSAGES/slint-shell.po");
            i18n_checks::validate_po_wrapping(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|error| panic!("i18n {code}: {error}"));
            let catalog = rspolib::pofile(path.as_str()).unwrap();
            validate_translations(&catalog, code);
            (name, catalog)
        })
        .collect();
    let mut generated = String::from("enum RustText {\n");
    for (name, _) in rust_messages::MESSAGES {
        generated.push_str(&format!("    {name},\n"));
    }
    generated.push_str("}\nimpl RustText { fn text(self, language: Language) -> &'static str { match (self, language) {\n");
    for (name, source) in rust_messages::MESSAGES {
        generated.push_str(&format!(
            "(Self::{name}, Language::English) => {source:?},\n"
        ));
        for (language, catalog) in &catalogs {
            let entry = catalog
                .entries
                .iter()
                .find(|entry| entry.msgctxt.is_none() && entry.msgid == *source)
                .unwrap();
            let translated = entry.msgstr.as_ref().unwrap();
            generated.push_str(&format!(
                "(Self::{name}, Language::{language}) => {translated:?},\n"
            ));
        }
    }
    generated.push_str("} } }\nfn catalog(language: Language) -> &'static [CatalogEntry] { match language {\nLanguage::English => &[],\n");
    for (language, catalog) in &catalogs {
        generated.push_str(&format!("Language::{language} => &[\n"));
        let mut entries: Vec<_> = catalog
            .entries
            .iter()
            .filter(|entry| !entry.obsolete)
            .collect();
        entries.sort_by_key(|entry| {
            (
                entry.msgctxt.as_deref().unwrap_or_default(),
                entry.msgid.as_str(),
            )
        });
        for entry in entries {
            let translations = if entry.msgid_plural.is_some() {
                entry.msgstr_plural.clone()
            } else {
                entry.msgstr.clone().into_iter().collect()
            };
            generated.push_str(&format!(
                "({:?}, {:?}, {:?}, &{:?}),\n",
                entry.msgctxt.as_deref().unwrap_or_default(),
                entry.msgid,
                entry.msgid_plural,
                translations
            ));
        }
        generated.push_str("],\n");
    }
    generated.push_str("} }\n");
    std::fs::write(output.join("rust_translations.rs"), generated).unwrap();
    let config = slint_build::CompilerConfiguration::new()
        .with_include_paths(vec!["ui/widgets".into(), "ui/widgets/lhc".into()])
        .with_style("lhc".into())
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
    // Live Preview generates an interpreter wrapper without the bundled
    // translation tables; the compiler switches on the same variable.
    if std::env::var("SLINT_LIVE_PREVIEW").is_ok() {
        return;
    }
    let generated_ui = std::fs::read_to_string(output.join("app.rs")).unwrap();
    i18n_checks::validate_bundled(&generated_ui)
        .unwrap_or_else(|error| panic!("i18n bundled parity: {error}"));
}

fn validate_translations(catalog: &rspolib::POFile, code: &str) {
    assert_eq!(
        catalog.metadata.get("Language").map(String::as_str),
        Some(code)
    );
    assert!(!catalog.metadata_is_fuzzy, "fuzzy catalog header");
    let entries: Vec<_> = catalog
        .entries
        .iter()
        .filter(|entry| !entry.obsolete)
        .map(|entry| i18n_checks::Entry {
            context: entry.msgctxt.clone().unwrap_or_default(),
            source: entry.msgid.clone(),
            plural: entry.msgid_plural.clone(),
            translations: if entry.msgid_plural.is_some() {
                entry.msgstr_plural.clone()
            } else {
                entry.msgstr.clone().into_iter().collect()
            },
            fuzzy: entry.fuzzy(),
        })
        .collect();
    let mut sources = Vec::new();
    collect_slint(std::path::Path::new("ui"), &mut sources);
    let borrowed: Vec<_> = sources
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let rust = std::fs::read_to_string("src/i18n.rs").unwrap();
    let ids = rust
        .split_once("let (id, arg, count) = match self {")
        .unwrap()
        .1
        .split_once("\n        Message {")
        .unwrap()
        .0;
    let locale = std::fs::read_to_string("ui/i18n.slint").unwrap();
    let plural_count = catalog.metadata["Plural-Forms"]
        .split_once("nplurals=")
        .unwrap()
        .1
        .split(';')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    i18n_checks::validate_catalog(
        &entries,
        &borrowed,
        rust_messages::MESSAGES,
        ids,
        &locale,
        plural_count,
        code == "ru",
    )
    .unwrap_or_else(|error| panic!("i18n parity: {error}"));
    let builtins =
        std::fs::read_to_string("../../crates/lhc-core/src/mapper/builtin_macros.rs").unwrap();
    i18n_checks::validate_builtin_macros(&builtins, &locale, &entries)
        .unwrap_or_else(|error| panic!("i18n built-in parity: {error}"));
}

fn collect_slint(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_slint(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "slint") {
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(path).unwrap(),
            ));
        }
    }
}
