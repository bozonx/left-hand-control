//! Edits of layers and their keymaps on a keymap.
//!
//! These operations change the keymap only; validation of action strings
//! and saving belong to [`crate::config_document::ConfigDocument`].

use super::model::{HoldBehavior, KeyRule, KeymapPreset, Layer, LayerKeymap};
use std::{collections::HashMap, sync::LazyLock};

/// Position of every key in the key catalog, for ordering extra keys.
static CATALOG_ORDER: LazyLock<HashMap<&'static str, usize>> = LazyLock::new(|| {
    super::key_catalog::CATEGORIES
        .iter()
        .flat_map(|keys| keys.iter().copied())
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect()
});

impl LayerKeymap {
    /// Extra keys (every key off the layer grid) with their entries in
    /// display order: the saved order first, then the rest in key catalog
    /// order.
    pub fn extra_entries(&self) -> Vec<(String, Option<String>)> {
        let order: HashMap<&str, usize> = self
            .extra_key_order
            .iter()
            .enumerate()
            .map(|(index, key)| (key.as_str(), index))
            .collect();
        let mut entries: Vec<_> = self
            .keys
            .iter()
            .filter(|(key, _)| !super::layer_grid::contains(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        entries.sort_by(|(a, _), (b, _)| {
            let rank = |key: &str| {
                (
                    order.get(key).copied().unwrap_or(usize::MAX),
                    CATALOG_ORDER.get(key).copied().unwrap_or(usize::MAX),
                )
            };
            rank(a).cmp(&rank(b)).then_with(|| a.cmp(b))
        });
        entries
    }
}

/// What a key does inside a layer keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAssignment {
    /// No entry: the base keymap decides.
    Transparent,
    /// `null`: the key does nothing.
    Swallow,
    Action(String),
}

impl KeymapPreset {
    pub fn has_layer(&self, id: &str) -> bool {
        self.layers.iter().any(|layer| layer.id == id)
    }

    /// Whether the enabled base rules of `key` are simple enough to be
    /// edited as a plain tap action.
    pub fn base_tap_editable(&self, key: &str) -> bool {
        let matching: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| rule.key == key && rule.is_enabled())
            .collect();
        matching.len() <= 1
            && !matching.iter().any(|rule| {
                !rule.layer_id.is_empty()
                    || rule.condition_game_mode.is_some()
                    || rule.condition_keyboard_layouts.is_some()
                    || rule.condition_window_patterns.is_some()
                    || rule.condition_excluded_window_patterns.is_some()
                    || rule
                        .hold_action
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
                    || !rule.long_hold_action.is_empty()
                    || !rule.double_tap_action.is_empty()
            })
    }

    /// Set the tap action of `key`'s enabled rule, adding a rule if none.
    pub fn set_base_tap(&mut self, key: &str, action: &str) {
        if let Some(rule) = self
            .rules
            .iter_mut()
            .find(|rule| rule.key == key && rule.is_enabled())
        {
            rule.tap_action = Some(action.into());
            return;
        }
        let mut index = 1;
        let id = loop {
            let id = format!("slint-key-{index}");
            if self.rules.iter().all(|rule| rule.id != id) {
                break id;
            }
            index += 1;
        };
        let mut rule = KeyRule::new(id, key);
        rule.tap_action = Some(action.into());
        self.rules.push(rule);
    }

    pub fn add_layer(&mut self, id: &str, name: String, description: Option<String>) {
        self.layers.push(Layer {
            id: id.to_owned(),
            name,
            description,
        });
        self.get_or_create_layer_keymap(id);
    }

    pub fn rename_layer(&mut self, id: &str, name: String, description: Option<String>) {
        if let Some(layer) = self.layers.iter_mut().find(|layer| layer.id == id) {
            layer.name = name;
            layer.description = description;
        }
    }

    /// Add layer `id` with a copy of the keymap of `source_id`.
    pub fn clone_layer(
        &mut self,
        source_id: &str,
        id: &str,
        name: String,
        description: Option<String>,
    ) {
        let keymap = self
            .layer_keymaps
            .get(source_id)
            .cloned()
            .unwrap_or_default();
        self.layer_keymaps.insert(id.to_owned(), keymap);
        self.layers.push(Layer {
            id: id.to_owned(),
            name,
            description,
        });
    }

