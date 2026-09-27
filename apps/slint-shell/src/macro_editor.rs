use crate::{
    i18n::Msg,
    ui::{MacroEditor, MacroRow, MacroStepRow, SettingsWindow, SystemMacroRow},
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    mapper::system_macros::SYSTEM_MACROS,
    profile::{
        auto_switch::AutoSwitchContext,
        ids, macros,
        model::{AppConfig, Macro, MacroStep},
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

/// A macro being edited plus the raw delay fields, which may not parse yet.
#[derive(Clone)]
struct Entry {
    item: Macro,
    step_pause: String,
    modifier_delay: String,
}

impl Entry {
    fn new(item: Macro) -> Self {
        let text = |v: Option<u64>| v.map(|v| v.to_string()).unwrap_or_default();
        Self {
            step_pause: text(item.step_pause_ms),
            modifier_delay: text(item.modifier_delay_ms),
            item,
        }
    }
}

type Draft = Rc<RefCell<Vec<Entry>>>;

fn load(document: &ConfigDocument) -> Vec<Entry> {
    document
        .layout()
        .macros
        .iter()
        .cloned()
        .map(Entry::new)
        .collect()
}

fn parse_delay(value: &str) -> Result<Option<u64>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v <= 2000)
        .map(Some)
        .ok_or(())
}

fn candidate(document: &ConfigDocument, draft: &[Entry]) -> AppConfig {
    let mut config = document.config();
    config.macros = draft.iter().map(|e| e.item.clone()).collect();
    config
}

fn issue(config: &AppConfig, entry: &Entry) -> Option<ConfigError> {
    if parse_delay(&entry.step_pause).is_err() || parse_delay(&entry.modifier_delay).is_err() {
        return Some(ConfigError::Macro(macros::MacroIssue::DelayRange));
    }
    macros::validate(config, &entry.item).err()
}

fn steps(item: &Macro) -> ModelRc<MacroStepRow> {
    ModelRc::new(VecModel::from(
        item.steps
            .iter()
            .map(|step| {
                let pause = step.action.trim().strip_prefix("pause:");
                MacroStepRow {
                    action: step.action.clone().into(),
                    pause: pause.is_some(),
                    pause_ms: pause.unwrap_or_default().trim().into(),
                }
            })
            .collect::<Vec<_>>(),
    ))
}

fn usage(config: &AppConfig, id: &str) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        macros::usage(config, id)
            .into_iter()
            .map(Into::into)
            .collect::<Vec<SharedString>>(),
    ))
}

fn row(config: &AppConfig, entry: &Entry) -> MacroRow {
    MacroRow {
        id: entry.item.id.clone().into(),
        name: entry.item.name.clone().into(),
        step_pause: entry.step_pause.clone().into(),
        modifier_delay: entry.modifier_delay.clone().into(),
        usage: usage(config, &entry.item.id),
        error: issue(config, entry)
            .map_or(Msg::None, |e| Msg::from(&e))
            .to_ui(),
        steps: steps(&entry.item),
    }
}

/// Rebuild every row. Used after structural changes and reloads.
fn render(ui: &SettingsWindow, document: &ConfigDocument, draft: &[Entry]) {
    let editor = ui.global::<MacroEditor>();
    let config = candidate(document, draft);
    editor.set_macros(ModelRc::new(VecModel::from(
        draft.iter().map(|e| row(&config, e)).collect::<Vec<_>>(),
    )));
    editor.set_default_step_pause(
        document
            .settings()
            .default_macro_step_pause_ms
            .to_string()
            .into(),
    );
    editor.set_default_modifier_delay(
        document
            .settings()
            .default_macro_modifier_delay_ms
            .to_string()
            .into(),
    );
}

/// Refresh rows in place, keeping their step models, so fields keep focus while typing.
fn annotate(ui: &SettingsWindow, document: &ConfigDocument, draft: &[Entry]) {
    let config = candidate(document, draft);
    let rows = ui.global::<MacroEditor>().get_macros();
    for (index, entry) in draft.iter().enumerate() {
        if let Some(current) = rows.row_data(index) {
            rows.set_row_data(
                index,
                MacroRow {
                    steps: current.steps,
                    ..row(&config, entry)
                },
            );
        }
    }
}

