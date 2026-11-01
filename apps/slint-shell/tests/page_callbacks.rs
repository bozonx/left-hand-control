//! Settings-page callback regressions that need no real input or compositor:
//! Slint bindings → page modules → `Document` → temporary config files, on the
//! software test platform. Pointer, keyboard, geometry and theme checks stay in
//! the graphical `examples/`.

#[path = "../examples/support/mod.rs"]
mod support;

use lhc_core::{config_document::KeyAssignment, storage::StoragePaths};
use slint::{
    ComponentHandle, Model, ModelRc, VecModel,
    platform::{Platform, WindowAdapter, software_renderer::MinimalSoftwareWindow},
};
use slint_shell::{Document, bind_document, ui::*};
use std::{rc::Rc, time::Instant};
use support::{Fixture, quick_action, wait_until};

struct TestPlatform(Instant);
impl Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(MinimalSoftwareWindow::new(Default::default()))
    }
    fn duration_since_start(&self) -> std::time::Duration {
        self.0.elapsed()
    }
}

/// One test: the Slint platform and the UI language are process-global.
#[test]
fn page_callbacks_without_a_desktop() {
    slint::platform::set_platform(Box::new(TestPlatform(Instant::now()))).unwrap();
    key_editor().unwrap();
    conditions_dialog().unwrap();
    check_atomic_commands().unwrap();
    action_picker().unwrap();
    layer_editor_regressions().unwrap();
}

