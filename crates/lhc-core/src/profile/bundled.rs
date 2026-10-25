//! Layouts shipped with the application as templates for new library layouts.
//!
//! Each one is a layout file in `assets/`, in the same format as the user
//! library; `meta.name` supplies its display name and the default name of a
//! new copy, falling back to the file name (without `.json`).

use super::{layout_file, model::LayoutPreset};

#[derive(Debug, PartialEq, Eq)]
pub struct BundledLayout {
    pub name: &'static str,
    text: &'static str,
}

macro_rules! bundled {
    ($($name:literal),* $(,)?) => {
        &[$(BundledLayout {
            name: $name,
            text: include_str!(concat!("../../assets/", $name, ".json")),
        }),*]
    };
}

/// Bundled layouts in the order the library offers them.
pub const BUNDLED: &[BundledLayout] = bundled!["LHC1_by_Ivan_K"];

impl BundledLayout {
    /// Author's display name, or the asset filename when it is absent or blank.
    pub fn display_name(&self) -> String {
        self.layout()
            .ok()
            .and_then(|layout| layout.meta?.name)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.name.to_owned())
    }

    pub fn text(&self) -> &'static str {
        self.text
    }

    pub fn layout(&self) -> Result<LayoutPreset, String> {
        layout_file::parse(self.text)?.ok_or_else(|| format!("{}: empty layout", self.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::validate_layout_name;

    #[test]
    fn display_name_uses_metadata_or_filename() {
        for (text, expected) in [
            (
                r#"{"schemaVersion":1,"meta":{"name":"Friendly name"}}"#,
                "Friendly name",
            ),
            (r#"{"schemaVersion":1}"#, "asset_name"),
            (r#"{"schemaVersion":1,"meta":{"name":"  "}}"#, "asset_name"),
        ] {
            let bundled = BundledLayout {
                name: "asset_name",
                text,
            };
            assert_eq!(bundled.display_name(), expected);
        }
        assert_eq!(BUNDLED[0].display_name(), "LHC1 by Ivan K");
    }

    #[test]
    fn bundled_layouts_are_current() {
        assert!(!BUNDLED.is_empty());
        for bundled in BUNDLED {
            assert_eq!(
                validate_layout_name(bundled.name).as_deref(),
                Ok(bundled.name)
            );
            let layout = bundled.layout().unwrap();
            assert!(!layout.rules.is_empty(), "{}", bundled.name);
            let reparsed = layout_file::parse(&layout_file::serialize(&layout))
                .unwrap()
                .unwrap();
            assert_eq!(reparsed, layout, "{}", bundled.name);
        }
    }
}