/// Save the draft when every macro is valid, like the Tauri auto-save.
fn commit(ui: &SettingsWindow, config: &Rc<RefCell<ConfigDocument>>, draft: &[Entry]) {
    let editor = ui.global::<MacroEditor>();
    let valid = {
        let document = config.borrow();
        let candidate = candidate(&document, draft);
        draft.iter().all(|e| issue(&candidate, e).is_none())
    };
    editor.set_has_errors(!valid);
    if !valid {
        editor.set_status(Msg::None.to_ui());
        return;
    }
    let items: Vec<Macro> = draft
        .iter()
        .map(|e| Macro {
            step_pause_ms: parse_delay(&e.step_pause).unwrap_or_default(),
            modifier_delay_ms: parse_delay(&e.modifier_delay).unwrap_or_default(),
            ..e.item.clone()
        })
        .collect();
    if config.borrow().layout().macros == items {
        editor.set_status(Msg::None.to_ui());
        return;
    }
    let result = config.borrow_mut().save_macros(items);
    let message = match result {
        Ok(()) => match config.borrow().runtime_config(&AutoSwitchContext::current()) {
            Ok(runtime) => {
                match lhc_core::mapper::runtime::update_config_if_running(&runtime.json) {
                    Ok(()) => Msg::None,
                    Err(error) => Msg::SavedMapperNotUpdated(error),
                }
            }
            Err(error) => Msg::SavedMapperNotUpdated(error.to_string()),
        },
        Err(error) => Msg::from(&error),
    };
    editor.set_status(message.to_ui());
}

fn unique_id(draft: &[Entry], base: &str) -> String {
    let taken = |id: &str| {
        draft.iter().any(|e| e.item.id == id) || SYSTEM_MACROS.iter().any(|m| m.id == id)
    };
    if !taken(base) {
        return base.into();
    }
    (2..1000)
        .map(|n| format!("{base}{n}"))
        .find(|id| !taken(id))
        .unwrap_or_else(|| ids::generate("macro_"))
}

fn step(action: impl Into<String>) -> MacroStep {
    MacroStep {
        id: ids::generate("step_"),
        action: action.into(),
    }
}

