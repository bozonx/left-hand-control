//! Validation of names users give to layers and library layouts.
//!
//! Names are trimmed and must be unique ignoring case, so two entries never
//! look the same in lists and action pickers.

use super::model::LayoutPreset;
use crate::{config_document::ConfigDocument, storage::validate_layout_name};

/// What a name belongs to; selects the wording of the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Layer,
    Layout,
}

/// Why a name cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIssue {
    Empty,
    /// Contains characters a file name cannot hold (layouts only).
    Invalid,
    Taken,
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Trimmed `name` for a layer of `layout`; `except` is the id of the layer
/// being renamed.
pub fn layer_name(
    layout: &LayoutPreset,
    name: &str,
    except: Option<&str>,
) -> Result<String, NameIssue> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameIssue::Empty);
    }
    if layout
        .layers
        .iter()
        .any(|layer| Some(layer.id.as_str()) != except && same(&layer.name, name))
    {
        return Err(NameIssue::Taken);
    }
    Ok(name.to_owned())
}

impl ConfigDocument {
    /// Trimmed `name` for a library layout; `except` is the name of the
    /// layout being renamed.
    pub fn layout_name(&self, name: &str, except: Option<&str>) -> Result<String, NameIssue> {
        if name.trim().is_empty() {
            return Err(NameIssue::Empty);
        }
        let name = validate_layout_name(name).map_err(|_| NameIssue::Invalid)?;
        let taken = self.paths().list_user_layouts().is_ok_and(|names| {
            names
                .iter()
                .any(|item| except.is_none_or(|except| !same(item, except)) && same(item, &name))
        });
        if taken {
            return Err(NameIssue::Taken);
        }
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::model::Layer;

    fn layout() -> LayoutPreset {
        LayoutPreset {
            layers: vec![Layer {
                id: "l_nav".into(),
                name: "Nav".into(),
                description: None,
            }],
            ..LayoutPreset::default()
        }
    }

    #[test]
    fn layer_names_are_trimmed_and_unique_ignoring_case() {
        let layout = layout();
        assert_eq!(layer_name(&layout, "  Num ", None), Ok("Num".into()));
        assert_eq!(layer_name(&layout, "   ", None), Err(NameIssue::Empty));
        assert_eq!(layer_name(&layout, "nav", None), Err(NameIssue::Taken));
        assert_eq!(layer_name(&layout, "NAV", Some("l_nav")), Ok("NAV".into()));
    }
}