    /// Remove a layer, its keymap and the references of rules to it.
    pub fn remove_layer(&mut self, id: &str) {
        self.layers.retain(|layer| layer.id != id);
        self.layer_keymaps.remove(id);
        for rule in &mut self.rules {
            if rule.layer_id == id {
                if rule.hold_behavior() == HoldBehavior::Layer {
                    rule.hold_behavior = Some(HoldBehavior::None);
                }
                rule.layer_id.clear();
            }
        }
    }

    /// Enabled triggers that activate this layer when held, in rule order.
    pub fn layer_triggers(&self, id: &str) -> Vec<String> {
        let mut keys = Vec::new();
        for rule in &self.rules {
            if rule.is_enabled()
                && rule.layer_id == id
                && rule.hold_behavior() == HoldBehavior::Layer
                && !keys.contains(&rule.key)
            {
                keys.push(rule.key.clone());
            }
        }
        keys
    }

    pub fn clear_layer_keys(&mut self, id: &str) {
        let map = self.get_or_create_layer_keymap(id);
        map.keys.clear();
        map.extra_key_order.clear();
        map.hold_modifiers.clear();
    }

    /// Entry of `key` in the keymap of `layer_id`.
    pub fn layer_key(&self, layer_id: &str, key: &str) -> KeyAssignment {
        match self
            .layer_keymaps
            .get(layer_id)
            .and_then(|keymap| keymap.keys.get(key))
        {
            None => KeyAssignment::Transparent,
            Some(None) => KeyAssignment::Swallow,
            Some(Some(action)) => KeyAssignment::Action(action.clone()),
        }
    }

    pub fn layer_has_key(&self, layer_id: &str, key: &str) -> bool {
        self.layer_keymaps
            .get(layer_id)
            .is_some_and(|map| map.keys.contains_key(key))
    }

    pub fn set_layer_key(&mut self, layer_id: &str, key: &str, assignment: KeyAssignment) {
        let map = self.get_or_create_layer_keymap(layer_id);
        match assignment {
            KeyAssignment::Transparent => {
                map.keys.remove(key);
                map.extra_key_order.retain(|code| code != key);
                map.hold_modifiers.remove(key);
            }
            KeyAssignment::Swallow => {
                map.keys.insert(key.into(), None);
                map.hold_modifiers.remove(key);
            }
            KeyAssignment::Action(action) => {
                map.keys.insert(key.into(), Some(action));
            }
        }
    }

    /// Set an additional key of `layer_id`, replacing the row of `previous`
    /// when it is given. The row keeps its display position.
    pub fn set_extra_key(
        &mut self,
        layer_id: &str,
        previous: Option<&str>,
        key: &str,
        assignment: KeyAssignment,
    ) {
        if let Some(previous) = previous {
            let map = self.get_or_create_layer_keymap(layer_id);
            map.keys.remove(previous);
            for code in &mut map.extra_key_order {
                if code == previous {
                    *code = key.to_owned();
                }
            }
            if map.hold_modifiers.remove(previous) {
                map.hold_modifiers.insert(key.to_owned());
            }
        }
        self.set_layer_key(layer_id, key, assignment);
    }

    /// Whether `key` of `layer_id` keeps its modifiers pressed until the
    /// layer is released.
    pub fn layer_key_holds_modifiers(&self, layer_id: &str, key: &str) -> bool {
        self.layer_keymaps
            .get(layer_id)
            .is_some_and(|keymap| keymap.hold_modifiers.contains(key))
    }

    pub fn set_layer_key_holds_modifiers(&mut self, layer_id: &str, key: &str, hold: bool) {
        let held = &mut self.get_or_create_layer_keymap(layer_id).hold_modifiers;
        if hold {
            held.insert(key.to_owned());
        } else {
            held.remove(key);
        }
    }

