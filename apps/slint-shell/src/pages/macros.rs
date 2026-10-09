//! Macros page. The page edits a draft; the draft is saved whenever every
//! macro in it is valid. Structural changes save at once, typing is saved
//! after a short pause so a keystroke does not rewrite the layout file.

use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{MacroEditor, MacroField, MacroRow, MacroStepRow, SettingsWindow, SystemMacroRow},
};
use lhc_core::{
    config_document::ConfigError,
    mapper::system_macros::SYSTEM_MACROS,
    profile::{
        ids, macros,
        model::{AppConfig, Macro, MacroStep},
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Delay between the last keystroke and saving it.
const TYPING_DELAY: Duration = Duration::from_millis(400);
/// Longest delay a macro may configure between steps, in milliseconds.
const MAX_DELAY_MS: u64 = 2000;

/// A macro being edited plus the raw delay fields, which may not parse yet.
#[derive(Clone, PartialEq)]
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

    /// The macro as it is saved; `None` while a delay does not parse.
    fn saved(&self) -> Option<Macro> {
        Some(Macro {
            step_pause_ms: parse_delay(&self.step_pause).ok()?,
            modifier_delay_ms: parse_delay(&self.modifier_delay).ok()?,
            ..self.item.clone()
        })
    }
}

fn parse_delay(value: &str) -> Result<Option<u64>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|v| *v <= MAX_DELAY_MS)
        .map(Some)
        .ok_or(())
}

fn load(document: &Document) -> Vec<Entry> {
    document
        .read()
        .layout()
        .macros
        .iter()
        .cloned()
        .map(Entry::new)
        .collect()
}

#[derive(Default)]
struct State {
    draft: Vec<Entry>,
    /// Pending save of typed changes.
    timer: Option<slint::Timer>,
}

type Shared = Rc<RefCell<State>>;

/// The configuration with the draft macros, used for validation and usage.
fn candidate(document: &Document, draft: &[Entry]) -> AppConfig {
    let mut config = document.read().config();
    config.macros = draft.iter().map(|e| e.item.clone()).collect();
    config
}

fn issue(config: &AppConfig, entry: &Entry) -> Option<ConfigError> {
    if entry.saved().is_none() {
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
                    category: super::picker::category_for_value(&step.action),
                }
            })
            .collect::<Vec<_>>(),
    ))
}

fn row(config: &AppConfig, entry: &Entry) -> MacroRow {
    MacroRow {
        id: entry.item.id.clone().into(),
        name: entry.item.name.clone().into(),
        step_pause: entry.step_pause.clone().into(),
        modifier_delay: entry.modifier_delay.clone().into(),
        usage: super::strings(macros::usage(config, &entry.item.id)),
        error: issue(config, entry)
            .map_or(Msg::None, |e| Msg::from(&e))
            .to_ui(),
        steps: steps(&entry.item),
    }
}

/// Rebuild every row. Used after structural changes and reloads.
fn render(ui: &SettingsWindow, document: &Document, draft: &[Entry]) {
    let editor = ui.global::<MacroEditor>();
    let config = candidate(document, draft);
    editor.set_macros(ModelRc::new(VecModel::from(
        draft.iter().map(|e| row(&config, e)).collect::<Vec<_>>(),
    )));
    let settings = document.read();
    let settings = settings.settings();
    editor.set_default_step_pause(settings.default_macro_step_pause_ms.to_string().into());
    editor.set_default_modifier_delay(settings.default_macro_modifier_delay_ms.to_string().into());
}

