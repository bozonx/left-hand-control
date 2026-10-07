//! Pages of the settings window. Each module binds one Slint global to the
//! shared [`Document`] and refreshes it when the document changes.

mod conditions;
mod reorder;
mod keys;
mod layers;
mod library;
mod macros;
mod menus;
mod picker;
mod rules;
mod settings;

pub use library::refresh_active;
pub use picker::capture;

use crate::{
    document::Document,
    ui::{ActionKind, GameCondition, SettingsWindow},
};
use lhc_core::profile::{
    actions::Action,
    model::{Appearance, LocalePreference},
};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Bind every page of `ui` to `document`.
pub fn bind_document(ui: &SettingsWindow, document: &Rc<Document>) {
    reorder::bind(ui);
    conditions::bind(ui);
    picker::bind(ui, document);
    keys::bind(ui, document);
    library::bind(ui, document);
    rules::bind(ui, document);
    layers::bind(ui, document);
    macros::bind(ui, document);
    menus::bind(ui, document);
    settings::bind(ui, document);
}

pub(crate) fn strings(values: impl IntoIterator<Item = impl Into<SharedString>>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}

/// Comma-separated list as typed in the UI; empty items are dropped.
pub(crate) fn parse_list(value: &str) -> Vec<String> {
    if let Ok(items) = serde_json::from_str::<Vec<String>>(value) {
        return items;
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(crate) fn condition_list(items: &[String]) -> String {
    if items.iter().any(|item| item.contains(',') || item.starts_with('[')) {
        serde_json::to_string(items).unwrap_or_default()
    } else {
        items.join(", ")
    }
}

/// Game-mode condition as stored in the config (`"on"`, `"off"` or none).
pub(crate) fn game_condition(value: Option<&str>) -> GameCondition {
    match value {
        Some("on") => GameCondition::On,
        Some("off") => GameCondition::Off,
        _ => GameCondition::Any,
    }
}

pub(crate) fn game_condition_value(condition: GameCondition) -> Option<String> {
    match condition {
        GameCondition::On => Some("on".into()),
        GameCondition::Off => Some("off".into()),
        GameCondition::Any => None,
    }
}

/// Icon of an action on key caps.
pub(crate) fn action_kind(action: &str) -> ActionKind {
    match Action::parse(Some(action)) {
        Action::Keys(_) => ActionKind::Keys,
        Action::Macro(_) => ActionKind::Macro,
        Action::Command(_) => ActionKind::Command,
        Action::System(_) => ActionKind::System,
        Action::App(_) => ActionKind::App,
        Action::Text(_) => ActionKind::Text,
        Action::Native | Action::Swallow | Action::Pause(_) => ActionKind::None,
    }
}

/// Order of the appearance and language choices in the settings page.
pub(crate) const APPEARANCES: [Appearance; 3] =
    [Appearance::System, Appearance::Light, Appearance::Dark];
pub(crate) const LOCALES: [LocalePreference; 3] = [
    LocalePreference::Auto,
    LocalePreference::English,
    LocalePreference::Russian,
];

/// Index of `value` in `choices`, or 0.
pub(crate) fn choice_index<T: PartialEq>(choices: &[T], value: &T) -> i32 {
    choices
        .iter()
        .position(|choice| choice == value)
        .unwrap_or(0) as i32
}

/// Choice at a UI index, falling back to the first one.
pub(crate) fn choice<T: Copy>(choices: &[T], index: i32) -> T {
    usize::try_from(index)
        .ok()
        .and_then(|index| choices.get(index).copied())
        .unwrap_or(choices[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_round_trip() {
        assert_eq!(parse_list(" us, , ru "), ["us", "ru"]);
        for condition in [GameCondition::Any, GameCondition::On, GameCondition::Off] {
            assert_eq!(
                game_condition(game_condition_value(condition).as_deref()),
                condition
            );
        }
        assert_eq!(choice(&APPEARANCES, 2), Appearance::Dark);
        assert_eq!(choice(&APPEARANCES, 9), Appearance::System);
        assert_eq!(choice_index(&LOCALES, &LocalePreference::Russian), 2);
        assert_eq!(action_kind("text:hi"), ActionKind::Text);
        assert_eq!(action_kind("Ctrl+KeyC"), ActionKind::Keys);
    }
}
