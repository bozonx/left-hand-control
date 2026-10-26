//! Edits of layers and their keymaps on a layout.
//!
//! These operations change the layout only; validation of action strings
//! and saving belong to [`crate::config_document::ConfigDocument`].

use super::model::{Layer, LayerRule, LayoutPreset};

/// What a key does inside a layer keymap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAssignment {
    /// No entry: the base layout decides.
    Transparent,
    /// `null`: the key does nothing.
    Swallow,
    Action(String),
}

impl LayoutPreset {
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
                    || rule.condition_layouts.is_some()
                    || rule.condition_apps_whitelist.is_some()
                    || rule.condition_apps_blacklist.is_some()
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
        let mut rule = LayerRule::new(id, key);
        rule.tap_action = Some(action.into());
        self.rules.push(rule);
    }

    pub fn add_layer(&mut self, id: &str, name: String, description: Option<String>) {
        self.layers.push(Layer {
            id: id.to_owned(),
            name,
            description,
        });
        self.layer_keymap_mut(id);
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
                rule.layer_id.clear();
            }
        }
    }

    pub fn clear_layer_keys(&mut self, id: &str) {
        let map = self.layer_keymap_mut(id);
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
        let map = self.layer_keymap_mut(layer_id);
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
            let map = self.layer_keymap_mut(layer_id);
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
        let held = &mut self.layer_keymap_mut(layer_id).hold_modifiers;
        if hold {
            held.insert(key.to_owned());
        } else {
            held.remove(key);
        }
    }

    /// Exchange the entries of keys `a` and `b` in the keymap of `layer_id`.
    pub fn swap_layer_keys(&mut self, layer_id: &str, a: &str, b: &str) {
        let map = self.layer_keymap_mut(layer_id);
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