/// Key editor, translations of every bundled language and picker assignment.
fn key_editor() -> Result<(), Box<dyn std::error::Error>> {
    let Fixture {
        dir: _dir,
        paths: _,
        document,
        ui,
    } = Fixture::new()?;
    ui.set_page(Page::Keyboard);
    let keys = ui.global::<KeyEditor>();
    assert_eq!(keys.get_keys().row_count(), 80);
    assert_eq!(keys.get_keys().row_data(33).unwrap().label, "Q");
    let loaded = |count| Message {
        id: "config-loaded".into(),
        arg: "".into(),
        count,
        diagnostic_key: Default::default(),
    };
    let locale = ui.global::<Locale>();
    // Exercise every bundled language through Slint before the detailed RU/EN assertions.
    for code in [
        "en", "ru", "de", "fr", "es", "it", "pt", "pl", "tr", "uk", "cs", "hu", "ro", "el", "hr",
        "ka", "kk", "zh-Hans", "zh-Hant", "ja", "ko",
    ] {
        slint_shell::select_ui_language(code)?;
        for count in [0, 1, 2, 5, 11, 21] {
            let text = locale.invoke_text(loaded(count));
            assert!(!text.is_empty(), "{code}: missing translation");
            assert!(
                text.contains(&count.to_string()),
                "{code}: missing count: {text}"
            );
        }
    }
    slint_shell::select_ui_language("ru")?;
    assert_eq!(
        locale.invoke_text(loaded(1)),
        "Конфигурация загружена: 1 правило"
    );
    assert_eq!(
        locale.invoke_text(loaded(5)),
        "Конфигурация загружена: 5 правил"
    );
    assert_eq!(
        locale.invoke_text(loaded(2)),
        "Конфигурация загружена: 2 правила"
    );
    slint_shell::select_ui_language("en")?;
    assert_eq!(
        locale.invoke_text(loaded(1)),
        "Configuration loaded: 1 rule"
    );
    slint_shell::select_ui_language("ru")?;
    assert_eq!(
        locale.invoke_text(loaded(21)),
        "Конфигурация загружена: 21 правило"
    );
    for id in [
        "action-invalid-syntax",
        "action-pause-outside-macro",
        "action-unknown-macro",
        "action-unknown-command",
        "action-unknown-system",
        "action-unknown-app",
        "action-unknown-type",
        "action-failed",
        "error",
        "mapper-failed",
        "config-io",
        "config-parse",
        "config-invalid",
    ] {
        let message = Message {
            id: id.into(),
            arg: "untranslated technical detail".into(),
            count: 0,
            diagnostic_key: Default::default(),
        };
        let russian = locale.invoke_text(message.clone());
        assert!(!russian.is_empty(), "missing Russian message: {id}");
        assert!(!russian.contains("untranslated technical detail"));
        slint_shell::select_ui_language("en")?;
        let english = locale.invoke_text(message);
        assert!(!english.is_empty(), "missing English message: {id}");
        assert_ne!(russian, english, "untranslated message: {id}");
        slint_shell::select_ui_language("ru")?;
    }
    let action_messages: Vec<_> = lhc_core::profile::actions::catalog(&document.read().config())
        .into_iter()
        .filter_map(|entry| {
            use lhc_core::profile::actions::ActionName;
            let (id, arg, count) = match entry.name {
                ActionName::BuiltinMacro { id } => ("builtin-macro", id, 0),
                ActionName::System { id, n } | ActionName::App { id, n } => {
                    ("picker-action", id, n)
                }
                ActionName::Verbatim(_) => return None,
            };
            Some(Message {
                id: id.into(),
                arg: arg.into(),
                count: i32::try_from(count).unwrap(),
                diagnostic_key: Default::default(),
            })
        })
        .collect();
    let russian: Vec<_> = action_messages
        .iter()
        .map(|message| locale.invoke_text(message.clone()))
        .collect();
    slint_shell::select_ui_language("en")?;
    for (message, russian) in action_messages.into_iter().zip(russian) {
        let english = locale.invoke_text(message.clone());
        assert!(
            !russian.is_empty() && !english.is_empty(),
            "missing catalog label: {message:?}"
        );
        assert_ne!(russian, english, "untranslated catalog label: {message:?}");
    }
    slint_shell::select_ui_language("ru")?;
    assert_eq!(
        locale.invoke_text(Message {
            id: "unknown-id".into(),
            arg: "untranslated technical detail".into(),
            count: 0,
            diagnostic_key: Default::default(),
        }),
        ""
    );
    slint_shell::select_ui_language("en")?;

    // Editing a key opens the picker; applying saves the tap action.
    let picker = ui.global::<ActionPicker>();
    keys.invoke_edit(33);
    assert!(picker.get_opened());
    assert_eq!(keys.get_selected(), 33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:Привет 👋".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("text:Привет 👋")
    );
    assert_eq!(
        keys.get_keys().row_data(33).unwrap().action,
        "text:Привет 👋"
    );
    assert_eq!(ui.global::<AppState>().get_status().id, "");

    // Cancel keeps the saved value.
    keys.invoke_edit(33);
    assert_eq!(picker.get_value(), "text:Привет 👋");
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Ctrl+KeyC".into());
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("text:Привет 👋")
    );

    // Pauses are only valid inside macros.
    keys.invoke_edit(33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("pause:250".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert!(!picker.get_valid());
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Ctrl+Shift+KeyK".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("Ctrl+Shift+KeyK")
    );
    Ok(())
}

/// Conditions dialog: add, reject duplicates, edit, remove and save.
fn items(choices: &ConditionChoices<'_>, value: slint::SharedString) -> Vec<String> {
    choices
        .invoke_items(value)
        .iter()
        .map(|item| item.to_string())
        .collect()
}

