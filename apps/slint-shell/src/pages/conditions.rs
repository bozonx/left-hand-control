use super::{condition_list, parse_list, strings};
use crate::ui::{ConditionChoices, LanguageChoice, SettingsWindow, WindowChoice};
use lhc_core::active_window::{ActiveWindow, OpenWindow};
use lhc_core::profile::app_match;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// How many running processes the picker lists.
const PROCESSES: usize = 8;

thread_local! {
    /// Running processes, re-read when older than two seconds.
    static PROCESS_CACHE: RefCell<Option<(Instant, Vec<String>)>> = const { RefCell::new(None) };
}

/// Put the focused application first in the picker's open-window list.
pub(crate) fn show_active_window(ui: &SettingsWindow, window: Option<&ActiveWindow>) {
    let choices = ui.global::<ConditionChoices>();
    let rows = choices.get_open_windows().iter().collect::<Vec<_>>();
    choices.set_open_windows(ModelRc::new(VecModel::from(arrange(rows, window))));
}

/// One row per application, in the order of `windows`; the first title
/// stands for the others.
fn group(windows: Vec<OpenWindow>) -> Vec<WindowChoice> {
    let mut rows: Vec<WindowChoice> = Vec::new();
    for window in windows {
        let pattern = window_pattern(&window.app_id, window.process_name.as_deref());
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.pattern.eq_ignore_ascii_case(&pattern))
        {
            row.count += 1;
            if row.title.is_empty() {
                row.title = window.title.into();
            }
            continue;
        }
        rows.push(WindowChoice {
            pattern: pattern.into(),
            app_id: window.app_id.into(),
            process: window.process_name.unwrap_or_default().into(),
            title: window.title.into(),
            count: 1,
            recent: false,
        });
    }
    rows
}

/// Mark the active application and put it first, the others by app id.
/// It is not added without an open window: that is mostly the desktop
/// shell, focused when the settings are opened from the tray.
fn arrange(mut rows: Vec<WindowChoice>, active: Option<&ActiveWindow>) -> Vec<WindowChoice> {
    let pattern = active
        .map(|active| window_pattern(&active.app_id, active.process_name.as_deref()))
        .unwrap_or_default();
    for row in &mut rows {
        row.recent = !pattern.is_empty() && row.pattern.eq_ignore_ascii_case(&pattern);
    }
    rows.sort_by_key(|row| {
        (
            !row.recent,
            row.app_id.to_lowercase(),
            row.process.to_lowercase(),
        )
    });
    rows
}

/// Whether the window's app id, process or title contains `query`.
fn window_matches(window: &WindowChoice, query: &str) -> bool {
    [&window.app_id, &window.process, &window.title]
        .iter()
        .any(|text| text.to_lowercase().contains(query))
}

/// Running processes containing `query` that `taken` does not list.
fn process_matches(processes: &[String], taken: &[String], query: &str) -> Vec<String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    processes
        .iter()
        .filter(|name| {
            name.to_lowercase().contains(&query)
                && !taken.iter().any(|taken| taken.eq_ignore_ascii_case(name))
        })
        .take(PROCESSES)
        .cloned()
        .collect()
}

fn running_processes() -> Vec<String> {
    PROCESS_CACHE.with_borrow_mut(|cache| {
        let stale = cache
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() > Duration::from_secs(2));
        if stale {
            *cache = Some((Instant::now(), lhc_core::gamemode::running_processes()));
        }
        cache
            .as_ref()
            .map(|(_, names)| names.clone())
            .unwrap_or_default()
    })
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
    choices.on_has(|patterns, pattern| {
        patterns
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&pattern))
    });
    choices.on_matching(|windows, query| {
        let query = query.trim().to_lowercase();
        ModelRc::new(VecModel::from(
            windows
                .iter()
                .filter(|window| window_matches(window, &query))
                .collect::<Vec<_>>(),
        ))
    });
    choices.on_processes(|windows, query| {
        if query.trim().is_empty() {
            return strings(Vec::<String>::new());
        }
        let taken = windows
            .iter()
            .flat_map(|window| [window.pattern.to_string(), window.process.to_string()])
            .collect::<Vec<_>>();
        strings(process_matches(&running_processes(), &taken, &query))
    });
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
                let rows = arrange(
                    group(windows),
                    lhc_core::active_window::cached_active_window().as_ref(),
                );
                choices.set_open_windows(ModelRc::new(VecModel::from(rows)));
                choices.set_open_windows_loading(false);
            }) {
                log::warn!("open windows: {error}");
            }
        });
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
    choices.on_unknown_languages(|value, languages| {
        strings(
            parse_list(&value)
                .into_iter()
                .filter(|code| {
                    !languages
                        .iter()
                        .any(|language| language.code == code.as_str())
                })
                .collect::<Vec<_>>(),
        )
    });
    choices.on_toggle(|value, item, checked| {
        let mut values = parse_list(&value);
        values.retain(|value| !value.eq_ignore_ascii_case(&item));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn open(app_id: &str, process: &str, title: &str) -> OpenWindow {
        OpenWindow {
            app_id: app_id.into(),
            process_name: Some(process.into()),
            title: title.into(),
        }
    }

    #[test]
    fn windows_are_grouped_by_application() {
        let rows = group(vec![
            open("firefox", "firefox", ""),
            open("Firefox", "firefox", "Docs"),
            open("", "kate", "notes.txt"),
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].count, rows[0].title.as_str()), (2, "Docs"));
        assert_eq!(rows[1].pattern, "kate");
    }

    #[test]
    fn active_application_goes_first() {
        let rows = group(vec![
            open("dolphin", "dolphin", ""),
            open("kate", "kate", ""),
        ]);
        let active = ActiveWindow {
            app_id: "kate".into(),
            ..Default::default()
        };
        let rows = arrange(rows, Some(&active));
        assert_eq!(rows[0].pattern, "kate");
        assert!(rows[0].recent && !rows[1].recent);

        // An application without an open window is not added; the order returns.
        let plasma = ActiveWindow {
            app_id: "plasmashell".into(),
            ..Default::default()
        };
        let rows = arrange(rows, Some(&plasma));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pattern, "dolphin");
        assert!(rows.iter().all(|row| !row.recent));
    }

    #[test]
    fn processes_skip_listed_ones() {
        let processes = ["steam".to_owned(), "steamwebhelper".into(), "kate".into()];
        assert_eq!(
            process_matches(&processes, &["Steam".into()], " STEAM "),
            vec!["steamwebhelper".to_owned()]
        );
        assert!(process_matches(&processes, &[], "").is_empty());
    }
}
