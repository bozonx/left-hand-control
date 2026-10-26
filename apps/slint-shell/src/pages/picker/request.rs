//! Typed picker requests and value restrictions.

use super::catalog::valid_pause;
use crate::{
    document::Document,
    i18n::Msg,
    pages::{layers, macros, rules},
    ui::{ActionPicker, PickerBehavior, PickerTarget, PickerTitle, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions::{self, Action},
        key_catalog::CATEGORIES,
    },
};

/// Mutually exclusive contexts of an action picker session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PickerMode {
    #[default]
    Action,
    Trigger,
    MacroStep,
    LayerAction,
    RuleTap,
    RuleHold,
    QuickAction,
}

/// What the page opening the picker allows; built by that page from its
/// own state so the picker does not read other pages.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct PickerRequest {
    pub title: PickerTitle,
    pub mode: PickerMode,
    /// Only a key can be chosen (no action).
    pub key_only: bool,
    /// The current assignment swallows the key.
    pub swallowed: bool,
    pub hold_modifiers: bool,
    pub allow_default: bool,
    pub allow_swallow: bool,
    pub inherit_default: bool,
    pub empty_default: bool,
    /// Holding also activates a layer.
    pub hold_layer: bool,
}

impl PickerRequest {
    /// Request of a target that has no page-specific options.
    pub(super) fn plain(target: PickerTarget, key_only: bool) -> Self {
        Self {
            key_only,
            mode: match target {
                PickerTarget::MacroStep => PickerMode::MacroStep,
                PickerTarget::QuickAction => PickerMode::QuickAction,
                _ => PickerMode::Action,
            },
            title: if target == PickerTarget::MacroStep {
                PickerTitle::MacroStep
            } else {
                PickerTitle::Auto
            },
            allow_default: target == PickerTarget::BaseKey,
            ..Self::default()
        }
    }

    /// Initial behavior for a current value (`empty` when unassigned).
    pub(super) fn behavior(&self, empty: bool) -> PickerBehavior {
        if self.swallowed && self.allow_swallow {
            PickerBehavior::Swallow
        } else if self.allow_default && (empty || self.swallowed) {
            PickerBehavior::Native
        } else {
            PickerBehavior::Action
        }
    }

    /// Label of the "default" behavior.
    pub(super) fn native_label(&self) -> Msg {
        if self.inherit_default {
            Msg::PickerInherit
        } else if self.mode == PickerMode::QuickAction {
            Msg::PickerEmptyCell
        } else if self.empty_default {
            Msg::PickerNoAction
        } else if self.hold_layer {
            Msg::PickerLayerOnly
        } else {
            Msg::PickerNative
        }
    }
}

/// Ask the page owning `target` what the picker may assign.
pub(super) fn request(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
    key_only: bool,
) -> PickerRequest {
    let mut request = match target {
        PickerTarget::Rule => rules::picker_request(ui, key_only),
        PickerTarget::LayerAction | PickerTarget::ExtraAction | PickerTarget::ExtraKey => {
            layers::picker_request(ui, document, target, index)
        }
        PickerTarget::MacroStep | PickerTarget::QuickAction | PickerTarget::BaseKey => {
            PickerRequest::plain(target, key_only)
        }
    };
    request.key_only = key_only;
    request.allow_default &= !key_only;
    request
}

/// Which values the open picker accepts. The list and the Apply check use
/// the same rules, so a listed value can always be applied.
#[derive(Debug, Default)]
pub(super) struct Filter {
    pub(super) key_only: bool,
    /// Rule trigger: a key or key chord.
    pub(super) trigger: bool,
    /// Only keys that can be held.
    pub(super) held_key: bool,
    pub(super) macro_step: bool,
    /// Macro being edited, which cannot call itself.
    pub(super) excluded_macro: String,
    /// Keys that already have an extra row.
    pub(super) unavailable_keys: Vec<String>,
}

impl Filter {
    /// Restrictions of the open picker; list items are checked with it.
    pub(super) fn admits(&self, value: &str) -> bool {
        if self.unavailable_keys.iter().any(|key| key == value) {
            return false;
        }
        if self.trigger {
            return lhc_core::profile::key_catalog::valid_trigger(value);
        }
        if self.key_only {
            return CATEGORIES.iter().any(|keys| keys.contains(&value));
        }
        if self.held_key && !actions::valid_held_key(value) {
            return false;
        }
        !matches!(Action::parse(Some(value)),
            Action::Macro(id) if !self.excluded_macro.is_empty() && id == self.excluded_macro)
    }

    /// Whether `value` can be applied; `config` validates references.
    pub(super) fn accepts(&self, value: &str, config: Option<&ConfigDocument>) -> bool {
        if value.trim().is_empty() || !self.admits(value) {
            return false;
        }
        if self.key_only || self.trigger {
            return true;
        }
        let action = Action::parse(Some(value));
        match &action {
            Action::Keys(_) if !actions::valid_held_key(value) => return false,
            Action::Text(text) if text.is_empty() => return false,
            Action::Pause(ms) => return self.macro_step && valid_pause(ms),
            _ => {}
        }
        config.is_none_or(|config| actions::validate(&action, &config.config()).is_none())
    }
}

/// Filter of the open picker.
pub(super) fn filter(
    ui: &SettingsWindow,
    document: &Document,
    picker: &ActionPicker,
    target: Option<(PickerTarget, i32)>,
) -> Filter {
    let target = target.map(|(target, _)| target);
    let key_only = picker.get_key_only();
    Filter {
        key_only,
        trigger: key_only && target == Some(PickerTarget::Rule),
        held_key: picker.get_hold_action(),
        macro_step: picker.get_macro_step(),
        excluded_macro: if picker.get_macro_step() {
            macros::picker_macro_id(ui)
        } else {
            String::new()
        },
        unavailable_keys: if target == Some(PickerTarget::ExtraKey) {
            layers::taken_extra_keys(ui, document)
        } else {
            Vec::new()
        },
    }
}
