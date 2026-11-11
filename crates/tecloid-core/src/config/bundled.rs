//! Keymaps shipped with the application as templates for new library keymaps.
//!
//! Each one is a keymap file in `assets/`, in the same format as the user
//! library; `meta.name` supplies its display name and the default name of a
//! new copy, falling back to the file name (without `.json`).

use super::{keymap_file, model::KeymapPreset};

#[derive(Debug, PartialEq, Eq)]
pub struct BundledKeymap {
    pub name: &'static str,
    text: &'static str,
}

macro_rules! bundled {
    ($($name:literal),* $(,)?) => {
        &[$(BundledKeymap {
            name: $name,
            text: include_str!(concat!("../../assets/", $name, ".json")),
        }),*]
    };
}

/// Bundled keymaps in the order the library offers them.
pub const BUNDLED: &[BundledKeymap] = bundled!["LHC1_by_Ivan_K"];

impl BundledKeymap {
    /// Author's display name, or the asset filename when it is absent or blank.
    pub fn display_name(&self) -> String {
        self.keymap()
            .ok()
            .and_then(|keymap| keymap.meta?.name)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.name.to_owned())
    }

    pub fn text(&self) -> &'static str {
        self.text
    }

    pub fn keymap(&self) -> Result<KeymapPreset, String> {
        keymap_file::parse(self.text)?.ok_or_else(|| format!("{}: empty keymap", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::validate_keymap_name;

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
            let bundled = BundledKeymap {
                name: "asset_name",
                text,
            };
            assert_eq!(bundled.display_name(), expected);
        }
        assert_eq!(BUNDLED[0].display_name(), "Left Hand Control");
    }

    #[test]
    fn bundled_keymaps_are_current() {
        assert!(!BUNDLED.is_empty());
        for bundled in BUNDLED {
            assert_eq!(
                validate_keymap_name(bundled.name).as_deref(),
                Ok(bundled.name)
            );
            let keymap = bundled.keymap().unwrap();
            assert!(!keymap.rules.is_empty(), "{}", bundled.name);
            let reparsed = keymap_file::parse(&keymap_file::serialize(&keymap))
                .unwrap()
                .unwrap();
            assert_eq!(reparsed, keymap, "{}", bundled.name);
        }
    }
}
