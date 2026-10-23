//! Layouts shipped with the application as templates for new library layouts.
//!
//! Each one is a layout file in `assets/`, in the same format as the user
//! library; its file name (without `.json`) is its name and the default
//! name of a layout created from it.

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
