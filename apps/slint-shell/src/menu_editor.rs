use crate::{
    i18n::Msg,
    ui::{CommandRow, EmojiCategory, MenuCell, MenuEditor, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        ids,
        menus::{command_issue, empty_quick_action},
        model::*,
    },
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

const PAGE: usize = LEFT_HAND_HOTKEYS.len();

struct Draft {
    baseline: LayoutPreset,
    layout: LayoutPreset,
}

#[derive(serde::Deserialize)]
struct Category {
    id: String,
    items: Vec<String>,
}

fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}

fn key_label(key: &str) -> SharedString {
    key.trim_start_matches("Key").into()
}

fn normalize(layout: &mut LayoutPreset) {
    if layout.emoji_pages.is_empty() {
        layout.emoji_pages.push(EmojiPage {
            id: ids::generate("emoji_"),
            name: "1".into(),
            cells: Default::default(),
        });
    }
    let count = layout
        .quick_actions
        .len()
        .div_ceil(PAGE)
        .max(layout.quick_action_pages.len())
        .max(1);
    while layout.quick_action_pages.len() < count {
        layout.quick_action_pages.push(QuickActionPage {
            id: ids::generate("page_"),
            name: (layout.quick_action_pages.len() + 1).to_string(),
        });
    }
    layout
        .quick_actions
        .resize_with(count * PAGE, empty_quick_action);
}

fn page_count(e: &MenuEditor, layout: &LayoutPreset) -> usize {
    match e.get_kind() {
        0 => layout.emoji_pages.len(),
        1 => layout.quick_action_pages.len(),
        _ => 0,
    }
}

fn first_filled(layout: &LayoutPreset, page: usize) -> i32 {
    layout.quick_actions[page * PAGE..(page + 1) * PAGE]
        .iter()
        .position(|item| !item.action.trim().is_empty())
        .map_or(-1, |i| i as i32)
}

fn command_row(config: &AppConfig, commands: &[Command], index: usize) -> CommandRow {
    let item = &commands[index];
    CommandRow {
        id: item.id.clone().into(),
        name: item.name.clone().into(),
        linux: item.linux.clone().into(),
        usage: strings(lhc_core::profile::macros::action_usage(
            config,
            &format!("cmd:{}", item.id),
        )),
        error: command_issue(commands, index)
            .map_or(Msg::None, Msg::MenuIssue)
            .to_ui(),
    }
}

fn config_of(layout: &LayoutPreset) -> AppConfig {
    AppConfig::from_parts(AppSettings::default(), layout.clone(), None)
}

/// Push the draft to the UI. `fields` also resets the bound text fields,
/// which must not happen while the user types into them.
fn refresh(ui: &SettingsWindow, draft: &Draft, fields: bool) {
    let e = ui.global::<MenuEditor>();
    let l = &draft.layout;
    if e.get_kind() == 2 {
        let config = config_of(l);
        e.set_commands(ModelRc::new(VecModel::from(
            (0..l.commands.len())
                .map(|i| command_row(&config, &l.commands, i))
                .collect::<Vec<_>>(),
        )));
        e.set_has_errors((0..l.commands.len()).any(|i| command_issue(&l.commands, i).is_some()));
        return;
    }
    let names: Vec<_> = if e.get_kind() == 0 {
        l.emoji_pages.iter().map(|p| p.name.clone()).collect()
    } else {
        l.quick_action_pages.iter().map(|p| p.name.clone()).collect()
    };
    let p = (e.get_selected_page().max(0) as usize).min(names.len().saturating_sub(1));
    e.set_selected_page(p as i32);
    e.set_pages(strings(names.clone()));
    let c = e.get_selected_cell().clamp(-1, PAGE as i32 - 1);
    let cells: Vec<MenuCell> = if e.get_kind() == 0 {
        let c = c.max(0);
        e.set_selected_cell(c);
        let page = &l.emoji_pages[p];
        if fields {
            e.set_value(
                page.cells
                    .get(LEFT_HAND_HOTKEYS[c as usize])
                    .cloned()
                    .unwrap_or_default()
                    .into(),
            );
        }
        LEFT_HAND_HOTKEYS
            .iter()
            .map(|key| MenuCell {
                key: key_label(key),
                value: page.cells.get(*key).cloned().unwrap_or_default().into(),
                name: SharedString::new(),
            })
            .collect()
    } else {
        e.set_selected_cell(c);
        if fields {
            let item = (c >= 0).then(|| &l.quick_actions[p * PAGE + c as usize]);
            e.set_value(item.map(|i| i.action.clone()).unwrap_or_default().into());
            e.set_name(item.map(|i| i.name.clone()).unwrap_or_default().into());
        }
        LEFT_HAND_HOTKEYS
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let item = &l.quick_actions[p * PAGE + i];
                MenuCell {
                    key: key_label(key),
                    value: item.action.clone().into(),
                    name: if item.name.is_empty() {
                        item.action.clone()
                    } else {
                        item.name.clone()
                    }
                    .into(),
                }
            })
            .collect()
    };
    if fields {
        e.set_page_name(names[p].clone().into());
    }
    e.set_cells(ModelRc::new(VecModel::from(cells)));
}

