use std::collections::BTreeMap;

#[derive(Default)]
pub struct UiState {
    selected_layer_id: String,
    label_mode: i32,
    extra_key_orders: BTreeMap<(String, String), Vec<String>>,
}

impl UiState {
    pub fn selected_layer_id(&self) -> &str {
        &self.selected_layer_id
    }

    pub fn label_mode(&self) -> i32 {
        self.label_mode
    }

    /// Additional-key row order for the current UI session.
    pub fn extra_key_order(&self, layout: &str, layer: &str) -> &[String] {
        self.extra_key_orders
            .get(&(layout.to_owned(), layer.to_owned()))
            .map_or(&[], Vec::as_slice)
    }

    pub fn reorder_extra_keys(
        &mut self,
        layout: &str,
        layer: &str,
        mut keys: Vec<String>,
        from: usize,
        to: usize,
    ) {
        if from >= keys.len() || to >= keys.len() || from == to {
            return;
        }
        let key = keys.remove(from);
        keys.insert(to, key);
        self.extra_key_orders
            .insert((layout.to_owned(), layer.to_owned()), keys);
    }

    pub fn rename_extra_key(&mut self, layout: &str, layer: &str, old: &str, new: &str) {
        if let Some(keys) = self
            .extra_key_orders
            .get_mut(&(layout.to_owned(), layer.to_owned()))
        {
            for key in keys.iter_mut() {
                if key == old {
                    *key = new.to_owned();
                }
            }
            keys.retain(|key| !key.is_empty());
        }
    }

    pub fn update(&mut self, layer: Option<&str>, mode: Option<i32>) {
        if let Some(layer) = layer {
            self.selected_layer_id = layer.to_owned();
        }
        if let Some(mode) = mode {
            self.label_mode = mode.clamp(0, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_key_order_is_scoped_and_tracks_renaming_and_removal() {
        let mut state = UiState::default();
        let keys = vec!["F13".into(), "F14".into(), "F15".into()];
        state.reorder_extra_keys("one", "nav", keys.clone(), 0, 2);
        assert_eq!(state.extra_key_order("one", "nav"), ["F14", "F15", "F13"]);
        assert!(state.extra_key_order("two", "nav").is_empty());
        assert!(state.extra_key_order("one", "sym").is_empty());
        state.reorder_extra_keys("one", "nav", keys, 8, 0);
        assert_eq!(state.extra_key_order("one", "nav"), ["F14", "F15", "F13"]);
        state.rename_extra_key("one", "nav", "F14", "F16");
        state.rename_extra_key("one", "nav", "F15", "");
        assert_eq!(state.extra_key_order("one", "nav"), ["F16", "F13"]);
        assert!(UiState::default().extra_key_order("one", "nav").is_empty());
    }

    #[test]
    fn preferences_do_not_survive_a_new_session() {
        let mut state = UiState::default();
        state.update(Some("nav"), Some(1));
        assert_eq!(state.selected_layer_id(), "nav");
        assert_eq!(state.label_mode(), 1);
        let next = UiState::default();
        assert_eq!(next.selected_layer_id(), "");
        assert_eq!(next.label_mode(), 0);
    }
}
