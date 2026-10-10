use super::{condition_list, parse_list, strings};
use crate::ui::{ConditionChoices, LanguageChoice, SettingsWindow, WindowChoice};
use lhc_core::active_window::ActiveWindow;
use lhc_core::profile::app_match;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

/// Seconds the user gets to focus a window before it is read.
const PICK_DELAY: i32 = 3;

/// Show which application has the focus, as conditions see it.
pub(crate) fn show_active_window(ui: &SettingsWindow, window: Option<&ActiveWindow>) {
    let text = window.map_or_else(String::new, |window| {
        let mut parts: Vec<&str> = Vec::new();
        if !window.app_id.is_empty() {
            parts.push(&window.app_id);
        }
        if let Some(process) = window.process_name.as_deref()
            && !process.is_empty()
            && !process.eq_ignore_ascii_case(&window.app_id)
        {
            parts.push(process);
        }
        parts.join(" · ")
    });
    ui.global::<ConditionChoices>()
        .set_active_window(text.into());
}

/// The pattern a condition stores for a window: its app id, or the
/// process when there is none.
fn window_pattern(app_id: &str, process: Option<&str>) -> String {
    if app_id.is_empty() {
        process.unwrap_or_default().to_owned()
    } else {
        app_id.to_owned()
    }
}

pub(super) fn bind(ui: &SettingsWindow) {
    let choices = ui.global::<ConditionChoices>();
    show_active_window(ui, lhc_core::active_window::cached_active_window().as_ref());
    choices.on_is_title(|pattern| app_match::title_text(&pattern).is_some());
    choices.on_title_text(|pattern| app_match::title_text(&pattern).unwrap_or("").into());
    choices.on_make_pattern(|text, title| {
        if title {
            app_match::title_pattern(&text).into()
        } else {
            text.trim().into()
        }
    });

    let weak = ui.as_weak();
    choices.on_refresh_open_windows(move || {
        let Some(ui) = weak.upgrade() else { return };
        let choices = ui.global::<ConditionChoices>();
        if choices.get_open_windows_loading() {
            return;
        }
        choices.set_open_windows_loading(true);
        let weak = weak.clone();
        // The compositor may take a moment; keep the UI responsive.
        std::thread::spawn(move || {
            let windows = lhc_core::active_window::open_windows();
            if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
                let choices = ui.global::<ConditionChoices>();
                choices.set_open_windows(ModelRc::new(VecModel::from(
                    windows
                        .into_iter()
                        .map(|window| WindowChoice {
                            pattern: window_pattern(&window.app_id, window.process_name.as_deref())
                                .into(),
                            app_id: window.app_id.into(),
                            process: window.process_name.unwrap_or_default().into(),
                            title: window.title.into(),
                        })
                        .collect::<Vec<_>>(),
                )));
                choices.set_open_windows_loading(false);
            }) {
                log::warn!("open windows: {error}");
            }
        });
    });

    let timer = Rc::new(slint::Timer::default());
    let weak = ui.as_weak();
    choices.on_pick_active_window(move || {
        let Some(ui) = weak.upgrade() else { return };
        ui.global::<ConditionChoices>()
            .set_pick_countdown(PICK_DELAY);
        let weak = weak.clone();
        let stop = Rc::downgrade(&timer);
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(1),
            move || {
                let Some(ui) = weak.upgrade() else { return };
                let choices = ui.global::<ConditionChoices>();
                let left = (choices.get_pick_countdown() - 1).max(0);
                choices.set_pick_countdown(left);
                if left > 0 {
                    return;
                }
                if let Some(timer) = stop.upgrade() {
                    timer.stop();
                }
                let picked = lhc_core::active_window::detect_active_window_now()
                    .map(|window| window_pattern(&window.app_id, window.process_name.as_deref()))
                    .unwrap_or_default();
                choices.set_picked(picked.into());
                choices.set_picked_serial(choices.get_picked_serial() + 1);
            },
        );
    });

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
