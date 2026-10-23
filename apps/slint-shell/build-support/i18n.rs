//! Strict catalog checks shared by the build script and regression tests.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub context: String,
    pub source: String,
    pub plural: Option<String>,
    pub translations: Vec<String>,
    pub fuzzy: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    String(String),
    Word(String),
    Symbol(char),
}

// Tokenize strings and comments rather than searching raw text: translator
// context and multiline @tr calls must have the same coverage as simple calls.
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let mut chars = text.chars().peekable();
    let mut out = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut depth = 1;
                while let Some(c) = chars.next() {
                    if c == '/' && chars.peek() == Some(&'*') {
                        chars.next();
                        depth += 1;
                    } else if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                if depth != 0 {
                    return Err("unterminated comment".into());
                }
            }
            '"' => {
                let mut value = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            closed = true;
                            break;
                        }
                        '\\' => match chars.next() {
                            Some('n') => value.push('\n'),
                            Some('t') => value.push('\t'),
                            Some('r') => value.push('\r'),
                            Some('"') => value.push('"'),
                            Some('\\') => value.push('\\'),
                            Some('u') if chars.next() == Some('{') => {
                                let hex: String =
                                    chars.by_ref().take_while(|c| *c != '}').collect();
                                let code =
                                    u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                                value.push(char::from_u32(code).ok_or("invalid Unicode escape")?);
                            }
                            _ => return Err("unsupported string escape".into()),
                        },
                        c => value.push(c),
                    }
                }
                if !closed {
                    return Err("unterminated string".into());
                }
                out.push(Token::String(value));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut word = String::from(c);
                while chars
                    .peek()
                    .is_some_and(|c| c.is_alphanumeric() || *c == '_')
                {
                    word.push(chars.next().ok_or("missing character")?);
                }
                out.push(Token::Word(word));
            }
            c if c.is_whitespace() => {}
            c => out.push(Token::Symbol(c)),
        }
    }
    Ok(out)
}

fn placeholders(text: &str) -> Result<BTreeMap<String, usize>, String> {
    let mut out = BTreeMap::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if (c == '{' || c == '}') && chars.peek() == Some(&c) {
            chars.next();
        } else if c == '{' {
            let mut name = String::new();
            let mut closed = false;
            for c in chars.by_ref() {
                if c == '}' {
                    closed = true;
                    break;
                }
                if c == '{' {
                    return Err(format!("invalid placeholder in {text:?}"));
                }
                name.push(c);
            }
            if !closed {
                return Err(format!("unclosed placeholder in {text:?}"));
            }
            *out.entry(name).or_default() += 1;
        } else if c == '}' {
            return Err(format!("unmatched brace in {text:?}"));
        }
    }
    Ok(out)
}