fn conditions_dialog() -> Result<(), Box<dyn std::error::Error>> {
    let Fixture {
        dir: _dir,
        paths: _,
        document,
        ui,
    } = Fixture::new()?;
    document.edit(slint_shell::document::View::Library, |config| {
        config.update_layout(|layout| {
            layout.rules = vec![lhc_core::profile::model::KeyRule::new("r".into(), "F13")];
        })
    })?;
    ui.set_page(Page::Rules);
    let choices = ui.global::<ConditionChoices>();
    // Supply stable window choices; the scenario does not query the compositor.
    choices.on_refresh_open_windows(|| {});
    choices.set_open_windows(ModelRc::new(VecModel::from(vec![WindowChoice {
        pattern: "org.kde.konsole".into(),
        app_id: "org.kde.konsole".into(),
        count: 1,
        titles: ModelRc::new(VecModel::from(vec!["Terminal".into()])),
        ..Default::default()
    }])));
    choices.set_open_windows_ready(true);
    let rules = ui.global::<RulesEditor>();
    rules.invoke_open_dialog(0, RuleDialog::Conditions);
    ui.window().set_size(slint::LogicalSize::new(1100., 800.));
    // Instantiate the visible condition-list bindings before editing its dialog.
    let _ = ui.window().take_snapshot()?;
    let dialog = ui.global::<AppEntryDialog>();
    let names = [
        "org.kde.konsole",
        "firefox",
        "a-very-long-application-identifier.with.many.parts",
    ];
    for (index, name) in names.iter().enumerate() {
        dialog.invoke_open(rules.get_apps_include(), false, -1);
        dialog.set_text((*name).into());
        assert!(dialog.invoke_valid());
        dialog.invoke_save();
        wait_until("added application condition", || {
            items(&choices, rules.get_apps_include()).len() == index + 1
        });
        assert_eq!(items(&choices, rules.get_apps_include()), names[..=index]);
    }
    dialog.invoke_open(rules.get_apps_include(), false, -1);
    dialog.set_text("FIREFOX".into());
    assert!(!dialog.invoke_valid(), "duplicates are case insensitive");
    let revision = dialog.get_revision();
    dialog.invoke_save();
    assert_eq!(dialog.get_revision(), revision);
    dialog.invoke_close();

    dialog.invoke_open(rules.get_apps_include(), false, 1);
    dialog.set_text("org.mozilla.firefox".into());
    assert!(dialog.invoke_valid());
    dialog.invoke_save();
    wait_until("edited application condition", || {
        items(&choices, rules.get_apps_include())[1] == "org.mozilla.firefox"
    });
    dialog.invoke_confirm_remove(rules.get_apps_include(), false, 2);
    dialog.invoke_remove();
    wait_until("removed application condition", || {
        items(&choices, rules.get_apps_include()).len() == 2
    });
    assert_eq!(
        items(&choices, rules.get_apps_include()),
        ["org.kde.konsole", "org.mozilla.firefox"]
    );
    rules.invoke_apply_conditions();
    assert_eq!(
        document.read().layout().rules[0]
            .condition_window_patterns
            .as_deref(),
        Some(
            [
                "org.kde.konsole".to_string(),
                "org.mozilla.firefox".to_string()
            ]
            .as_slice()
        )
    );
    Ok(())
}