/// Refresh rows in place, keeping their step models, so fields keep focus
/// while typing.
fn annotate(ui: &SettingsWindow, document: &Document, draft: &[Entry]) {
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

/// Save the draft when every macro is valid.
fn save(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.timer = None;
    let editor = ui.global::<MacroEditor>();
    let config = candidate(document, &state.draft);
    let valid = state.draft.iter().all(|e| issue(&config, e).is_none());
    editor.set_has_errors(!valid);
    if !valid {
        editor.set_status(Msg::None.to_ui());
        return;
    }
    let items: Vec<Macro> = state.draft.iter().filter_map(Entry::saved).collect();
    if document.read().layout().macros == items {
        editor.set_status(Msg::None.to_ui());
        return;
    }
    let message = match document.edit(View::Macros, |config| config.save_macros(items)) {
        Ok(saved) => saved.message(Msg::None),
        Err(error) => {
            // The document reloaded the files; show what is on disk now.
            if error == ConfigError::ExternalChange {
                state.draft = load(document);
                render(ui, document, &state.draft);
            }
            Msg::from(&error)
        }
    };
    editor.set_status(crate::notifications::report(ui, &message));
}

/// Save typed changes once typing pauses.
fn save_later(ui: &SettingsWindow, document: &Rc<Document>, state: &Shared) {
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), Rc::downgrade(state));
    timer.start(slint::TimerMode::SingleShot, TYPING_DELAY, move || {
        if let (Some(ui), Some(state)) = (weak.upgrade(), shared.upgrade()) {
            save(&ui, &doc, &mut state.borrow_mut());
        }
    });
    state.borrow_mut().timer = Some(timer);
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

