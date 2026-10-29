//! Read-only UI diagnostics; every configuration operation uses a temporary directory.
use i_slint_core::{
    accessibility::{AccessibleStringProperty as P, nearest_accessible_item},
    item_tree::ItemRc,
    window::WindowInner,
};
use lhc_core::{config_document::KeyAssignment, storage::StoragePaths};
use serde_json::{Value, json};
use slint::ComponentHandle;
use slint_shell::{Document, bind_document, ui::*};
use std::{cell::Cell, path::PathBuf, rc::Rc, time::Duration};
fn node(item: &ItemRc) -> Value {
    let pos = item.map_to_window(item.geometry().origin);
    json!({"role":format!("{:?}",item.accessible_role()),"label":item.accessible_string_property(P::Label).map(|v|v.to_string()),"value":item.accessible_string_property(P::Value).map(|v|v.to_string()),"description":item.accessible_string_property(P::Description).map(|v|v.to_string()),"enabled":item.accessible_string_property(P::Enabled).map(|v|v.to_string()),"checked":item.accessible_string_property(P::Checked).map(|v|v.to_string()),"selected":item.accessible_string_property(P::ItemSelected).map(|v|v.to_string()),"live":item.accessible_string_property(P::LiveRegion).map(|v|v.to_string()),"x":pos.x,"y":pos.y,"width":item.geometry().size.width,"height":item.geometry().size.height})
}
fn key(ui: &impl ComponentHandle, text: slint::SharedString) {
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
}
fn dump(ui: &impl ComponentHandle, name: &str, out: &std::path::Path) {
    let pixels = ui.window().take_snapshot().unwrap();
    let path = out.join(format!("{name}.png"));
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create(path).unwrap()),
        pixels.width(),
        pixels.height(),
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = pixels
        .as_slice()
        .iter()
        .flat_map(|p| [p.r, p.g, p.b])
        .collect();
    enc.write_header()
        .unwrap()
        .write_image_data(&bytes)
        .unwrap();
    let inner = WindowInner::from_pub(ui.window());
    let mut stack = vec![ItemRc::new_root(inner.component())];
    let mut nodes = vec![];
    while let Some(item) = stack.pop() {
        if item.is_visible() && item.is_accessible() {
            nodes.push(node(&item));
        }
        let mut child = item.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    let initial = inner
        .focus_item
        .borrow()
        .upgrade()
        .map(nearest_accessible_item)
        .map(|i| node(&i));
    let mut focus = vec![];
    for _ in 0..180 {
        key(ui, slint::platform::Key::Tab.into());
        if let Some(i) = inner.focus_item.borrow().upgrade() {
            focus.push(node(&nearest_accessible_item(i)));
        }
    }
    std::fs::write(out.join(format!("{name}.json")),serde_json::to_vec_pretty(&json!({"initial":initial,"nodes":nodes,"tab_sequence":focus,"scale":ui.window().scale_factor(),"physical_size":[pixels.width(),pixels.height()]})).unwrap()).unwrap();
    println!("DUMP {name}");
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let doc = Document::load(StoragePaths::new(
        dir.path().join("config"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &doc);
    ui.global::<AppState>().set_is_linux(true);
    ui.global::<AppState>().set_mapper_supported(true);
    ui.global::<AppState>().set_autostart_supported(true);
    ui.global::<Theme>().set_dark(true);
    slint_shell::ui::apply_theme(&ui.global::<Theme>());
    ui.global::<AppState>().set_in_development(true);
    ui.global::<SettingsEditor>().set_in_development(true);
    ui.global::<LayoutLibrary>()
        .invoke_create("Audit".into(), "".into(), LayoutSource::Bundled, 0);
    doc.edit(slint_shell::document::View::Library, |c| {
        let id = c.create_layer("Audit layer", "")?;
        c.set_extra_key(&id, None, "F13", KeyAssignment::Action("Alt+Tab".into()))?;
        Ok(())
    })?;
    ui.global::<MacroEditor>().invoke_add("Audit macro".into());
    ui.global::<ConditionChoices>()
        .on_refresh_open_windows(|| {});
    ui.global::<ConditionChoices>()
        .set_open_windows(slint::ModelRc::new(slint::VecModel::from(vec![
            WindowChoice {
                pattern: "org.kde.konsole".into(),
                title: "Terminal".into(),
                count: 1,
                ..Default::default()
            },
        ])));
    ui.global::<ConditionChoices>().set_open_windows_ready(true);
    ui.show()?;
    ui.window().set_size(slint::LogicalSize::new(1120., 760.));
    let weak = ui.as_weak();
    let step = Rc::new(Cell::new(0));
    let timer = slint::Timer::default();
    let out = PathBuf::from(std::env::var("LHC_AUDIT_OUT")?);
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(700),
        move || {
            let ui = weak.unwrap();
            let s = step.get();
            step.set(s + 1);
            match s {
                0 => {
                    ui.invoke_navigate(Page::Layouts, MenuKind::Emoji);
                }
                2 => {
                    dump(&ui, "library", &out);
                }
                3 => {
                    ui.global::<LayoutLibrary>().invoke_set_automatic(true);
                }
                4 => {
                    ui.global::<LayoutLibrary>().invoke_open_rule(-1, 0);
                }
                6 => {
                    dump(&ui, "auto-rule-dialog", &out);
                }
                7 => {
                    ui.global::<LayoutLibrary>().invoke_cancel_dialog();
                    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::General);
                }
                8 => {
                    dump(&ui, "settings-general", &out);
                }
                9 => {
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper);
                }
                10 => {
                    dump(&ui, "settings-mapper", &out);
                }
                11 => {
                    ui.global::<SettingsEditor>()
                        .set_tab(SettingsTab::KeyBehavior);
                }
                12 => {
                    dump(&ui, "settings-behavior", &out);
                }
                13 => {
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::GameMode);
                }
                14 => {
                    dump(&ui, "settings-game", &out);
                }
                15 => {
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::Commands);
                }
                16 => {
                    dump(&ui, "settings-commands", &out);
                }
                17 => {
                    ui.global::<SettingsEditor>()
                        .set_tab(SettingsTab::Accessibility);
                }
                18 => {
                    dump(&ui, "settings-accessibility", &out);
                }
                19 => {
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper);
                    ui.global::<DeviceDialog>().set_open(true);
                }
                20 => {
                    dump(&ui, "device-dialog", &out);
                }
                21 => {
                    ui.global::<DeviceDialog>().set_open(false);
                    ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
                    ui.global::<LayersEditor>().invoke_choose(0);
                }
                22 => {
                    dump(&ui, "layers", &out);
                }
                23 => {
                    ui.global::<LayersEditor>()
                        .invoke_open_dialog(LayerDialog::Create, -1);
                }
                24 => {
                    dump(&ui, "new-layer-dialog", &out);
                }
                25 => {
                    ui.global::<LayersEditor>().set_dialog(LayerDialog::None);
                    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
                }
                26 => {
                    dump(&ui, "rules", &out);
                }
                27 => {
                    ui.global::<RulesEditor>()
                        .invoke_open_dialog(0, RuleDialog::Conditions);
                }
                28 => {
                    dump(&ui, "conditions", &out);
                }
                29 => {
                    ui.global::<AppEntryDialog>()
                        .invoke_open("firefox".into(), false, 0);
                }
                30 => {
                    dump(&ui, "nested-app-dialog", &out);
                }
                31 => {
                    ui.global::<AppEntryDialog>().invoke_close();
                    ui.global::<RulesEditor>().set_dialog(RuleDialog::None);
                    ui.global::<ActionPicker>().invoke_open(
                        PickerTarget::Rule,
                        0,
                        "KeyA".into(),
                        false,
                    );
                }
                32 => {
                    dump(&ui, "picker", &out);
                }
                33 => {
                    ui.global::<ActionPicker>().invoke_dismiss_picker();
                    ui.invoke_navigate(Page::Macros, MenuKind::Emoji);
                }
                34 => {
                    dump(&ui, "macros", &out);
                }
                35 => {
                    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
                }
                36 => {
                    dump(&ui, "emoji-editor", &out);
                }
                37 => {
                    ui.invoke_navigate(Page::Menus, MenuKind::Quick);
                }
                38 => {
                    dump(&ui, "quick-editor", &out);
                }
                39 => {
                    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
                    ui.global::<RulesEditor>()
                        .invoke_open_dialog(0, RuleDialog::Advanced);
                }
                40 => {
                    dump(&ui, "advanced", &out);
                }
                41 => {
                    ui.global::<RulesEditor>().set_dialog(RuleDialog::None);
                    ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
                    ui.global::<LayersEditor>().invoke_choose(5);
                }
                42 => {
                    dump(&ui, "extra-keys", &out);
                }
                43 => {
                    ui.global::<Theme>().set_dark(false);
                    slint_shell::ui::apply_theme(&ui.global::<Theme>());
                    ui.invoke_navigate(Page::Settings, MenuKind::Emoji);
                    ui.global::<SettingsEditor>()
                        .set_tab(SettingsTab::KeyBehavior);
                    ui.window().set_size(slint::LogicalSize::new(940., 700.));
                }
                44 => {
                    dump(&ui, "light-940", &out);
                }
                45 => {
                    ui.global::<Theme>().set_high_contrast(true);
                    slint_shell::ui::apply_theme(&ui.global::<Theme>());
                }
                46 => {
                    dump(&ui, "light-hc-940", &out);
                }
                47 => {
                    ui.global::<Theme>().set_dark(true);
                    slint_shell::ui::apply_theme(&ui.global::<Theme>());
                }
                48 => {
                    dump(&ui, "dark-hc-940", &out);
                }
                49 => {
                    ui.global::<Theme>().set_eink(true);
                    slint_shell::ui::apply_theme(&ui.global::<Theme>());
                }
                50 => {
                    dump(&ui, "eink-940", &out);
                }
                51 => {
                    ui.global::<Theme>().set_eink(false);
                    ui.global::<Theme>().set_high_contrast(false);
                    slint_shell::ui::apply_theme(&ui.global::<Theme>());
                    ui.global::<SettingsEditor>().set_tab(SettingsTab::Mapper);
                    ui.global::<SettingsEditor>()
                        .set_input_devices(slint::ModelRc::new(slint::VecModel::from(vec![
                            DeviceChoice {
                                label: "Audit keyboard".into(),
                                path: "/dev/input/audit-not-real".into(),
                                group: DeviceGroup::Suggested,
                                ..Default::default()
                            },
                        ])));
                    ui.global::<DeviceDialog>().set_open(true);
                }
                52 => {
                    dump(&ui, "device-with-fixture", &out);
                }
                53 => {
                    let inner = WindowInner::from_pub(ui.window());
                    for _ in 0..180 {
                        let f = inner
                            .focus_item
                            .borrow()
                            .upgrade()
                            .map(nearest_accessible_item);
                        if f.is_some_and(|i| {
                            i.accessible_string_property(P::Label).as_deref() == Some("Refresh")
                        }) {
                            break;
                        }
                        key(&ui, slint::platform::Key::Tab.into());
                    }
                    println!(
                        "EXPERIMENT device before Enter: open={}, path={}",
                        ui.global::<DeviceDialog>().get_open(),
                        ui.global::<SettingsEditor>().get_keyboard_device_path()
                    );
                    key(&ui, slint::platform::Key::Return.into());
                    println!(
                        "EXPERIMENT device after Enter on Refresh: open={}, path={}",
                        ui.global::<DeviceDialog>().get_open(),
                        ui.global::<SettingsEditor>().get_keyboard_device_path()
                    );
                    ui.global::<DeviceDialog>().set_open(false);
                    ui.invoke_navigate(Page::Layers, MenuKind::Emoji);
                }
                54 => {
                    let inner = WindowInner::from_pub(ui.window());
                    for _ in 0..180 {
                        let f = inner
                            .focus_item
                            .borrow()
                            .upgrade()
                            .map(nearest_accessible_item);
                        if f.is_some_and(|i| {
                            i.accessible_string_property(P::Label).as_deref() == Some("US layout")
                        }) {
                            break;
                        }
                        key(&ui, slint::platform::Key::Tab.into());
                    }
                    key(&ui, slint::platform::Key::RightArrow.into());
                    println!(
                        "EXPERIMENT ButtonGroup after Right: {:?}",
                        ui.global::<LayersEditor>().get_label_mode()
                    );
                    key(&ui, slint::platform::Key::Return.into());
                    println!(
                        "EXPERIMENT ButtonGroup after Enter: {:?}",
                        ui.global::<LayersEditor>().get_label_mode()
                    );
                }
                56 => {
                    let popup = EmojiPopup::new().unwrap();
                    popup.set_page_names(slint::ModelRc::new(slint::VecModel::from(vec![
                        "Audit emoji".into(),
                    ])));
                    popup.set_emojis(slint::ModelRc::new(slint::VecModel::from(vec![
                        "A".into();
                        15
                    ])));
                    popup.show().unwrap();
                    popup.invoke_prepare();
                    dump(&popup, "emoji-popup", &out);
                    popup.hide().unwrap();
                    let popup = QuickPopup::new().unwrap();
                    popup.set_page_names(slint::ModelRc::new(slint::VecModel::from(vec![
                        "Audit actions".into(),
                    ])));
                    popup.set_items(slint::ModelRc::new(slint::VecModel::from(vec![
                        "Action"
                            .into();
                        15
                    ])));
                    popup.show().unwrap();
                    popup.invoke_prepare();
                    dump(&popup, "quick-popup", &out);
                    popup.hide().unwrap();
                    slint::quit_event_loop().unwrap();
                }
                _ => {}
            }
        },
    );
    slint::run_event_loop()?;
    Ok(())
}
