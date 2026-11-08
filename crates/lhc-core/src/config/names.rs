//! Validation of names users give to layers and library profiles.
//!
//! Names are trimmed and must be unique ignoring case, so two entries never
//! look the same in lists and action pickers.

use super::model::ProfilePreset;
use crate::storage::validate_profile_name;

/// What a name belongs to; selects the wording of the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Layer,
    Profile,
}

/// Why a name cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIssue {
    Empty,
    /// Contains characters a file name cannot hold (profiles only).
    Invalid,
    Taken,
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Trimmed `name` for a layer of `profile`; `except` is the id of the layer
/// being renamed.
pub fn layer_name(
    profile: &ProfilePreset,
    name: &str,
    except: Option<&str>,
) -> Result<String, NameIssue> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameIssue::Empty);
    }
    if profile
        .layers
        .iter()
        .any(|layer| Some(layer.id.as_str()) != except && same(&layer.name, name))
    {
        return Err(NameIssue::Taken);
    }
    Ok(name.to_owned())
}

/// Validate a profile name against a previously loaded library listing.
pub fn profile_name<'a>(
    name: &str,
    except: Option<&str>,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<String, NameIssue> {
    if name.trim().is_empty() {
        return Err(NameIssue::Empty);
    }
    let name = validate_profile_name(name).map_err(|_| NameIssue::Invalid)?;
    if names
        .into_iter()
        .any(|item| except.is_none_or(|except| !same(item, except)) && same(item, &name))
    {
        return Err(NameIssue::Taken);
    }
    Ok(name)
}

/// First available profile name, using a loaded listing rather than disk access.
pub fn unique_profile_name(base: &str, names: &[String]) -> Result<String, NameIssue> {
    let base = profile_name(base, None, std::iter::empty())?;
    let taken: std::collections::HashSet<_> =
        names.iter().map(|name| name.to_lowercase()).collect();
    let mut name = base.clone();
    let mut suffix = 2;
    while taken.contains(&name.to_lowercase()) {
        let ending = format!(" ({suffix})");
        let mut stem = base.clone();
        loop {
            name = format!("{stem}{ending}");
            if validate_profile_name(&name).is_ok() {
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

    fn profile() -> ProfilePreset {
        ProfilePreset {
            layers: vec![Layer {
                id: "l_nav".into(),
                name: "Nav".into(),
                description: None,
            }],
            ..ProfilePreset::default()
        }
    }

    #[test]
    fn library_snapshot_checks_names_without_filesystem_access() {
        let names = vec!["Work".to_owned(), "Work (2)".to_owned()];
        assert_eq!(
            profile_name(" work ", None, names.iter().map(String::as_str)),
            Err(NameIssue::Taken)
        );
        assert_eq!(
            profile_name(" work ", Some("WORK"), names.iter().map(String::as_str)),
            Ok("work".into())
        );
        assert_eq!(unique_profile_name("Work", &names), Ok("Work (3)".into()));
        assert_eq!(
            unique_profile_name("bad/name", &names),
            Err(NameIssue::Invalid)
        );
    }

    #[test]
    fn layer_names_are_trimmed_and_unique_ignoring_case() {
        let profile = profile();
        assert_eq!(layer_name(&profile, "  Num ", None), Ok("Num".into()));
        assert_eq!(layer_name(&profile, "   ", None), Err(NameIssue::Empty));
        assert_eq!(layer_name(&profile, "nav", None), Err(NameIssue::Taken));
        assert_eq!(layer_name(&profile, "NAV", Some("l_nav")), Ok("NAV".into()));
    }
}
