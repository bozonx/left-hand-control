//! Shared key labels and hand grids for the layer editor and held-layer hint.

use crate::{
    keyboard,
    pages::{action_kind, picker::action_label},
    ui::{ActionKind, Assignment, LayerKeyCell, PickerCategory, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions,
        model::{LayerKeymap, LayoutPreset, ResolvedAppConfig},
    },
};
use std::collections::HashMap;

/// Display names of catalog actions (macros, commands, built-ins).
pub(crate) fn action_names(
    ui: &SettingsWindow,
    config: &ResolvedAppConfig,
) -> HashMap<String, String> {
    let mut names: HashMap<String, String> = actions::catalog(config)
        .into_iter()
        .filter_map(|entry| Some((entry.action.format()?, action_label(ui, entry.name)?)))
        .collect();
    crate::pages::picker::DESKTOP_SHORTCUTS.with(|cache| {
        names.extend(
            cache
                .borrow()
                .iter()
                .map(|shortcut| (shortcut.value.clone(), shortcut.label.clone())),
        );
    });
    names
}

/// How a keymap value is shown: `None` no entry, `Some(None)` swallow.
pub(crate) fn assignment(
    value: Option<&Option<String>>,
    names: &HashMap<String, String>,
) -> (Assignment, String, ActionKind, PickerCategory) {
    match value {
        None => (
            Assignment::Transparent,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(None) => (
            Assignment::Swallow,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(Some(action)) if action.is_empty() => (
            Assignment::Transparent,
            String::new(),
            ActionKind::None,
            PickerCategory::None,
        ),
        Some(Some(action)) => {
            let kind = action_kind(action);
            let label = match actions::Action::parse(Some(action)) {
                actions::Action::Text(text) => crate::pages::one_line(&text),
                actions::Action::KdeShortcut(id) => {
                    names.get(action).cloned().unwrap_or_else(|| {
                        actions::kde_shortcut_parts(&id)
                            .map_or(id.clone(), |(_, shortcut)| shortcut.to_owned())
                    })
                }
                _ => names.get(action).cloned().unwrap_or_else(|| action.clone()),
            };
            (
                Assignment::Action,
                label,
                kind,
                crate::pages::picker::category_for_value(action),
            )
        }
    }
}

/// Grid slots of one hand; `first` is the index of its first key and
/// `offset` shifts the last (thumb) row right.
fn hand_cells(
    rows: &[&[&str]],
    columns: usize,
    first: usize,
    offset: usize,
    lookup: &dyn Fn(&str) -> (Assignment, String, ActionKind, PickerCategory),
) -> Vec<LayerKeyCell> {
    let empty = LayerKeyCell {
        index: -1,
        ..Default::default()
    };
    let mut cells = Vec::with_capacity(rows.len() * columns);
    let mut index = first;
    for (row_index, row) in rows.iter().enumerate() {
        let skip = if row_index + 1 == rows.len() {
            offset
        } else {
            0
        };
        cells.extend(std::iter::repeat_n(empty.clone(), skip));
        for code in row.iter() {
            let (kind, action, icon, category) = lookup(code);
            cells.push(LayerKeyCell {
                index: index as i32,
                label: keyboard::label(code).into(),
                code: (*code).into(),
                kind,
                emoji: category == PickerCategory::Text && is_emoji(&action),
                category,
                action: action.into(),
                icon,
                holds: false,
            });
            index += 1;
        }
        cells.extend(std::iter::repeat_n(
            empty.clone(),
            columns - skip - row.len(),
        ));
    }
    cells
}

/// Both hand grids of a layer with `keymap`.
pub(crate) fn hands(
    keymap: Option<&LayerKeymap>,
    names: &HashMap<String, String>,
) -> (Vec<LayerKeyCell>, Vec<LayerKeyCell>) {
    let lookup = |code: &str| assignment(keymap.and_then(|map| map.keys.get(code)), names);
    let left_len: usize = keyboard::LEFT_HAND.iter().map(|row| row.len()).sum();
    let mark_holds = |mut cells: Vec<LayerKeyCell>| {
        if let Some(map) = keymap {
            for cell in &mut cells {
                cell.holds = map.hold_modifiers.contains(cell.code.as_str());
            }
        }
        cells
    };
    (
        mark_holds(hand_cells(
            &keyboard::LEFT_HAND,
            keyboard::LEFT_COLUMNS,
            0,
            2,
            &lookup,
        )),
        mark_holds(hand_cells(
            &keyboard::RIGHT_HAND,
            keyboard::RIGHT_COLUMNS,
            left_len,
            0,
            &lookup,
        )),
    )
}

/// The keys of layer `id` of the running `layout` for the layer hint, or
/// `None` when the layout has no such layer.
pub(crate) fn hint(
    ui: &SettingsWindow,
    config: &ConfigDocument,
    layout: &LayoutPreset,
    id: &str,
) -> Option<crate::layer_hint_model::LayerHint> {
    let layer = layout.layers.iter().find(|layer| layer.id == id)?;
    let names = action_names(
        ui,
        &ResolvedAppConfig::from_parts(config.settings().clone(), layout.clone(), None),
    );
    let (left, right) = hands(layout.layer_keymaps.get(id), &names);
    Some(crate::layer_hint_model::LayerHint::from_cells(
        layer.name.clone(),
        &left,
        &right,
    ))
}

/// Whether `text` consists only of emoji, so the key shows it in the colour
/// emoji font; mixed text keeps the regular font, whose digits stay plain.
fn is_emoji(text: &str) -> bool {
    let pictographic = |c: char| {
        matches!(c as u32,
            0x1F000..=0x1FAFF | 0x2300..=0x23FF | 0x2600..=0x27BF | 0x2B00..=0x2BFF)
    };
    // Joiners, variation selectors, skin tones and tag characters only extend a glyph.
    let modifier = |c: char| {
        matches!(
            c as u32,
            0x200D | 0xFE0E | 0xFE0F | 0x20E3 | 0xE0020..=0xE007F
        )
    };
    let mut chars = text.chars().peekable();
    let mut any = false;
    while let Some(c) = chars.next() {
        // U+FE0F asks for emoji presentation of an otherwise textual symbol, like ℹ️.
        if pictographic(c) || chars.peek() == Some(&'\u{FE0F}') {
            any = true;
        } else if !modifier(c) {
            return false;
        }
    }
    any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_emoji_text_uses_the_emoji_font() {
        assert!(is_emoji("👍"));
        assert!(is_emoji("⚡"));
        assert!(is_emoji("👍🏽"));
        assert!(is_emoji("❤️"));
        assert!(is_emoji("ℹ️"));
        assert!(is_emoji("👨‍👩‍👧"));
        assert!(!is_emoji("≠"));
        assert!(!is_emoji("№"));
        assert!(!is_emoji("ℹ"));
        assert!(!is_emoji("ok 👍"));
        assert!(!is_emoji(""));
    }

    #[test]
    fn assignments_describe_keymap_values() {
        let names = HashMap::from([("macro:copyLine".to_owned(), "Copy line".to_owned())]);
        assert_eq!(assignment(None, &names).0, Assignment::Transparent);
        assert_eq!(assignment(Some(&None), &names).0, Assignment::Swallow);
        assert_eq!(
            assignment(Some(&Some("macro:copyLine".into())), &names),
            (
                Assignment::Action,
                "Copy line".into(),
                ActionKind::Macro,
                PickerCategory::Macros
            )
        );
        assert_eq!(
            assignment(Some(&Some("text:hi".into())), &names),
            (
                Assignment::Action,
                "hi".into(),
                ActionKind::Text,
                PickerCategory::Text
            )
        );
    }

    #[test]
    fn hand_grids_index_every_key() {
        let lookup = |_: &str| {
            (
                Assignment::Transparent,
                String::new(),
                ActionKind::None,
                PickerCategory::None,
            )
        };
        let left = hand_cells(&keyboard::LEFT_HAND, keyboard::LEFT_COLUMNS, 0, 2, &lookup);
        assert_eq!(left.len(), 30);
        let thumb = &left[24..];
        assert_eq!(thumb[0].index, -1);
        assert_eq!(thumb[2].code, "ControlLeft");
        let indexed: Vec<_> = left.iter().filter(|cell| cell.index >= 0).collect();
        assert_eq!(
            indexed.len(),
            keyboard::LEFT_HAND.iter().map(|r| r.len()).sum::<usize>()
        );
        assert_eq!(keyboard::layer_key(indexed[1].index), Some("Digit1"));
    }
}