/// Refresh command errors and usage in place, keeping focus in the rows.
fn annotate_commands(ui: &SettingsWindow, draft: &Draft) {
    let e = ui.global::<MenuEditor>();
    let commands = &draft.layout.commands;
    let config = config_of(&draft.layout);
    let rows = e.get_commands();
    for index in 0..commands.len() {
        if let Some(mut row) = rows.row_data(index) {
            let fresh = command_row(&config, commands, index);
            row.usage = fresh.usage;
            row.error = fresh.error;
            rows.set_row_data(index, row);
        }
    }
    e.set_has_errors((0..commands.len()).any(|i| command_issue(commands, i).is_some()));
}

/// Save the draft like the Tauri auto-save; invalid commands wait for a fix.
fn commit(ui: &SettingsWindow, config: &Rc<RefCell<ConfigDocument>>, draft: &mut Draft) {
    let e = ui.global::<MenuEditor>();
    let commands = &draft.layout.commands;
    if (0..commands.len()).any(|i| command_issue(commands, i).is_some()) {
        e.set_status(Msg::None.to_ui());
        return;
    }
    if draft.layout == draft.baseline {
        return;
    }
    let result = config
        .borrow_mut()
        .save_menu_pages(&draft.baseline, &draft.layout);
    match result {
        Ok(()) => {
            draft.baseline = config.borrow().layout().clone();
            saved(ui, &config.borrow());
        }
        Err(error) => e.set_status(Msg::from(&error).to_ui()),
    }
}

fn action_name(label: &str, action: &str) -> String {
    if label.is_empty() {
        action.into()
    } else {
        label.into()
    }
}

