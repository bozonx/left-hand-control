fn main() {
    use rspolib::AsBytes;

    println!("cargo:rerun-if-changed=translations");
    let mut russian = rspolib::pofile("translations/ru/LC_MESSAGES/slint-shell.po").unwrap();
    if let Some(forms) = russian.metadata.get_mut("Plural-Forms") {
        *forms = forms
            .split(';')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(";");
    }
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("ru.mo"), russian.as_bytes()).unwrap();
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).unwrap();
}
