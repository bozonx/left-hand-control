//! Real keyboard regressions for modal traversal, activation and focus restoration.
mod support;
use i_slint_core::{
    accessibility::{AccessibleStringProperty, nearest_accessible_item},
    input::FocusReason,
    item_tree::{ItemRc, ParentItemTraversalMode},
    window::WindowInner,
};
use lhc_core::storage::StoragePaths;
use slint::{ComponentHandle, Model, platform::Key};
use slint_shell::{Document, bind_document, ui::*};
use std::{cell::Cell, rc::Rc, time::Duration};
use support::snapshot;

fn key(ui: &SettingsWindow, key: impl Into<slint::SharedString>) {
    let text = key.into();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}

fn focused(ui: &SettingsWindow) -> ItemRc {
    WindowInner::from_pub(ui.window())
        .focus_item
        .borrow()
        .upgrade()
        .expect("keyboard focus")
}

fn label(ui: &SettingsWindow) -> String {
    WindowInner::from_pub(ui.window())
        .focus_item
        .borrow()
        .upgrade()
        .map(nearest_accessible_item)
        .and_then(|item| item.accessible_string_property(AccessibleStringProperty::Label))
        .unwrap_or_default()
        .to_string()
}

fn tab_to(ui: &SettingsWindow, expected: &str) {
    for _ in 0..400 {
        if label(ui) == expected {
            return;
        }
        key(ui, Key::Tab);
    }
    panic!("Tab did not reach {expected:?}; stopped on {:?}", label(ui));
}