pub fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let Some(config) = config else {
        return;
    };
    let draft = Rc::new(RefCell::new(Draft {
        baseline: config.borrow().layout().clone(),
        layout: config.borrow().layout().clone(),
    }));
    let e = ui.global::<MenuEditor>();
    e.set_trusted(config.borrow().commands_trusted());
    ui.set_commands_need_approval(!config.borrow().commands_trusted());
    e.set_catalog(ModelRc::new(VecModel::from(
        serde_json::from_str::<Vec<Category>>(include_str!("../ui/emoji-catalog.json"))
            .expect("emoji catalog")
            .into_iter()
            .map(|c| EmojiCategory {
                id: c.id.into(),
                items: strings(c.items),
            })
            .collect::<Vec<_>>(),
    )));

    // Run `edit` on the draft, save it, and push the result to the UI.
    let change = {
        let weak = ui.as_weak();
        let (cfg, d) = (config.clone(), draft.clone());
        Rc::new(
            move |fields: bool, edit: &dyn Fn(&MenuEditor, &mut LayoutPreset)| {
                let Some(ui) = weak.upgrade() else { return };
                let mut d = d.borrow_mut();
                edit(&ui.global::<MenuEditor>(), &mut d.layout);
                normalize(&mut d.layout);
                commit(&ui, &cfg, &mut d);
                refresh(&ui, &d, fields);
            },
        )
    };

    let weak = ui.as_weak();
    let (cfg, d) = (config.clone(), draft.clone());
    e.on_open(move |kind| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            d.baseline = cfg.borrow().layout().clone();
            d.layout = d.baseline.clone();
            normalize(&mut d.layout);
            let e = ui.global::<MenuEditor>();
            e.set_kind(kind);
            e.set_selected_page(0);
            e.set_selected_cell(if kind == 1 { first_filled(&d.layout, 0) } else { 0 });
            e.set_trusted(cfg.borrow().commands_trusted());
            e.set_status(Msg::None.to_ui());
            e.set_has_errors(false);
            refresh(&ui, &d, true);
        }
    });

    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_select_page(move |index| {
        if let Some(ui) = weak.upgrade() {
            let d = d.borrow();
            let e = ui.global::<MenuEditor>();
            e.set_selected_page(index);
            e.set_selected_cell(if e.get_kind() == 1 { first_filled(&d.layout, index as usize) } else { 0 });
            refresh(&ui, &d, true);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_select_cell(move |index| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<MenuEditor>().set_selected_cell(index);
            refresh(&ui, &d.borrow(), true);
        }
    });

    let c = change.clone();
    e.on_add_page(move |name| {
        c(true, &|e, layout| {
            let index = if e.get_kind() == 0 {
                layout.emoji_pages.push(EmojiPage {
                    id: ids::generate("emoji_"),
                    name: name.to_string(),
                    cells: Default::default(),
                });
                layout.emoji_pages.len() - 1
            } else {
                layout.quick_action_pages.push(QuickActionPage {
                    id: ids::generate("page_"),
                    name: name.to_string(),
                });
                layout.quick_action_pages.len() - 1
            };
            e.set_selected_page(index as i32);
            e.set_selected_cell(if e.get_kind() == 1 { -1 } else { 0 });
        });
    });
    let c = change.clone();
    e.on_remove_page(move || {
        c(true, &|e, layout| {
            let p = e.get_selected_page() as usize;
            match e.get_kind() {
                0 if p < layout.emoji_pages.len() => {
                    layout.emoji_pages.remove(p);
                }
                1 if p < layout.quick_action_pages.len() => {
                    layout.quick_action_pages.remove(p);
                    let end = ((p + 1) * PAGE).min(layout.quick_actions.len());
                    layout.quick_actions.drain((p * PAGE).min(end)..end);
                }
                _ => {}
            }
            e.set_selected_page(p.saturating_sub(1) as i32);
            e.set_selected_cell(if e.get_kind() == 1 { -1 } else { 0 });
        });
    });
    let c = change.clone();
    e.on_move_page(move |delta| {
        c(true, &|e, layout| {
            let p = e.get_selected_page();
            let n = p + delta;
            if n < 0 || n as usize >= page_count(e, layout) {
                return;
            }
            let (p, n) = (p as usize, n as usize);
            if e.get_kind() == 0 {
                layout.emoji_pages.swap(p, n);
            } else {
                layout.quick_action_pages.swap(p, n);
                for c in 0..PAGE {
                    layout.quick_actions.swap(p * PAGE + c, n * PAGE + c);
                }
            }
            e.set_selected_page(n as i32);
        });
    });
    let c = change.clone();
    e.on_rename_page(move || {
        c(false, &|e, layout| {
            let p = e.get_selected_page() as usize;
            let name = e.get_page_name().to_string();
            if e.get_kind() == 0 {
                if let Some(page) = layout.emoji_pages.get_mut(p) {
                    page.name = name;
                }
            } else if let Some(page) = layout.quick_action_pages.get_mut(p) {
                page.name = name;
            }
        });
    });
    let c = change.clone();
    e.on_set_cell(move || {
        c(false, &|e, layout| {
            let p = e.get_selected_page() as usize;
            let key = LEFT_HAND_HOTKEYS[e.get_selected_cell().clamp(0, PAGE as i32 - 1) as usize];
            let value = e.get_value().trim().to_owned();
            if let Some(page) = layout.emoji_pages.get_mut(p) {
                if value.is_empty() {
                    page.cells.remove(key);
                } else {
                    page.cells.insert(key.into(), value);
                }
            }
        });
    });
    let c = change.clone();
    e.on_set_name(move || {
        c(false, &|e, layout| {
            let index = e.get_selected_page() as usize * PAGE + e.get_selected_cell().max(0) as usize;
            if let Some(item) = layout.quick_actions.get_mut(index) {
                item.name = e.get_name().to_string();
            }
        });
    });
    let c = change.clone();
    e.on_set_action(move |cell, action, label| {
        c(true, &|e, layout| {
            let index = e.get_selected_page() as usize * PAGE + cell as usize;
            let Some(item) = layout.quick_actions.get_mut(index) else {
                return;
            };
            let automatic = item.name.trim().is_empty() || item.name == item.action;
            if item.action.trim().is_empty() {
                *item = QuickAction {
                    name: action_name(&label, &action),
                    action: action.to_string(),
                    ..empty_quick_action()
                };
            } else {
                item.action = action.to_string();
                if automatic {
                    item.name = action_name(&label, &action);
                }
            }
            e.set_selected_cell(cell);
        });
    });
    let c = change.clone();
    e.on_clear_cell(move || {
        c(true, &|e, layout| {
            let p = e.get_selected_page() as usize;
            let cell = e.get_selected_cell().clamp(0, PAGE as i32 - 1) as usize;
            if e.get_kind() == 0 {
                if let Some(page) = layout.emoji_pages.get_mut(p) {
                    page.cells.remove(LEFT_HAND_HOTKEYS[cell]);
                }
            } else if let Some(item) = layout.quick_actions.get_mut(p * PAGE + cell) {
                *item = empty_quick_action();
                e.set_selected_cell(-1);
            }
        });
    });
    let c = change.clone();
    e.on_move_cell(move |from, to| {
        c(true, &|e, layout| {
            if !(0..PAGE as i32).contains(&from) || !(0..PAGE as i32).contains(&to) || from == to {
                return;
            }
            let (from, to) = (from as usize, to as usize);
            let p = e.get_selected_page() as usize;
            if e.get_kind() == 0 {
                let Some(page) = layout.emoji_pages.get_mut(p) else {
                    return;
                };
                let mut values: Vec<_> = LEFT_HAND_HOTKEYS
                    .iter()
                    .map(|key| page.cells.remove(*key))
                    .collect();
                let item = values.remove(from);
                values.insert(to, item);
                for (key, value) in LEFT_HAND_HOTKEYS.iter().zip(values) {
                    if let Some(value) = value {
                        page.cells.insert((*key).into(), value);
                    }
                }
            } else {
                let item = layout.quick_actions.remove(p * PAGE + from);
                layout.quick_actions.insert(p * PAGE + to, item);
            }
            e.set_selected_cell(to as i32);
        });
    });

    let c = change.clone();
    e.on_add_command(move |name| {
        c(true, &|_, layout| {
            let mut id = ids::generate("cmd_");
            while layout.commands.iter().any(|c| c.id == id) {
                id = ids::generate("cmd_");
            }
            layout.commands.insert(
                0,
                Command {
                    id,
                    name: name.to_string(),
                    linux: String::new(),
                },
            );
        });
    });
    let c = change.clone();
    e.on_remove_command(move |index| {
        c(true, &|_, layout| {
            if (index as usize) < layout.commands.len() {
                layout.commands.remove(index as usize);
            }
        });
    });
    let c = change.clone();
    e.on_move_command(move |index, delta| {
        c(true, &|_, layout| {
            let next = index + delta;
            if index >= 0 && next >= 0 && (next as usize) < layout.commands.len() {
                layout.commands.swap(index as usize, next as usize);
            }
        });
    });
    let weak = ui.as_weak();
    let (cfg, d) = (config.clone(), draft.clone());
    e.on_set_command(move |index, field, value| {
        let Some(ui) = weak.upgrade() else { return };
        let mut d = d.borrow_mut();
        let Some(item) = d.layout.commands.get_mut(index as usize) else {
            return;
        };
        match field {
            0 => item.id = value.to_string(),
            1 => item.name = value.to_string(),
            _ => item.linux = value.to_string(),
        }
        commit(&ui, &cfg, &mut d);
        annotate_commands(&ui, &d);
    });

    let weak = ui.as_weak();
    e.on_trust(move |approve| {
        if let Some(ui) = weak.upgrade() {
            let d = draft.borrow();
            if approve && d.layout.commands != config.borrow().layout().commands {
                ui.global::<MenuEditor>()
                    .set_status(Msg::MenuSaveFirst.to_ui());
                return;
            }
            let result = config.borrow_mut().trust_commands(approve);
            match result {
                Ok(()) => saved(&ui, &config.borrow()),
                Err(error) => ui
                    .global::<MenuEditor>()
                    .set_status(Msg::from(&error).to_ui()),
            }
        }
    });
}

fn saved(ui: &SettingsWindow, document: &ConfigDocument) {
    let e = ui.global::<MenuEditor>();
    e.set_trusted(document.commands_trusted());
    ui.set_commands_need_approval(!document.commands_trusted());
    let result = document
        .runtime_config(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
        .map_err(|e| e.to_string())
        .and_then(|r| lhc_core::mapper::runtime::update_config_if_running(&r.json));
    e.set_status(
        match result {
            Ok(()) => Msg::None,
            Err(error) => {
                let _ = lhc_core::mapper::runtime::stop();
                Msg::SavedMapperNotUpdated(error)
            }
        }
        .to_ui(),
    );
}
