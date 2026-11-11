//! Profiles shipped with the application as templates for new library profiles.
//!
//! Each one is a profile file in `assets/`, in the same format as the user
//! library; `meta.name` supplies its display name and the default name of a
//! new copy, falling back to the file name (without `.json`).

use super::{model::ProfilePreset, profile_file};

#[derive(Debug, PartialEq, Eq)]
pub struct BundledProfile {
    pub name: &'static str,
    text: &'static str,
}

macro_rules! bundled {
    ($($name:literal),* $(,)?) => {
        &[$(BundledProfile {
            name: $name,
            text: include_str!(concat!("../../assets/", $name, ".json")),
        }),*]
    };
}

/// Bundled profiles in the order the library offers them.
pub const BUNDLED: &[BundledProfile] = bundled!["LHC1_by_Ivan_K"];

impl BundledProfile {
    /// Author's display name, or the asset filename when it is absent or blank.
    pub fn display_name(&self) -> String {
        self.profile()
            .ok()
            .and_then(|profile| profile.meta?.name)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.name.to_owned())
    }

    pub fn text(&self) -> &'static str {
        self.text
    }

    pub fn profile(&self) -> Result<ProfilePreset, String> {
        profile_file::parse(self.text)?.ok_or_else(|| format!("{}: empty profile", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::validate_profile_name;

    #[test]
    fn display_name_uses_metadata_or_filename() {
        for (text, expected) in [
            (
                r#"{"version":1,"meta":{"name":"Friendly name"}}"#,
                "Friendly name",
            ),
            (r#"{"version":1}"#, "asset_name"),
            (r#"{"version":1,"meta":{"name":"  "}}"#, "asset_name"),
        ] {
            let bundled = BundledProfile {
                name: "asset_name",
                text,
            };
            assert_eq!(bundled.display_name(), expected);
        }
        assert_eq!(BUNDLED[0].display_name(), "LHC1 by Ivan K");
    }

    #[test]
    fn bundled_profiles_are_current() {
        assert!(!BUNDLED.is_empty());
        for bundled in BUNDLED {
            assert_eq!(
                validate_profile_name(bundled.name).as_deref(),
                Ok(bundled.name)
            );
            let profile = bundled.profile().unwrap();
            assert!(!profile.rules.is_empty(), "{}", bundled.name);
            let reparsed = profile_file::parse(&profile_file::serialize(&profile))
                .unwrap()
                .unwrap();
            assert_eq!(reparsed, profile, "{}", bundled.name);
        }
    }
}
