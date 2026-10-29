//! Macros page. The page edits a draft; the draft is saved whenever every
//! macro in it is valid. Structural changes save at once, typing is saved
//! after a short pause so a keystroke does not rewrite the layout file.

use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{BuiltinMacroRow, MacroEditor, MacroField, MacroRow, MacroStepRow, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigError,
    mapper::builtin_macros::BUILTIN_MACROS,
    profile::{
        actions::Action,
        ids, macros,
        model::{Command, Macro, MacroStep, ResolvedAppConfig},
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// A macro being edited plus the raw delay fields, which may not parse yet.
#[derive(Clone, PartialEq)]
struct Entry {
    item: Macro,
    commands: Vec<Command>,
    /// Id on disk, `None` until first saved; used to follow id renames.
    saved_id: Option<String>,
    step_pause: String,
    modifier_delay: String,
}

impl Entry {
    fn new(item: Macro) -> Self {
        let text = |v: Option<u64>| v.map(|v| v.to_string()).unwrap_or_default();
        Self {
            step_pause: text(item.step_pause_ms),
            modifier_delay: text(item.modifier_delay_ms),
            saved_id: None,
            commands: Vec::new(),
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
    lhc_core::profile::timing::MACRO_DELAY
        .parse(value)
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
        .map(|item| Entry {
            saved_id: Some(item.id.clone()),
            ..Entry::new(item)
        })
        .collect()
}

#[derive(Default)]
pub(super) struct State {
    draft: Vec<Entry>,
    baseline: Vec<Entry>,
    /// Pending save of typed changes.
    autosave: Autosave,
}

type Shared = Rc<RefCell<State>>;

impl super::autosave::Draft for State {
    fn autosave(&self) -> &Autosave {
        &self.autosave
    }

    fn save(&mut self, ui: &SettingsWindow, document: &Document) {
        save(ui, document, self);
    }
}

/// The configuration with the draft macros, used for validation and usage.
fn candidate(document: &Document, draft: &[Entry]) -> ResolvedAppConfig {
    let mut config = document.read().config();
    config.macros = draft.iter().map(|e| e.item.clone()).collect();
    for command in draft.iter().flat_map(|entry| &entry.commands) {
        super::stage_command(&mut config.commands, command.clone());
    }
    config
}

fn issue(config: &ResolvedAppConfig, entry: &Entry) -> Option<ConfigError> {
    let Some(item) = entry.saved() else {
        return Some(ConfigError::Macro(macros::MacroIssue::DelayRange));
    };
    macros::validate(config, &item).err()
}

fn steps(item: &Macro) -> ModelRc<MacroStepRow> {
    ModelRc::new(VecModel::from(
        item.steps
            .iter()
            .map(|step| {
                let pause = match Action::parse(Some(&step.action)) {
                    Action::Pause(ms) => Some(ms),
                    _ => None,
                };
                MacroStepRow {
                    action: step.action.clone().into(),
                    pause: pause.is_some(),
                    pause_ms: pause.unwrap_or_default().into(),
                    category: super::picker::category_for_value(&step.action),
                }
            })
            .collect::<Vec<_>>(),
    ))
}

fn row(config: &ResolvedAppConfig, entry: &Entry) -> MacroRow {
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
    ui.global::<MacroEditor>()
        .set_has_errors(draft.iter().any(|e| issue(&config, e).is_some()));
    for (index, entry) in draft.iter().enumerate() {
        if let Some(current) = rows.row_data(index) {
            let next = row(&config, entry);
            // Models compare by identity, so compare usage by content. Skipping
            // unchanged rows matters for the interpreter (live preview): it wraps
            // the step model anew on every row update, which recreates the step
            // rows and drops their focus.
            let unchanged = next.usage.iter().eq(current.usage.iter())
                && MacroRow {
                    steps: current.steps.clone(),
                    usage: current.usage.clone(),
                    ..next.clone()
                } == current;
            if !unchanged {
                rows.set_row_data(
                    index,
                    MacroRow {
                        steps: current.steps,
                        ..next
                    },
                );
            }
        }
    }
}

enum SaveResult {
    Invalid,
    Unchanged,
    Saved(crate::document::Saved<()>),
}

/// Persist the draft without reading or updating presentation state.
fn persist(document: &Document, state: &mut State) -> Result<SaveResult, ConfigError> {
    for entry in &mut state.draft {
        entry.commands.retain(|command| {
            entry.item.steps.iter().any(|step| {
                matches!(Action::parse(Some(&step.action)), Action::Command(id) if id == command.id)
            })
        });
    }
    let config = candidate(document, &state.draft);
    let valid = state.draft.iter().all(|e| issue(&config, e).is_none());
    if !valid {
        return Ok(SaveResult::Invalid);
    }
    let items: Vec<Macro> = state.draft.iter().filter_map(Entry::saved).collect();
    let commands: Vec<_> = state
        .draft
        .iter()
        .flat_map(|entry| entry.commands.clone())
        .collect();
    if commands.is_empty() && document.read().layout().macros == items {
        state.baseline = state.draft.clone();
        return Ok(SaveResult::Unchanged);
    }
    let renames: Vec<(String, String)> = state
        .draft
        .iter()
        .filter_map(|e| Some((e.saved_id.clone()?, e.item.id.clone())))
        .collect();
    let edit = document.edit(View::Macros, |config| {
        config.edit_layout_with_commands(commands, |config| config.save_macros(items, &renames))
    });
    if edit.is_ok() {
        for entry in &mut state.draft {
            entry.saved_id = Some(entry.item.id.clone());
            entry.commands.clear();
        }
        state.baseline = state.draft.clone();
    }
    edit.map(SaveResult::Saved)
}

/// Save the draft when every macro is valid, then display the result.
fn save(ui: &SettingsWindow, document: &Document, state: &mut State) {
    state.autosave.cancel();
    let result = persist(document, state);
    let editor = ui.global::<MacroEditor>();
    editor.set_has_errors(matches!(result, Ok(SaveResult::Invalid)));
    editor.set_has_unsaved_changes(state.draft != state.baseline);
    let message = match result {
        Ok(SaveResult::Saved(saved)) => saved.message(Msg::None),
        Ok(_) => Msg::None,
        Err(error) => Msg::from(&error),
    };
    editor.set_status(crate::notifications::report(ui, &message));
}

fn taken(draft: &[Entry], id: &str) -> bool {
    draft.iter().any(|e| e.item.id == id)
}

fn unique_id(draft: &[Entry], base: &str) -> String {
    if !taken(draft, base) {
        return base.into();
    }
    (2..1000)
        .map(|n| format!("{base}{n}"))
        .find(|id| !taken(draft, id))
        .unwrap_or_else(|| ids::generate(base))
}

/// Short id for a new macro: the first free `m1`, `m2`, …
fn new_id(draft: &[Entry]) -> String {
    (1..)
        .map(|n| format!("m{n}"))
        .find(|id| !taken(draft, id))
        .unwrap_or_default()
}

fn step(action: impl Into<String>) -> MacroStep {
    MacroStep {
        id: ids::generate("step-"),
        action: action.into(),
    }
}

use super::{autosave::Autosave, index};

/// Row of the macro whose step the picker edits.
fn picker_macro(ui: &SettingsWindow) -> Option<crate::ui::MacroRow> {
    let editor = ui.global::<MacroEditor>();
    editor
        .get_macros()
        .row_data(editor.get_picker_macro().max(0) as usize)
}

/// Id of the macro whose step the picker edits; a macro cannot call itself.
pub(super) fn picker_macro_id(ui: &SettingsWindow) -> String {
    picker_macro(ui)
        .map(|row| row.id.into())
        .unwrap_or_default()
}

/// Step pause set on the macro whose step the picker edits.
pub(super) fn picker_macro_pause(ui: &SettingsWindow) -> Option<u64> {
    picker_macro(ui).and_then(|row| row.step_pause.trim().parse().ok())
}

pub(super) fn command(state: &RefCell<State>, id: &str) -> Option<Command> {
    state
        .borrow()
        .draft
        .iter()
        .flat_map(|entry| &entry.commands)
        .find(|command| command.id == id)
        .cloned()
}

/// Apply the picker to the page draft; commands are committed with the macros.
pub(super) fn choose(
    ui: &SettingsWindow,
    document: &Document,
    state: &RefCell<State>,
    step_index: i32,
    action: &str,
    command: Option<Command>,
) -> Result<(), Msg> {
    let mut state = state.borrow_mut();
    choose_step(
        &mut state,
        ui.global::<MacroEditor>().get_picker_macro(),
        step_index,
        action,
        command,
    )?;
    save(ui, document, &mut state);
    render(ui, document, &state.draft);
    Ok(())
}

fn choose_step(
    state: &mut State,
    at: i32,
    step_index: i32,
    action: &str,
    command: Option<Command>,
) -> Result<(), Msg> {
    let at = index(at, state.draft.len()).ok_or(Msg::from(&ConfigError::EditConflict))?;
    let entry = &mut state.draft[at];
    if step_index == -1 {
        entry.item.steps.push(step(action));
    } else {
        let index = index(step_index, entry.item.steps.len())
            .ok_or(Msg::from(&ConfigError::EditConflict))?;
        entry.item.steps[index].action = action.into();
    }
    if let Some(command) = command {
        super::stage_command(&mut entry.commands, command);
    }
    Ok(())
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) -> Shared {
    let editor = ui.global::<MacroEditor>();
    editor.set_builtin_macros(ModelRc::new(VecModel::from(
        BUILTIN_MACROS
            .iter()
            .map(|item| BuiltinMacroRow {
                id: item.id.into(),
                name: Msg::BuiltinMacro(item.id.into()).to_ui(),
                steps: item.steps.join("  ›  ").into(),
            })
            .collect::<Vec<_>>(),
    )));
    let state: Shared = Rc::new(RefCell::new(State {
        baseline: load(document),
        draft: load(document),
        autosave: Autosave::default(),
    }));
    render(ui, document, &state.borrow().draft);
    on!(ui, editor.on_flush_pending, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        save(&ui, &doc, &mut state);
        state.draft == state.baseline && !ui.global::<MacroEditor>().get_has_errors()
    });

    // Other pages change macro usage; a reload replaces a draft without
    // unsaved edits.
    super::autosave::subscribe(ui, document, View::Macros, &state, |ui, document, state| {
        let loaded = load(document);
        if state.draft == state.baseline && state.baseline != loaded {
            state.draft = loaded;
            state.baseline = state.draft.clone();
            ui.global::<MacroEditor>().set_has_errors(false);
            render(ui, document, &state.draft);
        } else {
            annotate(ui, document, &state.draft);
        }
    });

    on!(ui, editor.on_refresh, doc = document, shared = state => |ui| {
        let mut state = shared.borrow_mut();
        state.autosave.cancel();
        state.draft = load(&doc);
        state.baseline = state.draft.clone();
        let editor = ui.global::<MacroEditor>();
        editor.set_has_errors(false);
        editor.set_has_unsaved_changes(false);
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
            let id = new_id(draft);
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
    editor.on_clone_builtin(move |index, name| {
        let Some(builtin) = usize::try_from(index)
            .ok()
            .and_then(|i| BUILTIN_MACROS.get(i))
        else {
            return;
        };
        s(&|draft| {
            let id = unique_id(draft, &format!("{}Copy", builtin.id));
            draft.insert(
                0,
                Entry::new(Macro {
                    id,
                    name: name.to_string(),
                    steps: builtin.steps.iter().map(|a| step(*a)).collect(),
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
            super::move_item(draft, at, target);
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
                super::move_item(&mut entry.item.steps, step_index, target);
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
    on!(ui, editor.on_set_pause, doc = document, shared = state => |ui, at, step_index, ms| {
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
            step.action = Action::Pause(ms.to_string()).format().unwrap_or_default();
            ui.global::<MacroEditor>().set_has_unsaved_changes(state.draft != state.baseline);
            annotate(&ui, &doc, &state.draft);
        }
        super::autosave::schedule(&ui, &doc, &shared);
    });
    on!(ui, editor.on_set_field, doc = document, state = state => |ui, at, field, value| {
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
            ui.global::<MacroEditor>().set_has_unsaved_changes(state.draft != state.baseline);
            annotate(&ui, &doc, &state.draft);
        }
        super::autosave::schedule(&ui, &doc, &state);
    });
    state
}

#[cfg(test)]
#[path = "macros/tests.rs"]
mod tests;
