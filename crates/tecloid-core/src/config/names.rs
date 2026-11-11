//! Validation of names users give to layers and library keymaps.
//!
//! Names are trimmed and must be unique ignoring case, so two entries never
//! look the same in lists and action pickers.

use super::model::KeymapPreset;
use crate::storage::validate_keymap_name;

/// What a name belongs to; selects the wording of the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Layer,
    Keymap,
}

/// Why a name cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIssue {
    Empty,
    /// Contains characters a file name cannot hold (keymaps only).
    Invalid,
    Taken,
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Trimmed `name` for a layer of `keymap`; `except` is the id of the layer
/// being renamed.
pub fn layer_name(
    keymap: &KeymapPreset,
    name: &str,
    except: Option<&str>,
) -> Result<String, NameIssue> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameIssue::Empty);
    }
    if keymap
        .layers
        .iter()
        .any(|layer| Some(layer.id.as_str()) != except && same(&layer.name, name))
    {
        return Err(NameIssue::Taken);
    }
    Ok(name.to_owned())
}

/// Validate a keymap name against a previously loaded library listing.
pub fn keymap_name<'a>(
    name: &str,
    except: Option<&str>,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<String, NameIssue> {
    if name.trim().is_empty() {
        return Err(NameIssue::Empty);
    }
    let name = validate_keymap_name(name).map_err(|_| NameIssue::Invalid)?;
    if names
        .into_iter()
        .any(|item| except.is_none_or(|except| !same(item, except)) && same(item, &name))
    {
        return Err(NameIssue::Taken);
    }
    Ok(name)
}

/// First available keymap name, using a loaded listing rather than disk access.
pub fn unique_keymap_name(base: &str, names: &[String]) -> Result<String, NameIssue> {
    let base = keymap_name(base, None, std::iter::empty())?;
    let taken: std::collections::HashSet<_> =
        names.iter().map(|name| name.to_lowercase()).collect();
    let mut name = base.clone();
    let mut suffix = 2;
    while taken.contains(&name.to_lowercase()) {
        let ending = format!(" ({suffix})");
        let mut stem = base.clone();
        loop {
            name = format!("{stem}{ending}");
            if validate_keymap_name(&name).is_ok() {
                break;
            }
            stem.pop();
        }
        suffix += 1;
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::Layer;

    fn keymap() -> KeymapPreset {
        KeymapPreset {
            layers: vec![Layer {
                id: "l_nav".into(),
                name: "Nav".into(),
                description: None,
            }],
            ..KeymapPreset::default()
        }
    }

    #[test]
    fn library_snapshot_checks_names_without_filesystem_access() {
        let names = vec!["Work".to_owned(), "Work (2)".to_owned()];
        assert_eq!(
            keymap_name(" work ", None, names.iter().map(String::as_str)),
            Err(NameIssue::Taken)
        );
        assert_eq!(
            keymap_name(" work ", Some("WORK"), names.iter().map(String::as_str)),
            Ok("work".into())
        );
        assert_eq!(unique_keymap_name("Work", &names), Ok("Work (3)".into()));
        assert_eq!(
            unique_keymap_name("bad/name", &names),
            Err(NameIssue::Invalid)
        );
    }

    #[test]
    fn layer_names_are_trimmed_and_unique_ignoring_case() {
        let keymap = keymap();
        assert_eq!(layer_name(&keymap, "  Num ", None), Ok("Num".into()));
        assert_eq!(layer_name(&keymap, "   ", None), Err(NameIssue::Empty));
        assert_eq!(layer_name(&keymap, "nav", None), Err(NameIssue::Taken));
        assert_eq!(layer_name(&keymap, "NAV", Some("l_nav")), Ok("NAV".into()));
    }
}
