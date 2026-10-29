//! Message-language preferences from the process locale environment.

/// Ordered message-language candidates, following GNU gettext's LANGUAGE
/// override except when the effective locale is C or POSIX.
pub fn message_languages() -> Vec<String> {
    from_environment(|name| std::env::var(name).ok())
}

fn from_environment(mut get: impl FnMut(&str) -> Option<String>) -> Vec<String> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(&mut get)
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "C".into());
    if is_untranslated_locale(&locale) {
        return vec!["en".into()];
    }
    let mut languages: Vec<_> = get("LANGUAGE")
        .unwrap_or_default()
        .split(':')
        .filter(|value| !value.is_empty())
        .map(|value| {
            if is_untranslated_locale(value) {
                "en".into()
            } else {
                value.to_owned()
            }
        })
        .collect();
    if languages.is_empty() {
        languages.push(locale);
    }
    languages
}

fn is_untranslated_locale(locale: &str) -> bool {
    matches!(locale.split(['.', '@']).next(), Some("C" | "POSIX"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages(values: &[(&str, &str)]) -> Vec<String> {
        from_environment(|name| {
            values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).into())
        })
    }

    #[test]
    fn language_list_precedes_effective_locale() {
        assert_eq!(
            languages(&[("LANG", "en_US.UTF-8"), ("LANGUAGE", "xx::ru:en")]),
            ["xx", "ru", "en"]
        );
        assert_eq!(
            languages(&[("LANG", "ru_RU"), ("LANGUAGE", "de:C:ru")]),
            ["de", "en", "ru"]
        );
    }

    #[test]
    fn locale_categories_keep_their_precedence() {
        assert_eq!(
            languages(&[
                ("LC_ALL", "de_DE"),
                ("LC_MESSAGES", "ru_RU"),
                ("LANG", "en_US")
            ]),
            ["de_DE"]
        );
        assert_eq!(
            languages(&[
                ("LC_ALL", ""),
                ("LC_MESSAGES", "ru_RU"),
                ("LANG", "en_US"),
                ("LANGUAGE", "")
            ]),
            ["ru_RU"]
        );
    }

    #[test]
    fn c_and_posix_locales_disable_language_override() {
        for locale in ["C", "C.UTF-8", "POSIX"] {
            assert_eq!(languages(&[("LANG", locale), ("LANGUAGE", "ru")]), ["en"]);
        }
        assert_eq!(languages(&[]), ["en"]);
    }
}