fn index(value: i32, len: usize) -> Option<usize> {
    usize::try_from(value).ok().filter(|index| *index < len)
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
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
    let state: Shared = Rc::new(RefCell::new(State {
        draft: load(document),
        timer: None,
    }));
    render(ui, document, &state.borrow().draft);
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    editor.on_flush_pending(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = shared.borrow_mut();
            if state.timer.is_some() {
                save(&ui, &doc, &mut state);
            }
        }
    });

    // Other pages change macro usage; a reload replaces a draft without
    // unsaved edits.
    let weak = ui.as_weak();
    let shared = state.clone();
    document.subscribe(View::Macros, move |document| {
        let Some(ui) = weak.upgrade() else { return };
        // Busy: this page's own save is running and reloads by itself.
        let Ok(mut state) = shared.try_borrow_mut() else {
            return;
        };
        let saved: Vec<Macro> = state.draft.iter().filter_map(Entry::saved).collect();
        let pending = state.timer.is_some() || saved.len() != state.draft.len();
        if !pending && saved != document.read().layout().macros {
            state.draft = load(document);
            render(&ui, document, &state.draft);
        } else {
            annotate(&ui, document, &state.draft);
        }
    });

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    editor.on_refresh(move || {
        let Some(ui) = weak.upgrade() else { return };
        let mut state = shared.borrow_mut();
        state.timer = None;
        state.draft = load(&doc);
        let editor = ui.global::<MacroEditor>();
        editor.set_has_errors(false);
        editor.set_status(Msg::None.to_ui());
        render(&ui, &doc, &state.draft);
    });

    // Structural edits: change the draft, save it and rebuild all rows.
    let structural = {
        let weak = ui.as_weak();
        let (doc, shared) = (document.clone(), state.clone());
        Rc::new(move |edit: &dyn Fn(&mut Vec<Entry>)| {
            let Some(ui) = weak.upgrade() else { return };
            let mut state = shared.borrow_mut();
            edit(&mut state.draft);
            save(&ui, &doc, &mut state);
            render(&ui, &doc, &state.draft);
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
        let Some(system) = usize::try_from(index)
            .ok()
            .and_then(|i| SYSTEM_MACROS.get(i))
        else {
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
    editor.on_remove(move |at| {
        s(&|draft| {
            if let Some(at) = index(at, draft.len()) {
                draft.remove(at);
            }
        });
    });
    let s = structural.clone();
    editor.on_move(move |at, target| {
        s(&|draft| {
            if let (Some(from), Some(to)) = (index(at, draft.len()), index(target, draft.len())) {
                let item = draft.remove(from);
                draft.insert(to, item);
            }
        });
    });
    let s = structural.clone();
    let weak = ui.as_weak();
    editor.on_add_step(move |at, action| {
        if action.trim().is_empty() {
            if let Some(ui) = weak.upgrade() {
                ui.global::<crate::ui::MacroEditor>().set_picker_macro(at);
                ui.global::<crate::ui::ActionPicker>().invoke_open(
                    crate::ui::PickerTarget::MacroStep,
                    -1,
                    "".into(),
                    false,
                );
            }
            return;
        }
        s(&|draft| {
            if let Some(at) = index(at, draft.len()) {
                draft[at].item.steps.push(step(action.as_str()));
            }
        })
    });
    let s = structural.clone();
    editor.on_remove_step(move |at, step_index| {
        s(&|draft| {
            if let Some(entry) = index(at, draft.len()).map(|at| &mut draft[at])
                && let Some(step_index) = index(step_index, entry.item.steps.len())
            {
                entry.item.steps.remove(step_index);
            }
        })
    });
    let s = structural.clone();
    editor.on_move_step(move |at, step_index, target| {
        s(&|draft| {
            if let Some(entry) = index(at, draft.len()).map(|at| &mut draft[at]) {
                let len = entry.item.steps.len();
                if let (Some(from), Some(to)) = (index(step_index, len), index(target, len)) {
                    let item = entry.item.steps.remove(from);
                    entry.item.steps.insert(to, item);
                }
            }
        })
    });
    let s = structural.clone();
    editor.on_set_step(move |at, step_index, action| {
        if action.trim().is_empty() {
            return;
        }
        s(&|draft| {
            let Some(at) = index(at, draft.len()) else {
                return;
            };
            if step_index == -1 {
                draft[at].item.steps.push(step(action.as_str()));
            } else if let Some(step) = draft[at].item.steps.get_mut(step_index.max(0) as usize) {
                step.action = action.to_string();
            }
        })
    });

    // Typing: change the draft in place so the focused field survives.
    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    editor.on_set_pause(move |at, step_index, ms| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut state = shared.borrow_mut();
            let len = state.draft.len();
            let Some(step) = index(at, len)
                .and_then(|at| {
                    state.draft[at]
                        .item
                        .steps
                        .get_mut(step_index.max(0) as usize)
                })
                .filter(|_| step_index >= 0)
            else {
                return;
            };
            step.action = format!("pause:{ms}");
            annotate(&ui, &doc, &state.draft);
        }
        save_later(&ui, &doc, &shared);
    });
    let weak = ui.as_weak();
    let doc = document.clone();
    editor.on_set_field(move |at, field, value| {
        let Some(ui) = weak.upgrade() else { return };
        {
            let mut state = state.borrow_mut();
            let len = state.draft.len();
            let Some(entry) = index(at, len).map(|at| &mut state.draft[at]) else {
                return;
            };
            let value = value.to_string();
            match field {
                MacroField::Id => entry.item.id = value,
                MacroField::Name => entry.item.name = value,
                MacroField::StepPause => entry.step_pause = value,
                MacroField::ModifierDelay => entry.modifier_delay = value,
            }
            annotate(&ui, &doc, &state.draft);
        }
        save_later(&ui, &doc, &state);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_parse_within_range() {
        assert_eq!(parse_delay(""), Ok(None));
        assert_eq!(parse_delay(" 25 "), Ok(Some(25)));
        assert!(parse_delay("2001").is_err());
        assert!(parse_delay("x").is_err());
        let entry = Entry {
            step_pause: "abc".into(),
            ..Entry::new(Macro {
                id: "m".into(),
                name: String::new(),
                steps: vec![],
                step_pause_ms: None,
                modifier_delay_ms: None,
            })
        };
        assert!(entry.saved().is_none());
    }

    #[test]
    fn ids_avoid_system_macros() {
        let system = SYSTEM_MACROS[0].id;
        assert_ne!(unique_id(&[], system), system);
    }
}
