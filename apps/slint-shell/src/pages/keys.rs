//! Base keyboard page: the tap action of every key outside the layers.
//! Keys are edited through the action picker.

use crate::{
    document::{Document, View},
    i18n::Msg,
    keyboard,
    ui::{ActionPicker, AppState, BaseKey, KeyEditor, PickerTarget, SettingsWindow},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

fn refresh(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let keys: Vec<BaseKey> = keyboard::BASE
        .iter()
        .map(|code| BaseKey {
            code: (*code).into(),
            label: keyboard::label(code).into(),
            action: config.base_tap_action(code).unwrap_or_default().into(),
            category: super::picker::category_for_value(config.base_tap_action(code).unwrap_or_default()),
        })
        .collect();
    let editor = ui.global::<KeyEditor>();
    editor.set_columns(keyboard::BASE_COLUMNS as i32);
    editor.set_keys(ModelRc::new(VecModel::from(keys)));
}

/// Assign `value` as the tap action of base key `index`.
pub(super) fn assign(
    ui: &SettingsWindow,
    document: &Document,
    index: i32,
    value: &str,
) -> Result<(), Msg> {
    let key = keyboard::base_key(index).ok_or(Msg::None)?;
    let saved = document
        .edit(View::Keys, |config| config.set_base_tap_action(key, value))
        .map_err(|error| Msg::from(&error))?;
    refresh(ui, document);
    ui.global::<AppState>()
        .set_status(crate::notifications::report(ui, &saved.message(Msg::None)));
    Ok(())
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    refresh(ui, document);
    let weak = ui.as_weak();
    document.subscribe(View::Keys, move |document| {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, document);
        }
    });
    let weak = ui.as_weak();
    let doc = document.clone();
    ui.global::<KeyEditor>().on_edit(move |index| {
        let (Some(ui), Some(key)) = (weak.upgrade(), keyboard::base_key(index)) else {
            return;
        };
        ui.global::<KeyEditor>().set_selected(index);
        let current = doc.read().base_tap_action(key).unwrap_or_default().to_owned();
        ui.global::<ActionPicker>()
            .invoke_open(PickerTarget::BaseKey, index, current.into(), false);
    });
}
