use crate::{
    i18n::Msg,
    ui::{MacroEditor, MacroRow, MacroStepRow, SettingsWindow},
};
use lhc_core::{
    config_document::{ConfigDocument, ConfigError},
    mapper::system_macros::SYSTEM_MACROS,
    profile::{
        actions,
        auto_switch::AutoSwitchContext,
        ids, macros,
        model::{LayoutPreset, Macro, MacroStep},
    },
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn refresh(ui: &SettingsWindow, document: &ConfigDocument) {
    let editor = ui.global::<MacroEditor>();
    editor.set_macros(ModelRc::new(VecModel::from(
        document
            .layout()
            .macros
            .iter()
            .map(|item| MacroRow {
                id: item.id.clone().into(),
                name: item.name.clone().into(),
                steps: item
                    .steps
                    .iter()
                    .map(|s| s.action.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ")
                    .into(),
            })
            .collect::<Vec<_>>(),
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
    let choices = actions::catalog(&document.config());
    editor.set_action_choices(ModelRc::new(VecModel::from(
        choices
            .into_iter()
            .map(|choice| choice.action.format().unwrap_or_default().into())
            .collect::<Vec<slint::SharedString>>(),
    )));
}

fn draft(ui: &SettingsWindow, document: &ConfigDocument, index: i32, item: Macro) {
    let editor = ui.global::<MacroEditor>();
    editor.set_selected(index);
    editor.set_macro_id(item.id.clone().into());
    editor.set_name(item.name.into());
    editor.set_step_pause(
        item.step_pause_ms
            .map(|v| v.to_string())
            .unwrap_or_default()
            .into(),
    );
    editor.set_modifier_delay(
        item.modifier_delay_ms
            .map(|v| v.to_string())
            .unwrap_or_default()
            .into(),
    );
    editor.set_steps(ModelRc::new(VecModel::from(
        item.steps
            .into_iter()
            .map(|s| MacroStepRow {
                action: s.action.into(),
            })
            .collect::<Vec<_>>(),
    )));
    editor.set_usage(
        macros::usage(&document.config(), &item.id)
            .join(", ")
            .into(),
    );
    editor.set_status(Msg::None.to_ui());
    editor.set_editing(true);
}

fn changed(ui: &SettingsWindow, document: &ConfigDocument) {
    refresh(ui, document);
    let message = match document.runtime_config(&AutoSwitchContext::current()) {
        Ok(runtime) => match lhc_core::mapper::runtime::update_config_if_running(&runtime.json) {
            Ok(()) => Msg::MacroSaved,
            Err(error) => Msg::SavedMapperNotUpdated(error),
        },
        Err(error) => Msg::SavedMapperNotUpdated(error.to_string()),
    };
    ui.global::<MacroEditor>().set_status(message.to_ui());
}

pub fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let editor = ui.global::<MacroEditor>();
    editor.set_system_macros(ModelRc::new(VecModel::from(
        SYSTEM_MACROS
            .iter()
            .map(|item| MacroRow {
                id: item.id.into(),
                name: item.name.into(),
                steps: item.steps.join(" → ").into(),
            })
            .collect::<Vec<_>>(),
    )));
    let baseline = Rc::new(RefCell::new(None::<LayoutPreset>));
    if let Some(config) = &config {
        refresh(ui, &config.borrow());
    }
    let weak = ui.as_weak();
    let cfg = config.clone();
    editor.on_refresh(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &cfg) {
            refresh(&ui, &config.borrow());
        }
    });
    let weak = ui.as_weak();
    let cfg = config.clone();
    let base = baseline.clone();
    editor.on_edit(move |index| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &cfg) {
            let document = config.borrow();
            let item = if index < 0 {
                Macro {
                    id: ids::generate("macro_"),
                    name: String::new(),
                    steps: vec![],
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                }
            } else {
                let Some(item) = document.layout().macros.get(index as usize) else {
                    return;
                };
                item.clone()
            };
            *base.borrow_mut() = Some(document.layout().clone());
            draft(&ui, &document, index, item);
        }
    });
    let weak = ui.as_weak();
    let cfg = config.clone();
    let base = baseline.clone();
    editor.on_clone_system(move |index| {
        if let (Some(ui), Some(config), Some(system)) =
            (weak.upgrade(), &cfg, SYSTEM_MACROS.get(index as usize))
        {
            let document = config.borrow();
            *base.borrow_mut() = Some(document.layout().clone());
            draft(
                &ui,
                &document,
                -1,
                Macro {
                    id: ids::generate("macro_"),
                    name: system.name.into(),
                    steps: system
                        .steps
                        .iter()
                        .map(|action| MacroStep {
                            id: ids::generate("step_"),
                            action: (*action).into(),
                        })
                        .collect(),
                    step_pause_ms: None,
                    modifier_delay_ms: None,
                },
            );
        }
    });
    let weak = ui.as_weak();
    let cfg = config.clone();
    editor.on_save(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &cfg) {
            let editor = ui.global::<MacroEditor>();
            if baseline.borrow().as_ref() != Some(config.borrow().layout()) {
                editor.set_status(Msg::MacroDraftChanged.to_ui());
                return;
            }
            let parse_delay = |value: slint::SharedString| -> Result<Option<u64>, ()> {
                if value.trim().is_empty() {
                    Ok(None)
                } else {
                    value
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .filter(|v| *v <= 2000)
                        .map(Some)
                        .ok_or(())
                }
            };
            let (Ok(step_pause_ms), Ok(modifier_delay_ms)) = (
                parse_delay(editor.get_step_pause()),
                parse_delay(editor.get_modifier_delay()),
            ) else {
                editor.set_status(Msg::MacroIssue(macros::MacroIssue::DelayRange).to_ui());
                return;
            };
            let item = Macro {
                id: editor.get_macro_id().to_string(),
                name: editor.get_name().to_string(),
                steps: editor
                    .get_steps()
                    .iter()
                    .map(|step| MacroStep {
                        id: ids::generate("step_"),
                        action: step.action.to_string(),
                    })
                    .collect(),
                step_pause_ms,
                modifier_delay_ms,
            };
            let selected = editor.get_selected();
            let result = config
                .borrow_mut()
                .save_macro((selected >= 0).then_some(selected as usize), item);
            match result {
                Ok(()) => {
                    editor.set_editing(false);
                    changed(&ui, &config.borrow());
                }
                Err(error) => editor.set_status(Msg::from(&error).to_ui()),
            }
        }
    });
    let weak = ui.as_weak();
    let cfg = config.clone();
    editor.on_remove(move |id| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &cfg) {
            let Some(index) = config
                .borrow()
                .layout()
                .macros
                .iter()
                .position(|item| item.id == id.as_str())
            else {
                return;
            };
            let result = config.borrow_mut().remove_macro(index);
            finish(&ui, config, result);
        }
    });
    let weak = ui.as_weak();
    editor.on_move(move |index, delta| {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config) {
            let result = config
                .borrow_mut()
                .move_macro(index as usize, (index + delta) as usize);
            finish(&ui, config, result);
        }
    });
    let weak = ui.as_weak();
    editor.on_add_step(move |action| {
        if let Some(ui) = weak.upgrade() {
            let editor = ui.global::<MacroEditor>();
            let mut steps: Vec<_> = editor.get_steps().iter().collect();
            steps.push(MacroStepRow { action });
            editor.set_steps(ModelRc::new(VecModel::from(steps)));
        }
    });
    let weak = ui.as_weak();
    editor.on_remove_step(move |index| {
        if let Some(ui) = weak.upgrade() {
            let editor = ui.global::<MacroEditor>();
            let mut steps: Vec<_> = editor.get_steps().iter().collect();
            if index >= 0 && (index as usize) < steps.len() {
                steps.remove(index as usize);
            }
            editor.set_steps(ModelRc::new(VecModel::from(steps)));
        }
    });
    let weak = ui.as_weak();
    editor.on_move_step(move |index, delta| {
        if let Some(ui) = weak.upgrade() {
            let editor = ui.global::<MacroEditor>();
            let mut steps: Vec<_> = editor.get_steps().iter().collect();
            let next = index + delta;
            if index >= 0
                && next >= 0
                && (index as usize) < steps.len()
                && (next as usize) < steps.len()
            {
                steps.swap(index as usize, next as usize);
            }
            editor.set_steps(ModelRc::new(VecModel::from(steps)));
        }
    });
}

fn finish(
    ui: &SettingsWindow,
    config: &Rc<RefCell<ConfigDocument>>,
    result: Result<(), ConfigError>,
) {
    match result {
        Ok(()) => changed(ui, &config.borrow()),
        Err(error) => ui
            .global::<MacroEditor>()
            .set_status(Msg::from(&error).to_ui()),
    }
}
