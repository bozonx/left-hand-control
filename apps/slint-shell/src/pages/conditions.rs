use super::{condition_list, parse_list, strings};
use crate::ui::{ConditionChoices, LanguageChoice, SettingsWindow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

pub(super) fn bind(ui: &SettingsWindow) {
    let choices = ui.global::<ConditionChoices>();
    let weak = ui.as_weak();
    choices.on_refresh(move || {
        let Some(ui) = weak.upgrade() else { return };
        match lhc_core::layout::available_layouts() {
            Ok(layouts) => {
                ui.global::<ConditionChoices>()
                    .set_languages(ModelRc::new(VecModel::from(
                        layouts
                            .into_iter()
                            .map(|layout| LanguageChoice {
                                label: if layout.display.is_empty() {
                                    layout.short.clone()
                                } else {
                                    format!("{} ({})", layout.short, layout.display)
                                }
                                .into(),
                                code: layout.short.into(),
                            })
                            .collect::<Vec<_>>(),
                    )))
            }
            Err(error) => log::warn!("system languages: {error}"),
        }
    });
    let weak = ui.as_weak();
    choices.on_available(move |code| {
        weak.upgrade().is_some_and(|ui| {
            ui.global::<ConditionChoices>()
                .get_languages()
                .iter()
                .any(|item| item.code == code)
        })
    });
    choices.on_contains(|value, item| {
        parse_list(&value)
            .iter()
            .any(|value| value == item.as_str())
    });
    choices.on_items(|value| strings(parse_list(&value)));
    choices.on_toggle(|value, item, checked| {
        let mut values = parse_list(&value);
        values.retain(|value| value != item.as_str());
        if checked {
            values.push(item.to_string());
        }
        condition_list(&values).into()
    });
    choices.on_add(|value, item| {
        let mut values = parse_list(&value);
        let item = item.trim();
        if !item.is_empty() && !values.iter().any(|value| value == item) {
            values.push(item.to_owned());
        }
        condition_list(&values).into()
    });
    choices.on_remove(|value, index| {
        let mut values = parse_list(&value);
        if let Ok(index) = usize::try_from(index) {
            if index < values.len() {
                values.remove(index);
            }
        }
        condition_list(&values).into()
    });
}