pub fn validate(
    entries: &[Entry],
    slint: &[(&str, &str)],
    rust_messages: &[(&str, &str)],
    rust_ids: &str,
    locale: &str,
) -> Result<(), String> {
    let mut catalog = BTreeMap::new();
    for entry in entries {
        let key = (entry.context.as_str(), entry.source.as_str());
        if catalog.insert(key, entry).is_some() {
            return Err(format!("duplicate translation: {key:?}"));
        }
        if entry.fuzzy {
            return Err(format!("fuzzy translation: {key:?}"));
        }
        let count = if entry.plural.is_some() { 3 } else { 1 };
        if entry.translations.len() != count
            || entry.translations.iter().any(|s| s.trim().is_empty())
        {
            return Err(format!("empty or incomplete translation: {key:?}"));
        }
        let expected = placeholders(&entry.source)?;
        if let Some(plural) = &entry.plural
            && placeholders(plural)? != expected
        {
            return Err(format!("plural source placeholders differ: {key:?}"));
        }
        for translated in &entry.translations {
            // Product names, key names and executable names intentionally match EN.
            if translated == &entry.source
                && entry.source.chars().any(char::is_alphabetic)
                && !matches!(
                    entry.source.as_str(),
                    "Tab · {}/{}"
                        | "Left Hand Control"
                        | "F1–F24"
                        | "ID"
                        | "E-ink"
                        | "ydotool"
                        | "xdotool"
                )
            {
                return Err(format!("untranslated source text: {key:?}"));
            }
            if placeholders(translated)? != expected {
                return Err(format!("translation placeholders differ: {key:?}"));
            }
        }
    }
    for (file, text) in slint {
        let ts = tokens(text)?;
        for window in ts.windows(3) {
            if let [Token::Word(property), Token::Symbol(':'), Token::String(value)] = window
                && matches!(property.as_str(), "text" | "label" | "title")
                && value.chars().filter(|c| c.is_alphabetic()).count() > 1
                // Font-metric probe and the literal action syntax shown as data.
                && !matches!(value.as_str(), "Ag" | "text:")
            {
                return Err(format!("{file}: literal UI text outside @tr: {value:?}"));
            }
        }
        for (i, window) in ts.windows(3).enumerate() {
            if window
                != [
                    Token::Symbol('@'),
                    Token::Word("tr".into()),
                    Token::Symbol('('),
                ]
            {
                continue;
            }
            let mut at = i + 3;
            let Some(Token::String(first)) = ts.get(at) else {
                return Err(format!("{file}: @tr requires a literal"));
            };
            at += 1;
            let (context, source) = if ts.get(at) == Some(&Token::Symbol('='))
                && ts.get(at + 1) == Some(&Token::Symbol('>'))
            {
                at += 2;
                let Some(Token::String(source)) = ts.get(at) else {
                    return Err(format!("{file}: missing contextual source"));
                };
                at += 1;
                (first.as_str(), source.as_str())
            } else {
                ("", first.as_str())
            };
            let entry = catalog
                .get(&(context, source))
                .ok_or_else(|| format!("{file}: missing translation: {context:?} {source:?}"))?;
            let plural = if ts.get(at) == Some(&Token::Symbol('|')) {
                let Some(Token::String(plural)) = ts.get(at + 1) else {
                    return Err(format!("{file}: missing plural source"));
                };
                Some(plural.as_str())
            } else {
                None
            };
            if entry.plural.as_deref() != plural {
                return Err(format!("{file}: plural declaration differs: {source:?}"));
            }
        }
    }
    for (_, source) in rust_messages {
        let entry = catalog
            .get(&("", *source))
            .ok_or_else(|| format!("missing Rust translation: {source:?}"))?;
        if entry.plural.is_some() {
            return Err(format!("Rust message must be singular: {source:?}"));
        }
    }
    let handlers: BTreeSet<_> = tokens(locale)?
        .windows(6)
        .filter_map(|w| match w {
            [
                Token::Word(object),
                Token::Symbol('.'),
                Token::Word(property),
                Token::Symbol('='),
                Token::Symbol('='),
                Token::String(id),
            ] if object == "m" && property == "id" => Some(id.clone()),
            _ => None,
        })
        .collect();
    for token in tokens(rust_ids)? {
        if let Token::String(id) = token
            && !handlers.contains(&id)
        {
            return Err(format!("missing Locale.text handler: {id:?}"));
        }
    }
    Ok(())
}