    /// Exchange the entries of keys `a` and `b` in the keymap of `layer_id`.
    pub fn swap_layer_keys(&mut self, layer_id: &str, a: &str, b: &str) {
        let map = self.get_or_create_layer_keymap(layer_id);
        let first = map.keys.remove(a);
        let second = map.keys.remove(b);
        if let Some(value) = first {
            map.keys.insert(b.into(), value);
        }
        if let Some(value) = second {
            map.keys.insert(a.into(), value);
        }
        let held_a = map.hold_modifiers.remove(a);
        let held_b = map.hold_modifiers.remove(b);
        if held_a {
            map.hold_modifiers.insert(b.into());
        }
        if held_b {
            map.hold_modifiers.insert(a.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activating_triggers_exclude_disabled_and_other_hold_modes_and_deduplicate() {
        let mut keymap = KeymapPreset::default();
        for (key, layer, enabled, mode) in [
            ("CapsLock", "nav", true, HoldBehavior::Layer),
            ("CapsLock", "nav", true, HoldBehavior::Layer),
            ("KeyQ", "nav", false, HoldBehavior::Layer),
            ("KeyW", "other", true, HoldBehavior::Layer),
            ("KeyE", "nav", true, HoldBehavior::None),
            ("ShiftLeft+KeyA", "nav", true, HoldBehavior::Layer),
        ] {
            let mut rule = KeyRule::new(format!("rule-{}", keymap.rules.len()), key);
            rule.layer_id = layer.into();
            rule.enabled = Some(enabled);
            rule.hold_behavior = Some(mode);
            keymap.rules.push(rule);
        }
        assert_eq!(keymap.layer_triggers("nav"), ["CapsLock", "ShiftLeft+KeyA"]);
        assert!(keymap.layer_triggers("missing").is_empty());
    }

    fn keymap(keys: &[(&str, Option<&str>)]) -> LayerKeymap {
        LayerKeymap {
            keys: keys
                .iter()
                .map(|(key, value)| ((*key).into(), value.map(str::to_owned)))
                .collect(),
            ..Default::default()
        }
    }

    fn keys(entries: Vec<(String, Option<String>)>) -> Vec<String> {
        entries.into_iter().map(|(key, _)| key).collect()
    }

    #[test]
    fn extra_keys_exclude_grid_keys_and_preserve_assignments() {
        let map = keymap(&[
            ("KeyQ", Some("KeyA")),
            ("KeyW", None),
            ("Backspace", Some("sys:turnOffScreen")),
            ("F4", None),
        ]);
        let entries = map.extra_entries();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&("Backspace".into(), Some("sys:turnOffScreen".into()))));
        assert!(entries.contains(&("F4".into(), None)));
    }

    #[test]
    fn extra_key_suppression_is_preserved() {
        let keymap = crate::config::keymap_file::parse(
            r#"{"version": 1, "layers": [{"id": "extra", "keys": {"BracketRight": null, "Backslash": null, "F4": "F4"}}]}"#
        ).unwrap().unwrap();
        let entries = keymap.layer_keymaps["extra"].extra_entries();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&("BracketRight".into(), None)));
        assert!(entries.contains(&("F4".into(), Some("F4".into()))));
    }

    #[test]
    fn unordered_extra_keys_follow_the_saved_order_then_the_catalog() {
        let mut map = keymap(&[("F2", None), ("F14", None), ("F1", None), ("F13", None)]);
        map.extra_key_order = vec!["F14".into(), "F13".into()];
        assert_eq!(keys(map.extra_entries()), ["F14", "F13", "F1", "F2"]);
    }

    #[test]
    fn extra_order_is_stable_after_renaming_and_removal() {
        let mut keymap = KeymapPreset::default();
        keymap.add_layer("extra", "Extra".into(), None);
        for key in ["F13", "F14", "F15"] {
            keymap.set_extra_key("extra", None, key, KeyAssignment::Swallow);
        }
        keymap
            .layer_keymaps
            .get_mut("extra")
            .unwrap()
            .extra_key_order = vec!["F15".into(), "F13".into(), "F14".into()];
        keymap.set_extra_key("extra", Some("F15"), "F16", KeyAssignment::Swallow);
        keymap.set_layer_key("extra", "F13", KeyAssignment::Transparent);
        assert_eq!(
            keys(keymap.layer_keymaps["extra"].extra_entries()),
            ["F16", "F14"]
        );
    }
}
