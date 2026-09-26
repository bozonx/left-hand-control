use crate::{
    i18n::Msg,
    ui::{MenuCell, MenuEditor, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{actions, ids, menus::empty_quick_action, model::*},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

struct Draft {
    baseline: LayoutPreset,
    layout: LayoutPreset,
}
fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
fn apply(ui: &SettingsWindow, draft: &mut Draft) {
    let e = ui.global::<MenuEditor>();
    let p = e.get_selected_page() as usize;
    let c = e.get_selected_cell() as usize;
    match e.get_kind() {
        0 => {
            if let Some(page) = draft.layout.emoji_pages.get_mut(p) {
                page.name = e.get_page_name().to_string();
                if let Some(key) = LEFT_HAND_HOTKEYS.get(c) {
                    let value = e.get_value().trim().to_owned();
                    if value.is_empty() {
                        page.cells.remove(*key);
                    } else {
                        page.cells.insert((*key).into(), value);
                    }
                }
            }
        }
        1 => {
            if let Some(page) = draft.layout.quick_action_pages.get_mut(p) {
                page.name = e.get_page_name().to_string();
                if let Some(item) = draft.layout.quick_actions.get_mut(p * 15 + c) {
                    item.name = e.get_name().to_string();
                    item.action = e.get_value().to_string();
                    if item.name.trim().is_empty() {
                        item.name = item.action.clone();
                    }
                    item.icon = (!e.get_icon().trim().is_empty()).then(|| e.get_icon().to_string());
                }
            }
        }
        _ => {
            if let Some(item) = draft.layout.commands.get_mut(p) {
                item.name = e.get_page_name().to_string();
                item.id = e.get_command_id().to_string();
                item.linux = e.get_value().to_string();
            }
        }
    }
}
fn refresh(ui: &SettingsWindow, draft: &Draft) {
    let e = ui.global::<MenuEditor>();
    let l = &draft.layout;
    let names: Vec<_> = match e.get_kind() {
        0 => l.emoji_pages.iter().map(|p| p.name.clone()).collect(),
        1 => l
            .quick_action_pages
            .iter()
            .map(|p| p.name.clone())
            .collect(),
        _ => l
            .commands
            .iter()
            .map(|p| format!("{} · {}", p.name, p.id))
            .collect(),
    };
    let p = (e.get_selected_page().max(0) as usize).min(names.len().saturating_sub(1));
    e.set_selected_page(p as i32);
    e.set_pages(strings(names));
    let c = e.get_selected_cell().clamp(0, 14) as usize;
    e.set_selected_cell(c as i32);
    let mut cells = vec![];
    e.set_value("".into());
    e.set_page_name("".into());
    e.set_name("".into());
    e.set_icon("".into());
    e.set_command_id("".into());
    if e.get_kind() == 0 {
        if let Some(page) = l.emoji_pages.get(p) {
            e.set_page_name(page.name.clone().into());
            e.set_value(
                page.cells
                    .get(LEFT_HAND_HOTKEYS[c])
                    .cloned()
                    .unwrap_or_default()
                    .into(),
            );
            cells = LEFT_HAND_HOTKEYS
                .iter()
                .map(|key| MenuCell {
                    key: key.trim_start_matches("Key").into(),
                    value: page.cells.get(*key).cloned().unwrap_or_default().into(),
                })
                .collect();
        }
    } else if e.get_kind() == 1 {
        if let Some(page) = l.quick_action_pages.get(p) {
            e.set_page_name(page.name.clone().into());
            let item = &l.quick_actions[p * 15 + c];
            e.set_value(item.action.clone().into());
            e.set_name(item.name.clone().into());
            e.set_icon(item.icon.clone().unwrap_or_default().into());
            cells = LEFT_HAND_HOTKEYS
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    let item = &l.quick_actions[p * 15 + i];
                    MenuCell {
                        key: key.trim_start_matches("Key").into(),
                        value: if item.action.is_empty() {
                            String::new()
                        } else if item.name.is_empty() {
                            item.action.clone()
                        } else {
                            item.name.clone()
                        }
                        .into(),
                    }
                })
                .collect();
        }
    } else if let Some(item) = l.commands.get(p) {
        e.set_page_name(item.name.clone().into());
        e.set_command_id(item.id.clone().into());
        let config = AppConfig::from_parts(AppSettings::default(), l.clone(), None);
        e.set_usage(
            lhc_core::profile::macros::action_usage(&config, &format!("cmd:{}", item.id))
                .join(", ")
                .into(),
        );
        e.set_value(item.linux.clone().into());
    }
    e.set_cells(ModelRc::new(VecModel::from(cells)));
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
        .div_ceil(15)
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
        .resize_with(count * 15, empty_quick_action);
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
    e.set_catalog(ModelRc::new(VecModel::from(
        serde_json::from_str::<Vec<String>>(include_str!("../ui/emoji-catalog.json"))
            .expect("emoji catalog")
            .into_iter()
            .map(Into::into)
            .collect::<Vec<slint::SharedString>>(),
    )));
    let weak = ui.as_weak();
    let cfg = config.clone();
    let d = draft.clone();
    e.on_open(move |kind| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            d.baseline = cfg.borrow().layout().clone();
            d.layout = d.baseline.clone();
            normalize(&mut d.layout);
            let e = ui.global::<MenuEditor>();
            e.set_kind(kind);
            e.set_selected_page(0);
            e.set_selected_cell(0);
            e.set_trusted(cfg.borrow().commands_trusted());
            e.set_status(Msg::None.to_ui());
            e.set_choices(strings(
                actions::catalog(&cfg.borrow().config())
                    .into_iter()
                    .filter_map(|c| c.action.format()),
            ));
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_apply(move || {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_select_page(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            ui.global::<MenuEditor>().set_selected_page(index);
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_select_cell(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            ui.global::<MenuEditor>().set_selected_cell(index);
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_add(move || {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            let e = ui.global::<MenuEditor>();
            let index = match e.get_kind() {
                0 => {
                    let name = (d.layout.emoji_pages.len() + 1).to_string();
                    d.layout.emoji_pages.push(EmojiPage {
                        id: ids::generate("emoji_"),
                        name,
                        cells: Default::default(),
                    });
                    d.layout.emoji_pages.len() - 1
                }
                1 => {
                    let name = (d.layout.quick_action_pages.len() + 1).to_string();
                    d.layout.quick_action_pages.push(QuickActionPage {
                        id: ids::generate("page_"),
                        name,
                    });
                    normalize(&mut d.layout);
                    d.layout.quick_action_pages.len() - 1
                }
                _ => {
                    d.layout.commands.push(Command {
                        id: ids::generate("cmd_"),
                        name: String::new(),
                        linux: String::new(),
                    });
                    d.layout.commands.len() - 1
                }
            };
            e.set_selected_page(index as i32);
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_remove(move || {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            let e = ui.global::<MenuEditor>();
            let p = e.get_selected_page() as usize;
            match e.get_kind() {
                0 if p < d.layout.emoji_pages.len() => {
                    d.layout.emoji_pages.remove(p);
                }
                1 if p < d.layout.quick_action_pages.len() => {
                    d.layout.quick_action_pages.remove(p);
                    d.layout.quick_actions.drain(p * 15..(p + 1) * 15);
                }
                2 if p < d.layout.commands.len() => {
                    d.layout.commands.remove(p);
                }
                _ => {}
            }
            normalize(&mut d.layout);
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_move(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            let e = ui.global::<MenuEditor>();
            let p = e.get_selected_page();
            let n = p + delta;
            let len = match e.get_kind() {
                0 => d.layout.emoji_pages.len(),
                1 => d.layout.quick_action_pages.len(),
                _ => d.layout.commands.len(),
            };
            if n >= 0 && (n as usize) < len {
                let (p, n) = (p as usize, n as usize);
                match e.get_kind() {
                    0 => d.layout.emoji_pages.swap(p, n),
                    1 => {
                        d.layout.quick_action_pages.swap(p, n);
                        for c in 0..15 {
                            d.layout.quick_actions.swap(p * 15 + c, n * 15 + c);
                        }
                    }
                    _ => d.layout.commands.swap(p, n),
                };
                e.set_selected_page(n as i32);
            }
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    e.on_move_cell(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            let e = ui.global::<MenuEditor>();
            let p = e.get_selected_page() as usize;
            let c = e.get_selected_cell();
            let n = c + delta;
            if (0..15).contains(&n) {
                let (c, n) = (c as usize, n as usize);
                if e.get_kind() == 0 {
                    if let Some(page) = d.layout.emoji_pages.get_mut(p) {
                        let a = page.cells.remove(LEFT_HAND_HOTKEYS[c]);
                        let b = page.cells.remove(LEFT_HAND_HOTKEYS[n]);
                        if let Some(a) = a {
                            page.cells.insert(LEFT_HAND_HOTKEYS[n].into(), a);
                        }
                        if let Some(b) = b {
                            page.cells.insert(LEFT_HAND_HOTKEYS[c].into(), b);
                        }
                    }
                } else if p * 15 + n < d.layout.quick_actions.len() {
                    d.layout.quick_actions.swap(p * 15 + c, p * 15 + n);
                }
                e.set_selected_cell(n as i32);
            }
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    let d = draft.clone();
    let cfg = config.clone();
    e.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            let mut d = d.borrow_mut();
            apply(&ui, &mut d);
            let result = cfg.borrow_mut().save_menu_pages(&d.baseline, &d.layout);
            match result {
                Ok(()) => {
                    d.baseline = cfg.borrow().layout().clone();
                    saved(&ui, &cfg.borrow());
                }
                Err(error) => ui
                    .global::<MenuEditor>()
                    .set_status(Msg::from(&error).to_ui()),
            }
            refresh(&ui, &d);
        }
    });
    let weak = ui.as_weak();
    e.on_trust(move |approve| {
        if let Some(ui) = weak.upgrade() {
            let mut d = draft.borrow_mut();
            apply(&ui, &mut d);
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
    let result = document
        .runtime_config(&lhc_core::profile::auto_switch::AutoSwitchContext::current())
        .map_err(|e| e.to_string())
        .and_then(|r| lhc_core::mapper::runtime::update_config_if_running(&r.json));
    e.set_status(
        match result {
            Ok(()) => Msg::MenuSaved,
            Err(error) => {
                let _ = lhc_core::mapper::runtime::stop();
                Msg::SavedMapperNotUpdated(error)
            }
        }
        .to_ui(),
    );
}