fn check_modal(ui: &SettingsWindow) {
    WindowInner::from_pub(ui.window()).ensure_tree_instantiated();
    let mut parent = Some(focused(ui));
    let boundary = loop {
        let item = parent.expect("dialog has a named focus boundary");
        if item.accessible_role() == i_slint_core::items::AccessibleRole::Groupbox {
            break item;
        }
        parent = item.parent_item(ParentItemTraversalMode::FindAllParents);
    };
    // A removed dynamic control may leave no focused item before the next key.
    let current = focused(ui);
    WindowInner::from_pub(ui.window()).set_focus_item(&current, false, FocusReason::Programmatic);
    // Traversal must not reach global navigation or controls of the settings page.
    for backwards in [false, true] {
        if backwards {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: Key::Shift.into(),
                });
        }
        for _ in 0..180 {
            key(ui, Key::Tab);
            let name = label(ui);
            let mut current = Some(focused(ui));
            let mut inside = false;
            while let Some(item) = current {
                if item == boundary {
                    inside = true;
                    break;
                }
                current = item.parent_item(ParentItemTraversalMode::FindAllParents);
            }
            assert!(inside, "modal focus escaped its subtree to {name}");
            assert!(
                ![
                    "Layouts",
                    "Settings",
                    "Start",
                    "Stop",
                    "Keyboard device",
                    "Mouse device"
                ]
                .contains(&name.as_str()),
                "modal leaked to {name}"
            );
        }
        if backwards {
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: Key::Shift.into(),
                });
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    ui.global::<AppState>().set_mapper_supported(true);
    ui.global::<AppState>().set_in_development(true);
    ui.global::<SettingsEditor>().set_in_development(true);
    ui.global::<LayoutLibrary>().invoke_set_automatic(false);
    ui.global::<LayoutLibrary>().invoke_create(
        "Keyboard regression".into(),
        "".into(),
        LayoutSource::Bundled,
        0,
    );
    let refreshes = Rc::new(Cell::new(0));
    let counter = refreshes.clone();
    ui.global::<SettingsEditor>()
        .on_refresh_devices(move || counter.set(counter.get() + 1));
    let game_choice = Rc::new(Cell::new(None));
    let choice = game_choice.clone();
    ui.global::<AppState>()
        .on_set_game_control(move |control| choice.set(Some(control)));
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120., 760.));
    let weak = ui.as_weak();
    let step = Cell::new(0);
    let initiator = Rc::new(std::cell::RefCell::new(None));
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(180), move || {
        let ui = weak.unwrap();
        let stage = step.get();
        println!("keyboard stage {stage}: {:?}", label(&ui));
        step.set(stage + 1);
        match stage {
            0 => { ui.invoke_navigate(Page::Settings, MenuKind::Emoji); ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper); }
            1 => {
                tab_to(&ui, "Settings");
                *initiator.borrow_mut() = Some(focused(&ui).downgrade());
                ui.global::<SettingsEditor>().set_input_devices(slint::ModelRc::new(slint::VecModel::from(vec![DeviceChoice {
                    label: "Regression keyboard".into(), path: "/dev/input/regression-not-real".into(), group: DeviceGroup::Suggested, ..Default::default()
                }, DeviceChoice {
                    label: "Second regression keyboard".into(), path: "/dev/input/regression-second-not-real".into(), group: DeviceGroup::Suggested, ..Default::default()
                }])));
                ui.global::<DeviceDialog>().set_open(true);
            }
            2 => {
                check_modal(&ui);
                tab_to(&ui, "Refresh");
                let before = refreshes.get();
                key(&ui, Key::Return);
                assert_eq!(refreshes.get(), before + 1);
                assert!(ui.global::<DeviceDialog>().get_open());
                assert!(ui.global::<SettingsEditor>().get_keyboard_device_path().is_empty());
                tab_to(&ui, "Regression keyboard");
                key(&ui, Key::DownArrow);
                assert_eq!(label(&ui), "Second regression keyboard");
                assert_eq!(nearest_accessible_item(focused(&ui)).accessible_string_property(AccessibleStringProperty::ItemSelected).as_deref(), Some("true"));
                key(&ui, Key::UpArrow);
                assert_eq!(label(&ui), "Regression keyboard");
                key(&ui, Key::Escape);
            }
            3 => {
                assert!(!ui.global::<KeyboardFocus>().get_active());
                assert_eq!(focused(&ui), initiator.borrow().as_ref().unwrap().upgrade().unwrap());
                ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
            }
            4 => {
                tab_to(&ui, "US layout");
                key(&ui, Key::RightArrow);
                assert_eq!(ui.global::<LayersEditor>().get_label_mode(), LabelMode::Code);
                snapshot(&ui, "keyboard-button-group");
                key(&ui, Key::Return);
                assert_eq!(ui.global::<LayersEditor>().get_label_mode(), LabelMode::Code);
                ui.global::<LayersEditor>().invoke_open_dialog(LayerDialog::Create, -1);
            }
            5 => { check_modal(&ui); key(&ui, Key::Escape); }
            6 => { ui.invoke_navigate(Page::Rules, MenuKind::Emoji); ui.global::<RulesEditor>().invoke_open_dialog(0, RuleDialog::Conditions); }
            7 => {
                check_modal(&ui);
                tab_to(&ui, "Close");
                *initiator.borrow_mut() = Some(focused(&ui).downgrade());
                ui.global::<AppEntryDialog>().invoke_open("firefox".into(), false, 0);
            }
            8 => { check_modal(&ui); snapshot(&ui, "keyboard-nested-dialog"); key(&ui, Key::Escape); }
            9 => {
                assert!(ui.global::<KeyboardFocus>().get_active());
                assert_eq!(focused(&ui), initiator.borrow().as_ref().unwrap().upgrade().unwrap());
                key(&ui, Key::Escape);
            }
            10 => {
                assert!(!ui.global::<KeyboardFocus>().get_active());
                ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
                ui.global::<SettingsEditor>().set_tab(SettingsTab::KeyBehavior);
            }
            11 => {
                tab_to(&ui, "Scroll vertically");
                key(&ui, Key::End);
                let value = nearest_accessible_item(focused(&ui)).accessible_string_property(AccessibleStringProperty::Value).unwrap();
                assert!(value.parse::<f64>().unwrap() > 0., "End scrolls text-only content");
                key(&ui, Key::Home);
                let value = nearest_accessible_item(focused(&ui)).accessible_string_property(AccessibleStringProperty::Value).unwrap();
                assert_eq!(value.parse::<f64>().unwrap(), 0.);
                // The first info button on this page explains a timing parameter.
                for _ in 0..400 {
                    let name = label(&ui);
                    if name.len() > 100 { break; }
                    key(&ui, Key::Tab);
                }
                assert!(label(&ui).len() > 100, "InfoTip is reachable by Tab");
                *initiator.borrow_mut() = Some(focused(&ui).downgrade());
                key(&ui, Key::Return);
                assert!(!WindowInner::from_pub(ui.window()).active_popups.borrow().is_empty(), "InfoTip opens on Enter");
                snapshot(&ui, "keyboard-info-tip");
            }
            12 => { key(&ui, Key::Escape); }
            13 => {
                assert!(WindowInner::from_pub(ui.window()).active_popups.borrow().is_empty());
                assert_eq!(focused(&ui), initiator.borrow().as_ref().unwrap().upgrade().unwrap());
                key(&ui, Key::F1);
                assert!(!WindowInner::from_pub(ui.window()).active_popups.borrow().is_empty(), "F1 exposes full focused help");
            }
            14 => { key(&ui, Key::Escape); }
            15 => {
                assert!(WindowInner::from_pub(ui.window()).active_popups.borrow().is_empty());
                tab_to(&ui, "Game mode: Auto. ");
                key(&ui, Key::Return);
            }
            16 => {
                key(&ui, Key::DownArrow);
                assert!(label(&ui).starts_with("On"), "menu arrows move accessible focus");
                key(&ui, Key::Return);
                assert_eq!(game_choice.get(), Some(GameModeControl::On), "menu Enter activates the Tab-focused row");
                ui.global::<ActionPicker>().invoke_open(PickerTarget::LayerAction, 0, "".into(), false);
                ui.global::<ActionPicker>().invoke_select_behavior(PickerBehavior::Action);
            }
            17 => {
                tab_to(&ui, "Text");
                key(&ui, Key::Return);
                assert_eq!(ui.global::<ActionPicker>().get_category(), PickerCategory::Text);
                key(&ui, Key::Escape);
                assert!(!ui.global::<ActionPicker>().get_opened(), "Escape closes the picker after selecting a category");
            }
            18 => {
                ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
                let menu = ui.global::<MenuEditor>();
                menu.invoke_select_cell(0);
                menu.set_value("α".into());
                menu.invoke_set_cell();
            }
            19 => {
                let menu = ui.global::<MenuEditor>();
                let source = menu.get_cells().row_data(0).unwrap();
                let target = menu.get_cells().row_data(1).unwrap();
                tab_to(&ui, &format!("{} {}", source.key, source.value));
                key(&ui, Key::Return);
                assert_eq!(menu.get_selected_cell(), 0);
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: Key::Control.into() });
                key(&ui, "x");
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: Key::Control.into() });
                assert_eq!(menu.get_keyboard_source_cell(), 0, "Ctrl+X marks the focused source");
                key(&ui, Key::RightArrow);
                assert_eq!(label(&ui), format!("{} {}", target.key, target.value));
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: Key::Control.into() });
                key(&ui, "v");
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: Key::Control.into() });
                assert_eq!(menu.get_keyboard_source_cell(), -1, "Ctrl+V consumes the marked source");
                assert_eq!(menu.get_cells().row_data(1).unwrap().value, source.value);
                assert_eq!(menu.get_cells().row_data(0).unwrap().value, target.value);
                snapshot(&ui, "keyboard-menu-transfer");
                menu.invoke_add_page("Keyboard second".into());
            }
            20 => {
                let menu = ui.global::<MenuEditor>();
                let first = menu.get_pages().row_data(0).unwrap();
                let tab = if first.is_empty() { "1".to_string() } else { format!("1 · {first}") };
                tab_to(&ui, &tab);
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: Key::Alt.into() });
                key(&ui, Key::RightArrow);
                ui.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: Key::Alt.into() });
                assert_eq!(menu.get_pages().row_data(1).unwrap(), first);
                let name = ui.global::<LayoutLibrary>().get_active_label();
                let label = if name.is_empty() { "No active layout".to_string() } else { format!("Active: {name}") };
                tab_to(&ui, &label);
                *initiator.borrow_mut() = Some(focused(&ui).downgrade());
                key(&ui, Key::Return);
                assert!(!WindowInner::from_pub(ui.window()).active_popups.borrow().is_empty());
            }
            21 => { key(&ui, Key::Escape); }
            22 => {
                assert_eq!(focused(&ui), initiator.borrow().as_ref().unwrap().upgrade().unwrap());
                println!("PASS: 1440 Tab/Shift+Tab events; device/menu Enter; segmented arrows; modal restoration; InfoTip/F1; scrolling; action categories; menu cell transfer and page reorder; active layout selector");
                slint::quit_event_loop().unwrap();
            }
            _ => unreachable!(),
        }
    });
    slint::run_event_loop()?;
    Ok(())
}