/// Picker catalog, trigger validation, cancel, rule actions, macro steps and quick actions.
fn action_picker() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    slint::select_bundled_translation("ru")?;
    bind_document(&ui, &document);
    // Hidden pages and action categories stay unavailable outside development mode.
    ui.invoke_navigate(Page::Macros, MenuKind::Emoji);
    assert_eq!(ui.get_page(), Page::Layouts);
    ui.invoke_navigate(Page::Menus, MenuKind::Emoji);
    assert_eq!(ui.get_page(), Page::Layouts);
    ui.invoke_navigate(Page::Rules, MenuKind::Emoji);
    ui.global::<RulesEditor>().invoke_add();
    let restricted = ui.global::<ActionPicker>();
    restricted.set_query("Mouse".into());
    restricted.invoke_refresh();
    assert_eq!(restricted.get_items().row_count(), 0);
    assert!(
        restricted
            .get_items()
            .iter()
            .all(|item| item.category != PickerCategory::Mouse)
    );
    restricted.invoke_dismiss_picker();
    let settings = ui.global::<SettingsEditor>();
    settings.set_in_development(true);
    settings.invoke_save();
    ui.global::<AppState>().set_in_development(true);

    let rules = ui.global::<RulesEditor>();
    let count = document.read().layout().rules.len();
    rules.invoke_add();
    let picker = ui.global::<ActionPicker>();
    assert!(picker.get_opened());
    assert!(picker.get_key_only());
    assert_eq!(
        rules.get_dialog(),
        RuleDialog::None,
        "the picker replaces the dialog"
    );
    assert_eq!(
        picker
            .get_sidebar()
            .iter()
            .map(|entry| entry.visible)
            .collect::<Vec<_>>(),
        [[true; 8], [false; 8]].concat(),
        "a trigger offers only key categories"
    );
    picker.set_category(PickerCategory::Modifiers);
    picker.invoke_refresh();
    assert_eq!(picker.get_items().row_count(), 9);
    assert!(
        picker
            .get_items()
            .iter()
            .any(|item| item.value == "ControlLeft")
    );
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Tab".into());
    picker.invoke_dismiss_picker();
    assert_eq!(document.read().layout().rules.len(), count);
    rules.invoke_add();
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a trigger must be a key");
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Tab".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    let index = rules.get_selected();
    assert!(index >= 0);
    assert_eq!(document.read().layout().rules[index as usize].key, "Tab");
    rules.invoke_open_dialog(index, RuleDialog::Tap);
    picker.set_category(PickerCategory::Text);
    picker.set_query("ck".into());
    picker.invoke_refresh();
    // Backspace, MediaTrackNext and MouseBack.
    for category in [
        PickerCategory::ControlKeys,
        PickerCategory::Media,
        PickerCategory::Mouse,
    ] {
        assert!(
            picker
                .get_items()
                .iter()
                .any(|item| item.category == category)
        );
    }
    assert_eq!(picker.get_category(), PickerCategory::Text);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyA".into());
    picker.invoke_refresh();
    assert_eq!(picker.get_value_category(), PickerCategory::Letters);
    picker.set_query("рабочий стол".into());
    picker.invoke_refresh();
    assert!(picker.get_items().row_count() >= 2);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert_eq!(
        document.read().layout().rules[index as usize]
            .tap_action
            .as_deref(),
        Some("builtin:copyLineText")
    );
    rules.invoke_open_dialog(index, RuleDialog::Tap);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyB".into());
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    assert_eq!(picker.get_value(), "KeyB");
    picker.invoke_select_behavior(PickerBehavior::Action);
    assert_eq!(picker.get_value(), "KeyB");
    picker.set_value("".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert!(!picker.get_valid());
    picker.invoke_dismiss_picker();
    assert_eq!(
        document.read().layout().rules[index as usize]
            .tap_action
            .as_deref(),
        Some("builtin:copyLineText")
    );
    rules.invoke_open_dialog(index, RuleDialog::Hold);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("builtin:copyLineText".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    rules.invoke_open_dialog(index, RuleDialog::Key);
    picker.set_value("MouseLeft".into());
    picker.invoke_apply();
    assert!(picker.get_opened());
    picker.invoke_dismiss_picker();
    let macros = ui.global::<MacroEditor>();
    macros.invoke_add("Self".into());
    macros.invoke_set_field(0, MacroField::Id, "self".into());
    macros.invoke_add_step(0, "KeyA".into());
    macros.set_picker_macro(0);
    picker.invoke_open(PickerTarget::MacroStep, 0, "KeyA".into(), false);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("macro:self".into());
    picker.invoke_apply();
    assert!(picker.get_opened(), "a macro cannot call itself");
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.invoke_select_category(PickerCategory::Pause);
    assert_eq!(picker.get_value(), "pause:20");
    macros.invoke_set_field(0, MacroField::StepPause, "75".into());
    picker.set_category(PickerCategory::Modifiers);
    picker.set_value("KeyA".into());
    picker.invoke_refresh();
    picker.invoke_select_category(PickerCategory::Pause);
    assert_eq!(picker.get_value(), "pause:75");
    picker.set_value("pause:250".into());
    picker.invoke_refresh();
    assert_eq!(
        picker.get_value(),
        "pause:250",
        "refresh preserves explicit pauses"
    );
    picker.set_value("pause:".into());
    picker.invoke_refresh();
    assert!(!picker.get_valid(), "a pause requires an explicit duration");
    picker.set_value("pause:250".into());
    picker.invoke_apply();
    assert_eq!(
        macros
            .get_macros()
            .row_data(0)
            .unwrap()
            .steps
            .row_data(0)
            .unwrap()
            .action,
        "pause:250"
    );
    assert_eq!(
        document.read().layout().macros[0].steps[0].action,
        "pause:250"
    );
    ui.invoke_navigate(Page::Menus, MenuKind::Quick);
    picker.invoke_open(PickerTarget::QuickAction, 1, "".into(), false);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:Привет\nмир".into());
    picker.invoke_apply();
    let menus = ui.global::<MenuEditor>();
    assert_eq!(menus.get_value(), "text:Привет\nмир");
    assert_eq!(menus.get_selected_cell(), 1);
    assert_eq!(
        quick_action(document.read().layout(), 0, "KeyW"),
        "text:Привет\nмир"
    );
    document.edit(slint_shell::document::View::Layers, |config| {
        config.create_layer("Layer", "")
    })?;
    let layers = ui.global::<LayersEditor>();
    layers.invoke_choose(0);
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("Ctrl+KeyC".into());
    picker.invoke_apply();
    assert_eq!(
        document
            .read()
            .layer_key(&layers.get_selected_id(), "Backquote"),
        KeyAssignment::Action("Ctrl+KeyC".into())
    );
    layers.set_dialog(LayerDialog::None);
    ui.global::<KeyEditor>().invoke_edit(33);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("text:example".into());
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some("text:example")
    );
    Ok(())
}

