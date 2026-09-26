use crate::{editor::EditorHandle, ui::SettingsWindow};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{auto_switch::AutoSwitchContext, layout_file, model::LayoutPreset},
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

struct Library {
    names: Rc<VecModel<SharedString>>,
    selected: Option<String>,
    known: Option<String>,
}

fn refresh(
    ui: &SettingsWindow,
    document: &ConfigDocument,
    state: &mut Library,
) -> Result<(), String> {
    let names = document.paths().list_user_layouts()?;
    let selected = state
        .selected
        .as_ref()
        .and_then(|name| names.iter().position(|item| item == name));
    state
        .names
        .set_vec(names.into_iter().map(Into::into).collect::<Vec<_>>());
    ui.set_selected_layout(selected.map_or(-1, |index| index as i32));
    if selected.is_none() {
        state.selected = None;
        state.known = None;
        ui.set_layout_description("".into());
    }
    Ok(())
}

fn report(ui: &SettingsWindow, result: Result<String, String>) {
    ui.set_layout_status(match result {
        Ok(message) => message.into(),
        Err(error) => format!("Error: {error}").into(),
    });
}

pub(super) fn bind(
    ui: &SettingsWindow,
    config: Option<Rc<RefCell<ConfigDocument>>>,
    editor: EditorHandle,
) {
    let state = Rc::new(RefCell::new(Library {
        names: Rc::new(VecModel::default()),
        selected: None,
        known: None,
    }));
    ui.set_layout_names(ModelRc::new(state.borrow().names.clone()));
    if let Some(config) = &config {
        report(
            ui,
            refresh(ui, &config.borrow(), &mut state.borrow_mut()).map(|_| String::new()),
        );
    }
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_refresh_layouts(move || {
        if let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) {
            report(
                &ui,
                refresh(&ui, &config.borrow(), &mut state_copy.borrow_mut()).map(|_| String::new()),
            );
        }
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_select_layout(move |index| {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let Some(name) = usize::try_from(index)
            .ok()
            .and_then(|i| state_copy.borrow().names.row_data(i))
        else {
            return;
        };
        let name = name.to_string();
        let result = config
            .borrow()
            .paths()
            .load_user_layout(&name)
            .and_then(|text| {
                let layout = layout_file::parse(&text)?.ok_or("Layout is empty")?;
                let mut state = state_copy.borrow_mut();
                state.selected = Some(name.clone());
                state.known = Some(text);
                ui.set_selected_layout(index);
                ui.set_layout_name(name.clone().into());
                ui.set_layout_description(layout.description.unwrap_or_default().into());
                Ok(name)
            });
        report(&ui, result);
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_create_layout(move |name| {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let result = config
            .borrow()
            .paths()
            .save_user_layout(
                &name,
                &layout_file::serialize(&LayoutPreset::default()),
                false,
            )
            .and_then(|name| {
                refresh(&ui, &config.borrow(), &mut state_copy.borrow_mut())?;
                Ok(format!("Created: {name}"))
            });
        report(&ui, result);
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_save_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let name = ui.get_layout_name().trim().to_owned();
        let result = (|| {
            let document = config.borrow();
            let selected = state_copy.borrow().selected.clone();
            let overwrite = selected.as_deref() == Some(name.as_str());
            if overwrite {
                let current = document.paths().load_user_layout(&name)?;
                if Some(current) != state_copy.borrow().known {
                    return Err("Layout changed on disk; select it again".into());
                }
            }
            let saved = document.paths().save_user_layout(
                &name,
                &layout_file::serialize(document.layout()),
                overwrite,
            )?;
            drop(document);
            let text = config.borrow().paths().load_user_layout(&saved)?;
            {
                let mut state = state_copy.borrow_mut();
                state.selected = Some(saved.clone());
                state.known = Some(text);
                refresh(&ui, &config.borrow(), &mut state)?;
            }
            ui.set_layout_name(saved.clone().into());
            Ok(format!("Saved: {saved}"))
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    let state_copy = state.clone();
    let config_copy = config.clone();
    ui.on_load_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config_copy) else {
            return;
        };
        let result = (|| {
            let name = state_copy
                .borrow()
                .selected
                .clone()
                .ok_or("Select a layout")?;
            let text = config.borrow().paths().load_user_layout(&name)?;
            let layout = layout_file::parse(&text)?.ok_or("Layout is empty")?;
            config
                .borrow_mut()
                .update_layout(|current| *current = layout)
                .map_err(|error| error.to_string())?;
            editor.reload(&config.borrow());
            ui.invoke_refresh_rules();
            ui.invoke_refresh_layers();
            ui.global::<crate::ui::MacroEditor>().invoke_refresh();
            let runtime = config.borrow().runtime_config(&AutoSwitchContext::current()).map_err(|error| error.to_string())?;
            lhc_core::mapper::runtime::update_config_if_running(&runtime.json)?;
            ui.set_config_status(
                crate::i18n::Msg::ConfigSaved(config.borrow().layout().rules.len()).to_ui(),
            );
            Ok(format!("Loaded: {name}"))
        })();
        report(&ui, result);
    });
    let weak = ui.as_weak();
    ui.on_delete_layout(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config) else {
            return;
        };
        let result = (|| {
            let name = state.borrow().selected.clone().ok_or("Select a layout")?;
            let current = config.borrow().paths().load_user_layout(&name)?;
            if Some(current) != state.borrow().known {
                return Err("Layout changed on disk; select it again".into());
            }
            config.borrow().paths().delete_user_layout(&name)?;
            refresh(&ui, &config.borrow(), &mut state.borrow_mut())?;
            Ok(format!("Deleted: {name}"))
        })();
        report(&ui, result);
    });
}
