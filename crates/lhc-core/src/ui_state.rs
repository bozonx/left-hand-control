#[derive(Default)]
pub struct UiState {
    selected_layer_id: String,
    label_mode: i32,
}

impl UiState {
    pub fn selected_layer_id(&self) -> &str {
        &self.selected_layer_id
    }

    pub fn label_mode(&self) -> i32 {
        self.label_mode
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