/// Command edits stay with their target, including invalid page drafts.
fn check_atomic_commands() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let document = Document::load(StoragePaths::new(
        dir.path().join("settings"),
        dir.path().join("data"),
    ))?;
    let ui = SettingsWindow::new()?;
    bind_document(&ui, &document);
    let settings = ui.global::<SettingsEditor>();
    settings.set_commands_enabled(true);
    settings.set_in_development(true);
    settings.invoke_save();
    ui.global::<AppState>().set_in_development(true);
    let picker = ui.global::<ActionPicker>();
    let edit_command = |script: &str| {
        picker.invoke_select_behavior(PickerBehavior::Action);
        picker.set_command_script(script.into());
        picker.invoke_edit_command();
    };
    let before = document.read().layout().clone();
    picker.invoke_open(PickerTarget::BaseKey, i32::MAX, "".into(), false);
    edit_command("echo invalid-target");
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(picker.get_error().id, "config-edit-conflict");
    assert_eq!(document.read().layout(), &before);
    picker.invoke_dismiss_picker();
    ui.global::<KeyEditor>().invoke_edit(33);
    edit_command("echo assigned");
    let action = picker.get_value().to_string();
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().base_tap_action("KeyQ"),
        Some(action.as_str())
    );
    assert_eq!(document.read().layout().commands.len(), 1);
    let before = document.read().layout().clone();
    picker.invoke_open(PickerTarget::BaseKey, i32::MAX, action.into(), false);
    edit_command("echo must-not-overwrite");
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(document.read().layout(), &before);
    picker.invoke_dismiss_picker();

    let macros = ui.global::<MacroEditor>();
    macros.invoke_add("Draft".into());
    macros.invoke_set_field(0, MacroField::ModifierDelay, "invalid".into());
    macros.invoke_add_step(0, "".into());
    edit_command("echo macro-draft");
    let action = picker.get_value();
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(document.read().layout().commands.len(), 1);
    assert!(document.read().layout().macros[0].steps.is_empty());
    assert!(
        !ui.invoke_flush_drafts(),
        "quit must keep an invalid macro draft"
    );
    assert_eq!(ui.get_page(), Page::Macros);
    // Reopening a pending command restores its fields without committing it.
    macros.set_picker_macro(0);
    picker.invoke_open(PickerTarget::MacroStep, 0, action, false);
    assert_eq!(picker.get_command_script(), "echo macro-draft");
    picker.invoke_dismiss_picker();
    macros.invoke_set_field(0, MacroField::ModifierDelay, "10".into());
    assert!(ui.invoke_flush_drafts());
    assert_eq!(document.read().layout().commands.len(), 2);
    assert_eq!(document.read().layout().macros[0].steps.len(), 1);

    // A quick-menu conflict keeps both the pending command and the local menu.
    let menus = ui.global::<MenuEditor>();
    menus.invoke_open(MenuKind::Quick);
    menus.set_page_name("Local".into());
    menus.invoke_rename_page();
    document.edit(slint_shell::document::View::Shell, |config| {
        config.update_layout(|layout| {
            layout.quick_action_pages[0].name = "Concurrent".into();
        })
    })?;
    picker.invoke_open(PickerTarget::QuickAction, 0, "".into(), false);
    edit_command("echo menu-draft");
    let action = picker.get_value();
    picker.invoke_apply();
    assert_eq!(document.read().layout().commands.len(), 2);
    assert!(
        !ui.invoke_flush_drafts(),
        "quit must keep a conflicting menu draft"
    );
    assert_eq!(ui.get_page(), Page::Menus);
    assert_eq!(menus.get_value(), action);
    picker.invoke_open(PickerTarget::QuickAction, 0, action, false);
    assert_eq!(picker.get_command_script(), "echo menu-draft");
    picker.invoke_dismiss_picker();
    menus.invoke_refresh();
    assert!(ui.invoke_flush_drafts());
    assert_eq!(
        document.read().layout().commands.len(),
        2,
        "discarding the menu discards its command"
    );

    settings.set_hold_timeout("invalid".into());
    assert!(!ui.invoke_flush_drafts(), "quit must keep invalid settings");
    assert_eq!(ui.get_page(), Page::Settings);
    settings.set_hold_timeout(settings.get_default_hold_timeout());
    assert!(ui.invoke_flush_drafts());

    // Layer commands are committed atomically with the picker assignment.
    document.edit(slint_shell::document::View::Layers, |config| {
        config.create_layer("Layer", "")
    })?;
    let layers = ui.global::<LayersEditor>();
    layers.invoke_choose(0);
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    edit_command("echo cancelled-layer-command");
    picker.invoke_dismiss_picker();
    assert_eq!(document.read().layout().commands.len(), 2);
    assert_eq!(
        document
            .read()
            .layer_key(&layers.get_selected_id(), "Backquote"),
        KeyAssignment::Transparent
    );
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    edit_command("echo layer-command");
    let action = picker.get_value();
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(layers.get_dialog(), LayerDialog::None);
    assert_eq!(document.read().layout().commands.len(), 3);
    assert_eq!(
        document
            .read()
            .layer_key(&layers.get_selected_id(), "Backquote"),
        KeyAssignment::Action(action.to_string())
    );
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    picker.invoke_edit_command();
    assert_eq!(picker.get_command_script(), "echo layer-command");
    picker.invoke_dismiss_picker();
    // A stale target must not leave an orphan command in the layout.
    layers.invoke_open_dialog(LayerDialog::EditKey, 1);
    edit_command("echo stale-command");
    document.edit(slint_shell::document::View::Layers, |config| {
        config.rename_layer(&layers.get_selected_id(), "Renamed", "")
    })?;
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(picker.get_error().id, "config-edit-conflict");
    assert_eq!(document.read().layout().commands.len(), 3);
    picker.invoke_dismiss_picker();
    Ok(())
}