/// Core metadata must retain a translatable label for every built-in macro.
pub fn validate_builtin_macros(core: &str, locale: &str, entries: &[Entry]) -> Result<(), String> {
    let args: BTreeSet<_> = tokens(locale)?
        .windows(6)
        .filter_map(|w| match w {
            [
                Token::Word(object),
                Token::Symbol('.'),
                Token::Word(property),
                Token::Symbol('='),
                Token::Symbol('='),
                Token::String(id),
            ] if object == "m" && property == "arg" => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut id = None;
    let mut count = 0;
    for window in tokens(core)?.windows(3) {
        if let [Token::Word(field), Token::Symbol(':'), Token::String(value)] = window {
            if field == "id" {
                id = Some(value.clone());
            } else if field == "name"
                && let Some(id) = id.take()
            {
                count += 1;
                if !args.contains(&id) {
                    return Err(format!("missing built-in macro handler: {id}"));
                }
                if !entries
                    .iter()
                    .any(|entry| entry.context.is_empty() && entry.source == *value)
                {
                    return Err(format!("missing built-in macro translation: {value}"));
                }
            }
        }
    }
    if count == 0 {
        return Err("no built-in macro metadata found".into());
    }
    Ok(())
}

/// Imported std-widgets can add strings that are absent from our UI files.
/// Slint is pinned, so require its generated translation tables to be complete too.
pub fn validate_bundled(rust: &str) -> Result<(), String> {
    for name in [
        "_SLINT_TRANSLATED_STRINGS",
        "_SLINT_TRANSLATED_STRINGS_PLURALS",
    ] {
        let marker = format!("const {name} :");
        let start = rust
            .find(&marker)
            .ok_or_else(|| format!("missing generated Slint table: {name}"))?;
        let mut source = String::new();
        for token in tokens(&rust[start..])? {
            match token {
                Token::Symbol(';') => break,
                Token::String(value) => source = value,
                Token::Word(value) if value == "None" => {
                    return Err(format!("untranslated bundled Slint string near {source:?}"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Entry {
        Entry {
            source: "Hello {}".into(),
            translations: vec!["Привет {}".into()],
            ..Default::default()
        }
    }

    #[test]
    fn catalog_rejects_duplicates_empty_fuzzy_and_broken_placeholders() {
        let check = |entries: &[Entry]| validate(entries, &[], &[], "", "");
        assert!(check(&[entry()]).is_ok());
        assert!(
            check(&[Entry {
                translations: vec!["Hello {}".into()],
                ..entry()
            }])
            .unwrap_err()
            .contains("untranslated")
        );
        assert!(
            check(&[entry(), entry()])
                .unwrap_err()
                .contains("duplicate")
        );
        for translations in [
            vec![],
            vec![String::new()],
            vec!["Привет".into()],
            vec!["Привет {n}".into()],
        ] {
            assert!(
                check(&[Entry {
                    translations,
                    ..entry()
                }])
                .is_err()
            );
        }
        assert!(
            check(&[Entry {
                fuzzy: true,
                ..entry()
            }])
            .unwrap_err()
            .contains("fuzzy")
        );
    }

    #[test]
    fn catalog_requires_all_three_russian_plural_forms() {
        let plural = Entry {
            source: "{n} rule".into(),
            plural: Some("{n} rules".into()),
            translations: vec![
                "{n} правило".into(),
                "{n} правила".into(),
                "{n} правил".into(),
            ],
            ..Default::default()
        };
        let source = [("test.slint", "@tr(\"{n} rule\" | \"{n} rules\" % count)")];
        assert!(validate(std::slice::from_ref(&plural), &source, &[], "", "").is_ok());
        for translations in [
            vec!["{n} правило".into()],
            vec!["{n} правило".into(), "{n} правила".into(), "".into()],
        ] {
            assert!(
                validate(
                    &[Entry {
                        translations,
                        ..plural.clone()
                    }],
                    &source,
                    &[],
                    "",
                    ""
                )
                .is_err()
            );
        }
        assert!(
            validate(
                &[plural],
                &[("test.slint", "@tr(\"{n} rule\" | \"wrong\" % count)")],
                &[],
                "",
                ""
            )
            .is_err()
        );
    }

    #[test]
    fn coverage_checks_context_multiline_calls_comments_and_escapes() {
        let entries = [Entry {
            context: "Greeting".into(),
            ..entry()
        }];
        let source = [(
            "test.slint",
            "// @tr(\"ignored\")\n/* @tr(\"ignored too\") */\n@tr(\n \"Greeting\" => \"Hello {}\", name)",
        )];
        assert!(validate(&entries, &source, &[], "", "").is_ok());
        for source in [
            "@tr(\"Hello {}\", name)",
            "@tr(\"Other\" => \"Hello {}\", name)",
            "@tr(value)",
        ] {
            assert!(validate(&entries, &[("test.slint", source)], &[], "", "").is_err());
        }
        assert_eq!(
            tokens("\"a\\n\\\"b\\\"\"").unwrap(),
            vec![Token::String("a\n\"b\"".into())]
        );
    }

    #[test]
    fn every_builtin_macro_needs_a_translated_handler() {
        let core = "BuiltinMacro { id: \"sample\", name: \"Hello {}\" }";
        let locale = "if m.id == \"builtin-macro\" && m.arg == \"sample\" {}";
        assert!(validate_builtin_macros(core, locale, &[entry()]).is_ok());
        assert!(validate_builtin_macros(core, "", &[entry()]).is_err());
        assert!(validate_builtin_macros(core, locale, &[]).is_err());
        assert!(validate_builtin_macros("", locale, &[entry()]).is_err());
    }

    #[test]
    fn imported_widget_translations_cannot_fall_back() {
        let complete = "const _SLINT_TRANSLATED_STRINGS : X = [[Some(\"Select All\"), Some(\"Выделить всё\")]]; const _SLINT_TRANSLATED_STRINGS_PLURALS : X = [];";
        assert!(validate_bundled(complete).is_ok());
        assert!(
            validate_bundled(&complete.replace("Some(\"Выделить всё\")", "None"))
                .unwrap_err()
                .contains("Select All")
        );
        assert!(validate_bundled("").is_err());
    }

    #[test]
    fn literal_ui_labels_must_go_through_i18n() {
        for property in [
            "text",
            "title",
            "label",
            "accessible-label",
            "placeholder-text",
        ] {
            let text = format!("Text {{ {property}: \"Untranslated text\"; }}");
            assert!(
                validate(&[], &[("test.slint", &text)], &[], "", "")
                    .unwrap_err()
                    .contains("outside @tr")
            );
        }
        assert!(validate(&[], &[("test.slint", "Text { text: \"✓\"; }")], &[], "", "").is_ok());
    }

    #[test]
    fn coverage_checks_rust_text_and_message_handlers() {
        assert!(
            validate(
                &[entry()],
                &[],
                &[("Hello", "Hello {}")],
                "\"error\"",
                "if m.id == \"error\" {}"
            )
            .is_ok()
        );
        assert!(
            validate(&[], &[], &[("Hello", "Hello {}")], "", "")
                .unwrap_err()
                .contains("Rust translation")
        );
        assert!(
            validate(
                &[],
                &[],
                &[],
                "\"action-failed\"",
                "if m.id == \"error\" {}"
            )
            .unwrap_err()
            .contains("handler")
        );
    }
}