pub fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let editor = ui.global::<MacroEditor>();
    editor.set_system_macros(ModelRc::new(VecModel::from(
        SYSTEM_MACROS
            .iter()
            .map(|item| SystemMacroRow {
                id: item.id.into(),
                name: item.name.into(),
                steps: item.steps.join("  ›  ").into(),
            })
            .collect::<Vec<_>>(),
    )));
    let Some(config) = config else {
        return;
    };
    let draft: Draft = Rc::new(RefCell::new(load(&config.borrow())));
    render(ui, &config.borrow(), &draft.borrow());

    let weak = ui.as_weak();
    let (cfg, d) = (config.clone(), draft.clone());
    editor.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            let document = cfg.borrow();
            *d.borrow_mut() = load(&document);
            ui.global::<MacroEditor>().set_has_errors(false);
            ui.global::<MacroEditor>().set_status(Msg::None.to_ui());
            render(&ui, &document, &d.borrow());
        }
    });

    // Structural edits: change the draft, save it, and rebuild all rows.
    let structural = {
        let weak = ui.as_weak();
        let (cfg, d) = (config.clone(), draft.clone());
        Rc::new(move |edit: &dyn Fn(&mut Vec<Entry>)| {
            let Some(ui) = weak.upgrade() else { return };
            edit(&mut d.borrow_mut());
            commit(&ui, &cfg, &d.borrow());
            render(&ui, &cfg.borrow(), &d.borrow());
        })
    };

    let s = structural.clone();
    editor.on_add(move |name| {
        s(&|draft| {
            let id = unique_id(draft, &ids::generate("macro_"));
            draft.insert(
                0,
                Entry::new(Macro {
                    id,
                    name: name.to_string(),
                    steps: vec![],
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                }),
            );
        });
    });
    let s = structural.clone();
    editor.on_clone_system(move |index, suffix| {
        let Some(system) = SYSTEM_MACROS.get(index as usize) else {
            return;
        };
        s(&|draft| {
            let id = unique_id(draft, &format!("{}Copy", system.id));
            draft.insert(
                0,
                Entry::new(Macro {
                    id,
                    name: format!("{} {suffix}", system.name),
                    steps: system.steps.iter().map(|a| step(*a)).collect(),
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                }),
            );
        });
    });
    let s = structural.clone();
    editor.on_remove(move |index| {
        s(&|draft| {
            if (index as usize) < draft.len() {
                draft.remove(index as usize);
            }
        });
    });
    let s = structural.clone();
    editor.on_move(move |index, delta| {
        s(&|draft| {
            let next = index + delta;
            if index >= 0 && next >= 0 && (next as usize) < draft.len() {
                draft.swap(index as usize, next as usize);
            }
        });
    });

    // Step edits rebuild only the steps of one macro.
    let steps_changed = {
        let weak = ui.as_weak();
        let (cfg, d) = (config.clone(), draft.clone());
        Rc::new(move |index: i32, edit: &dyn Fn(&mut Vec<MacroStep>)| {
            let Some(ui) = weak.upgrade() else { return };
            {
                let mut draft = d.borrow_mut();
                let Some(entry) = draft.get_mut(index as usize) else {
                    return;
                };
                edit(&mut entry.item.steps);
            }
            let draft = d.borrow();
            commit(&ui, &cfg, &draft);
            let rows = ui.global::<MacroEditor>().get_macros();
            if let Some(mut current) = rows.row_data(index as usize) {
                current.steps = steps(&draft[index as usize].item);
                rows.set_row_data(index as usize, current);
            }
            annotate(&ui, &cfg.borrow(), &draft);
        })
    };
    let s = steps_changed.clone();
    editor.on_add_step(move |index, action| s(index, &|steps| steps.push(step(action.as_str()))));
    let s = steps_changed.clone();
    editor.on_remove_step(move |index, at| {
        s(index, &|steps| {
            if (at as usize) < steps.len() {
                steps.remove(at as usize);
            }
        })
    });
    let s = steps_changed.clone();
    editor.on_move_step(move |index, at, delta| {
        s(index, &|steps| {
            let next = at + delta;
            if at >= 0 && next >= 0 && (at as usize) < steps.len() && (next as usize) < steps.len() {
                steps.swap(at as usize, next as usize);
            }
        })
    });

    // In-place edits keep the rows so the focused field is not recreated.
    let weak = ui.as_weak();
    let (cfg, d) = (config.clone(), draft.clone());
    editor.on_set_step(move |index, at, action| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut draft = d.borrow_mut();
            let Some(step) = draft
                .get_mut(index as usize)
                .and_then(|e| e.item.steps.get_mut(at as usize))
            else {
                return;
            };
            step.action = action.to_string();
        }
        let draft = d.borrow();
        commit(&ui, &cfg, &draft);
        let rows = ui.global::<MacroEditor>().get_macros();
        if let Some(mut current) = rows.row_data(index as usize) {
            let pause = action.trim().strip_prefix("pause:");
            match (current.steps.row_data(at as usize), pause) {
                (Some(mut item), Some(ms)) if item.pause => {
                    item.action = action.clone();
                    item.pause_ms = ms.trim().into();
                    current.steps.set_row_data(at as usize, item);
                }
                _ => {
                    current.steps = steps(&draft[index as usize].item);
                    rows.set_row_data(index as usize, current);
                }
            }
        }
        annotate(&ui, &cfg.borrow(), &draft);
    });
    let weak = ui.as_weak();
    editor.on_set_field(move |index, field, value| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut draft = draft.borrow_mut();
            let Some(entry) = draft.get_mut(index as usize) else {
                return;
            };
            match field {
                0 => entry.item.id = value.to_string(),
                1 => entry.item.name = value.to_string(),
                2 => entry.step_pause = value.to_string(),
                _ => entry.modifier_delay = value.to_string(),
            }
        }
        let draft = draft.borrow();
        commit(&ui, &config, &draft);
        annotate(&ui, &config.borrow(), &draft);
    });
}