fn layer_editor_regressions() -> Result<(), Box<dyn std::error::Error>> {
    use slint_shell::document::View;
    let fixture = support::Fixture::new()?;
    let (ui, document) = (&fixture.ui, &fixture.document);
    let (first, second) = document
        .edit(View::Layers, |config| {
            let first = config.create_layer("First", "")?;
            let second = config.create_layer("Second", "")?;
            config.set_extra_key(&first, None, "F13", KeyAssignment::Action("Alt+Tab".into()))?;
            config.set_layer_key_holds_modifiers(&first, "F13", true)?;
            config.set_extra_key(&first, None, "F14", KeyAssignment::Swallow)?;
            Ok((first, second))
        })?
        .value;
    let layers = ui.global::<LayersEditor>();
    let picker = ui.global::<ActionPicker>();
    layers.invoke_choose(0);
    assert_eq!(layers.get_activation().id, "layer-unused");
    document.edit(View::Rules, |config| {
        config.update_layout(|layout| {
            let mut rule = lhc_core::profile::model::KeyRule::new("activate".into(), "CapsLock");
            rule.layer_id = first.clone();
            rule.hold_behavior = Some(lhc_core::profile::model::HoldBehavior::Layer);
            layout.rules.push(rule);
        })
    })?;
    assert_eq!(layers.get_activation().id, "layer-triggers");
    assert_eq!(layers.get_activation().arg, "CapsLock");

    // A selection change cannot redirect an already-open key editor.
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyZ".into());
    layers.invoke_choose(1);
    picker.invoke_apply();
    assert!(picker.get_opened());
    assert_eq!(picker.get_error().id, "config-edit-conflict");
    assert_eq!(
        document.read().layer_key(&second, "Backquote"),
        KeyAssignment::Transparent
    );
    picker.invoke_dismiss_picker();
    layers.invoke_choose(0);

    // Reordering extras invalidates the old row index before any mutation.
    layers.invoke_pick_extra(0, false);
    picker.invoke_select_behavior(PickerBehavior::Swallow);
    layers.invoke_reorder_extra(0, 1);
    picker.invoke_apply();
    assert_eq!(picker.get_error().id, "config-edit-conflict");
    assert!(document.read().layer_key_holds_modifiers(&first, "F13"));
    assert_eq!(
        document.read().layer_key(&first, "F13"),
        KeyAssignment::Action("Alt+Tab".into())
    );
    picker.invoke_dismiss_picker();

    // Deletion confirmations also bind their target, and remain open on failure.
    layers.invoke_request_remove_extra(0);
    layers.invoke_reorder_extra(0, 1);
    assert!(!layers.invoke_remove_extra(0));
    assert!(layers.get_delete_extra() >= 0);
    assert_eq!(document.read().layout().layer_keymaps[&first].keys.len(), 2);
    layers.set_delete_extra(-1);
    layers.invoke_open_dialog(LayerDialog::Delete, -1);
    layers.invoke_choose(1);
    layers.invoke_apply_dialog();
    assert_eq!(layers.get_dialog(), LayerDialog::Delete);
    assert_eq!(layers.get_status().id, "config-edit-conflict");
    assert_eq!(document.read().layout().layers.len(), 2);
    layers.set_dialog(LayerDialog::None);
    layers.invoke_choose(0);

    // Invalid names preserve the draft and allow a corrected retry.
    layers.invoke_open_dialog(LayerDialog::Duplicate, -1);
    layers.set_dialog_name("Second".into());
    layers.invoke_apply_dialog();
    assert_eq!(layers.get_dialog(), LayerDialog::Duplicate);
    assert_eq!(layers.get_dialog_name(), "Second");
    layers.set_dialog_name("Copy".into());
    layers.invoke_apply_dialog();
    assert_eq!(layers.get_dialog(), LayerDialog::None);
    assert_eq!(document.read().layout().layers.len(), 3);
    layers.invoke_choose(0);

    // A failed settings write must not invalidate a pending layer edit.
    layers.invoke_open_dialog(LayerDialog::EditKey, 0);
    picker.invoke_select_behavior(PickerBehavior::Action);
    picker.set_value("KeyZ".into());
    let revision = document.revision();
    let config_path = fixture.paths.config_path();
    let backup = config_path.with_extension("saved");
    let existed = config_path.exists();
    if existed {
        std::fs::rename(&config_path, &backup)?;
    }
    std::fs::create_dir(&config_path)?;
    assert!(
        document
            .edit(View::Settings, |config| config.update_settings(
                |settings| {
                    settings.default_hold_timeout_ms += 1;
                }
            ))
            .is_err()
    );
    assert_eq!(document.revision(), revision);
    std::fs::remove_dir(&config_path)?;
    if existed {
        std::fs::rename(&backup, &config_path)?;
    }
    picker.invoke_apply();
    assert!(!picker.get_opened());
    assert_eq!(
        document.read().layer_key(&first, "Backquote"),
        KeyAssignment::Action("KeyZ".into())
    );

    layers.invoke_open_dialog(LayerDialog::Delete, -1);
    layers.invoke_apply_dialog();
    assert_eq!(
        document.read().layout().rules[0].hold_behavior(),
        lhc_core::profile::model::HoldBehavior::None
    );
    assert!(document.read().layout().rules[0].layer_id.is_empty());
    println!(
        "Layer regressions passed: activation, stale pickers and confirmations, invalid names, failed write and retry"
    );
    Ok(())
}
