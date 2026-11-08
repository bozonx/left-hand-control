//! Regression tests for settings.

use super::*;

#[test]
fn matchers_are_added_changed_and_kept_unique() {
    let mut items = Vec::new();
    assert!(save_matcher(&mut items, -1, " Game.exe ", false, false));
    assert!(save_matcher(&mut items, -1, "steam", true, true));
    assert!(!save_matcher(&mut items, -1, "game.EXE", true, true));
    assert!(!save_matcher(&mut items, -1, "  ", true, false));
    assert!(!save_matcher(&mut items, 0, "Steam", false, false));
    assert!(save_matcher(&mut items, 0, "game.exe", true, false));
    assert!(!save_matcher(&mut items, 5, "other", true, false));
    assert!(save_matcher(&mut items, -1, "Игра", false, false));
    assert!(!save_matcher(&mut items, -1, "игра", false, true));
    items.pop();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "game.exe");
    assert!(items[0].only_active_window && !items[0].is_blacklist);
    assert!(items[1].only_active_window && items[1].is_blacklist);
}

fn found(
    devices: Vec<lhc_core::mapper::devices::InputDevice>,
) -> lhc_core::mapper::devices::DeviceDiscovery {
    lhc_core::mapper::devices::DeviceDiscovery {
        devices,
        unavailable: Vec::new(),
    }
}

fn input_devices() -> Vec<lhc_core::mapper::devices::InputDevice> {
    [
        ("Power button", "/dev/input/event0", false, false),
        ("Mouse", "/dev/input/event1", false, true),
        ("Клавиатура", "/dev/input/event2", true, false),
    ]
    .into_iter()
    .map(
        |(name, path, is_keyboard, is_mouse)| lhc_core::mapper::devices::InputDevice {
            name: name.into(),
            path: path.into(),
            is_keyboard,
            is_mouse,
            is_virtual: false,
        },
    )
    .collect()
}

#[test]
fn pickers_suggest_matching_devices_and_list_the_rest_under_all() {
    let devices = input_devices();
    for (mouse, expected_path) in [(false, "/dev/input/event2"), (true, "/dev/input/event1")] {
        let choices = device_choices(&devices, mouse, "");
        let groups: Vec<_> = choices.iter().map(|choice| choice.group).collect();
        assert_eq!(
            groups,
            [
                DeviceGroup::Suggested,
                DeviceGroup::Other,
                DeviceGroup::Other
            ]
        );
        assert_eq!(choices[0].path, expected_path);
        let model = ModelRc::new(VecModel::from(choices));
        assert_eq!(filtered_devices(model.clone(), "", false).row_count(), 1);
        assert_eq!(filtered_devices(model, "", true).row_count(), 3);
    }
}

#[test]
fn virtual_keyboards_are_not_suggested() {
    let mut devices = input_devices();
    devices[2].is_virtual = true;
    let choices = device_choices(&devices, false, "");
    let keyboard = choices.iter().find(|c| c.label == "Клавиатура").unwrap();
    assert_eq!(keyboard.group, DeviceGroup::VirtualDevice);
    let model = ModelRc::new(VecModel::from(choices));
    assert_eq!(filtered_devices(model, "", false).row_count(), 0);
}

#[test]
fn search_matches_names_and_paths_within_the_tab() {
    let choices = device_choices(&input_devices(), false, "");
    let model = ModelRc::new(VecModel::from(choices.clone()));
    for query in [" КЛАВИАТУРА ", "event2"] {
        let filtered = filtered_devices(model.clone(), query, false);
        assert_eq!(filtered.row_count(), 1);
        assert_eq!(filtered.row_data(0), Some(choices[0].clone()));
    }
    assert_eq!(
        filtered_devices(model.clone(), "mouse", false).row_count(),
        0
    );
    let filtered = filtered_devices(model, "mouse", true);
    assert_eq!(filtered.row_count(), 1);
    assert_eq!(filtered.row_data(0).unwrap().path, "/dev/input/event1");
}

#[test]
fn saved_unavailable_path_stays_listed_as_not_connected() {
    let choices = device_choices(&input_devices(), false, " /dev/input/missing ");
    assert_eq!(choices[0].group, DeviceGroup::Missing);
    assert_eq!(choices[0].path, "/dev/input/missing");
    let model = ModelRc::new(VecModel::from(choices));
    // Shown in both tabs, so the current choice is never hidden.
    assert_eq!(filtered_devices(model.clone(), "", false).row_count(), 2);
    assert_eq!(filtered_devices(model, "", true).row_count(), 4);
    let connected = device_choices(&input_devices(), false, "/dev/input/event2");
    assert!(connected.iter().all(|c| c.group != DeviceGroup::Missing));
}

#[cfg(unix)]
#[test]
fn typed_paths_find_the_device_they_open() {
    let temp = tempfile::tempdir().unwrap();
    let event = temp.path().join("event7");
    let link = temp.path().join("usb-Kbd-event-kbd");
    std::fs::write(&event, "").unwrap();
    std::os::unix::fs::symlink(&event, &link).unwrap();
    let mut devices = input_devices();
    devices[2].path = link.to_string_lossy().into_owned();
    let model = ModelRc::new(VecModel::from(device_choices(&devices, false, "")));
    let found = |path: &str| {
        let found = find_device(model.clone(), path);
        (found.index, found.exact)
    };

    let index = model
        .iter()
        .position(|c| c.path == devices[2].path)
        .unwrap() as i32;
    assert_eq!(found(&format!(" {} ", devices[2].path)), (index, true));
    // The event node is the same keyboard under its unstable name.
    assert_eq!(found(&event.to_string_lossy()), (index, false));
    assert_eq!(found("/dev/input/nothing-here"), (-1, false));
    assert_eq!(found("  "), (-1, false));
}

#[test]
fn suggested_rows_sharing_a_name_keep_their_path() {
    let mut devices = input_devices();
    devices[0].name = "Клавиатура".into();
    devices[0].is_keyboard = true;
    let choices = device_choices(&devices, false, "");
    assert!(
        choices
            .iter()
            .filter(|c| c.group == DeviceGroup::Suggested)
            .all(|c| c.ambiguous)
    );
    let unique = device_choices(&input_devices(), false, "");
    assert!(unique.iter().all(|c| !c.ambiguous));
}

#[test]
fn virtual_group_rows_start_under_a_heading() {
    let mut devices = input_devices();
    devices[0].is_virtual = true;
    let model = ModelRc::new(VecModel::from(device_choices(&devices, false, "")));
    // Suggested, other, then the virtual group with its heading.
    assert_eq!(device_offset(model.clone(), 2, 44, 28), 88);
    assert_eq!(device_offset(model, 3, 44, 28), 88 + 44 + 28);
}

#[test]
fn saving_keeps_fields_changed_elsewhere() {
    let mut settings = AppSettings::default();
    let base = Form::from_settings(&settings);
    let mut form = base.clone();
    form.hold = "310".into();
    // Another process changed the double-tap timeout meanwhile.
    settings.default_double_tap_timeout_ms = 999;
    form.apply(&base, &mut settings).unwrap();
    assert_eq!(settings.default_hold_timeout_ms, 310);
    assert_eq!(settings.default_double_tap_timeout_ms, 999);
}

#[test]
fn invalid_numbers_are_rejected() {
    let mut settings = AppSettings::default();
    let base = Form::from_settings(&settings);
    let mut form = base.clone();
    form.macro_pause = "-5".into();
    assert_eq!(
        form.apply(&base, &mut settings)
            .map_err(|problem| problem.message),
        Err(Msg::SettingsTimingInvalid)
    );
}

#[test]
fn empty_device_paths_are_none() {
    let mut settings = AppSettings {
        input_mouse_device_path: Some("/dev/old".into()),
        ..AppSettings::default()
    };
    let base = Form::from_settings(&settings);
    let mut form = base.clone();
    form.keyboard = "/dev/input/event3".into();
    form.mouse = " ".into();
    form.apply(&base, &mut settings).unwrap();
    assert_eq!(
        settings.input_device_path.as_deref(),
        Some("/dev/input/event3")
    );
    assert_eq!(settings.input_mouse_device_path, None);
}

#[test]
fn out_of_range_timings_do_not_modify_settings() {
    let mut settings = AppSettings::default();
    let original = settings.clone();
    let base = Form::from_settings(&settings);
    for (field, value) in [(0, "0"), (1, "1001"), (2, "2001"), (3, "501"), (4, "999")] {
        let mut form = base.clone();
        match field {
            0 => form.hold = value.into(),
            1 => form.double_tap = value.into(),
            2 => form.macro_pause = value.into(),
            3 => form.trigger_chord = value.into(),
            _ => form.macro_timeout = value.into(),
        }
        assert_eq!(
            form.apply(&base, &mut settings)
                .map_err(|problem| problem.message),
            Err(Msg::SettingsTimingInvalid)
        );
        assert_eq!(settings, original);
    }
}

#[test]
fn saving_a_form_without_a_window_preserves_other_edits_and_rejects_invalid_input() {
    let (_dir, document) = super::super::test_document();
    let base = Form::from_settings(document.read().settings());
    let mut state = State {
        base: Some(base.clone()),
        ..State::default()
    };
    let mut form = base.clone();
    form.hold = "0".into();
    assert_eq!(
        persist(&document, &mut state, &form),
        (false, Msg::SettingsTimingInvalid)
    );
    assert!(state.base == Some(base.clone()));
    document
        .edit(View::Settings, |config| {
            config.update_settings(|settings| {
                settings.layer_hint = !settings.layer_hint;
            })
        })
        .unwrap();
    let layer_hint = document.read().settings().layer_hint;
    form.hold = "350".into();
    assert_eq!(persist(&document, &mut state, &form), (true, Msg::None));
    assert!(state.base.is_none());
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
    assert_eq!(document.read().settings().layer_hint, layer_hint);
}

#[test]
fn write_failure_keeps_the_form_baseline_and_allows_retry() {
    let (dir, document) = super::super::test_document();
    let base = Form::from_settings(document.read().settings());
    let mut state = State {
        base: Some(base.clone()),
        ..State::default()
    };
    let mut form = base.clone();
    form.hold = "350".into();
    std::fs::remove_dir_all(dir.path().join("settings")).unwrap();
    std::fs::write(dir.path().join("settings"), "blocks directory creation").unwrap();
    assert!(!persist(&document, &mut state, &form).0);
    assert!(state.base == Some(base));
    assert_ne!(document.read().settings().default_hold_timeout_ms, 350);
    std::fs::remove_file(dir.path().join("settings")).unwrap();
    assert_eq!(persist(&document, &mut state, &form), (true, Msg::None));
    assert!(state.base.is_none());
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
}

#[test]
fn invalid_timing_does_not_block_independent_settings_and_can_be_retried() {
    let (_dir, document) = super::super::test_document();
    let base = Form::from_settings(document.read().settings());
    let mut state = State {
        base: Some(base.clone()),
        ..State::default()
    };
    let mut form = base.clone();
    form.hold = String::new();
    form.high_contrast = (base.high_contrast + 1) % 3;
    form.keyboard = "/dev/input/new".into();
    assert_eq!(
        persist(&document, &mut state, &form),
        (true, Msg::SettingsTimingInvalid)
    );
    let loaded = Form::from_settings(document.read().settings());
    assert_eq!(loaded.high_contrast, form.high_contrast);
    assert_eq!(loaded.keyboard, form.keyboard);
    assert_eq!(loaded.hold, base.hold);
    let mut rebased = form.rebase(&base, &loaded);
    assert!(rebased.hold.is_empty());
    state.base = Some(loaded);
    rebased.hold = "350".into();
    assert_eq!(persist(&document, &mut state, &rebased), (true, Msg::None));
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
}

#[test]
fn document_updates_merge_into_untouched_fields_while_typing() {
    let base = Form::from_settings(&AppSettings::default());
    let mut draft = base.clone();
    draft.hold = String::new();
    let mut loaded = base.clone();
    loaded.double_tap = "999".into();
    loaded.reduce_motion = (base.reduce_motion + 1) % 3;
    let merged = draft.rebase(&base, &loaded);
    assert!(merged.hold.is_empty());
    assert_eq!(merged.double_tap, "999");
    assert_eq!(merged.reduce_motion, loaded.reduce_motion);
    let mut cancelled = merged;
    cancelled.hold = base.hold.clone();
    assert_eq!(cancelled.rebase(&loaded, &loaded), loaded);
}

#[test]
fn equivalent_numeric_input_is_normalized_without_a_write() {
    let (_dir, document) = super::super::test_document();
    let base = Form::from_settings(document.read().settings());
    let mut form = base.clone();
    form.hold = format!(" {} ", base.hold);
    let mut state = State {
        base: Some(base),
        ..State::default()
    };
    assert_eq!(persist(&document, &mut state, &form), (false, Msg::None));
    assert!(state.base.is_none());
}

#[test]
fn device_selection_retries_write_errors_and_applies_only_saved_changes() {
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    struct Platform;
    impl slint::platform::Platform for Platform {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
        }
    }
    slint::platform::set_platform(Box::new(Platform)).unwrap();
    let (dir, document) = super::super::test_document();
    let ui = SettingsWindow::new().unwrap();
    let e = ui.global::<SettingsEditor>();
    // Keep this test independent of the host's input devices and worker scheduling.
    e.set_devices_loading(true);
    bind(&ui, &document);
    discover_devices(&ui, || {
        panic!("a second discovery must not run while loading")
    });
    publish_devices(&ui, Err("discovery failed".into()));
    assert!(!e.get_devices_loading());
    assert_ne!(e.get_device_discovery_error().id, "");
    publish_devices(&ui, Ok(found(input_devices())));
    assert_eq!(e.get_device_discovery_error().id, "");
    assert_eq!(e.get_input_devices().row_count(), 3);
    e.set_devices_loading(true);
    let changes = Rc::new(std::cell::Cell::new(0));
    let observed = changes.clone();
    ui.global::<AppState>()
        .on_devices_changed(move || observed.set(observed.get() + 1));
    e.set_hold_timeout(String::new().into());
    std::fs::remove_dir_all(dir.path().join("settings")).unwrap();
    std::fs::write(dir.path().join("settings"), "blocks saving").unwrap();
    assert!(!e.invoke_select_device("/dev/input/new".into()));
    assert_eq!(changes.get(), 0);
    assert_eq!(e.get_keyboard_device_path(), "");
    std::fs::remove_file(dir.path().join("settings")).unwrap();
    assert!(e.invoke_select_device("/dev/input/new".into()));
    assert_eq!(changes.get(), 1);
    assert_eq!(
        document.read().settings().input_device_path.as_deref(),
        Some("/dev/input/new")
    );
    e.set_hold_timeout("350".into());
    e.invoke_save();
    assert_eq!(changes.get(), 1);
    e.set_devices_loading(false);
    e.set_reduce_motion_index((e.get_reduce_motion_index() + 1) % 3);
    e.invoke_save();
    assert!(
        !e.get_devices_loading(),
        "unrelated saves must not enumerate devices"
    );
    e.set_hold_timeout(String::new().into());
    e.set_tab(crate::ui::SettingsTab::General);
    assert!(!e.invoke_flush_pending());
    assert_eq!(e.get_tab(), crate::ui::SettingsTab::KeyBehavior);
}

fn bound_settings() -> (tempfile::TempDir, Rc<Document>, SettingsWindow) {
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
    struct Platform;
    impl slint::platform::Platform for Platform {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer))
        }
    }
    slint::platform::set_platform(Box::new(Platform)).unwrap();
    let (dir, document) = super::super::test_document();
    let ui = SettingsWindow::new().unwrap();
    ui.global::<SettingsEditor>().set_devices_loading(true);
    super::super::bind_document(&ui, &document);
    (dir, document, ui)
}

#[test]
fn hidden_long_hold_draft_routes_to_its_field_and_can_be_discarded() {
    let (_dir, document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    let original = e.get_long_hold_timeout();
    e.set_in_development(true);
    e.invoke_save();
    e.set_long_hold_timeout("".into());
    e.invoke_save();
    assert_eq!(e.get_problem_field(), SettingsField::LongHold);
    e.set_in_development(false);
    e.invoke_save();
    assert!(!document.read().settings().in_development);
    e.set_tab(crate::ui::SettingsTab::General);
    assert!(!e.invoke_flush_pending());
    assert_eq!(e.get_tab(), crate::ui::SettingsTab::KeyBehavior);
    assert_eq!(e.get_problem_field(), SettingsField::LongHold);
    assert!(e.get_long_hold_timeout().is_empty());
    e.invoke_discard_pending();
    assert_eq!(e.get_long_hold_timeout(), original);
    assert_eq!(e.get_problem_field(), SettingsField::None);
    assert_eq!(e.get_message().id, "");
    assert!(e.invoke_flush_pending());
}

#[test]
fn returning_an_invalid_field_to_the_saved_value_clears_its_problem() {
    let (_dir, _document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    let saved = e.get_hold_timeout();
    e.set_hold_timeout("".into());
    assert!(!e.invoke_flush_pending());
    assert_eq!(e.get_problem_field(), SettingsField::Hold);
    e.set_hold_timeout(saved);
    assert!(e.invoke_flush_pending());
    assert_eq!(e.get_problem_field(), SettingsField::None);
}

#[test]
fn quitting_can_retry_a_write_failure_or_discard_only_unsaved_settings() {
    use crate::ui::{LibraryDialog, PendingProfileAction, ProfileLibrary};
    use std::cell::Cell;
    let (dir, document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    let original = e.get_hold_timeout();
    let quit = Rc::new(Cell::new(false));
    let requested = quit.clone();
    ui.global::<ProfileLibrary>()
        .on_quit(move || requested.set(true));
    std::fs::remove_dir_all(dir.path().join("settings")).unwrap();
    std::fs::write(dir.path().join("settings"), "blocks saving").unwrap();
    e.set_hold_timeout("350".into());
    assert!(!ui.invoke_prepare_quit());
    assert!(ui.get_settings_quit_dialog());
    ui.invoke_continue_settings_quit(false);
    assert!(!quit.get());
    assert!(ui.get_settings_quit_dialog());
    std::fs::remove_file(dir.path().join("settings")).unwrap();
    ui.invoke_continue_settings_quit(false);
    assert!(quit.get());
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
    quit.set(false);

    // Discarding settings must still ask about independent profile changes.
    document
        .edit(View::Rules, |config| config.create_layer("Keep me", ""))
        .unwrap();
    e.set_hold_timeout("".into());
    assert!(!ui.invoke_prepare_quit());
    ui.invoke_continue_settings_quit(true);
    assert!(!quit.get());
    assert_eq!(e.get_hold_timeout(), "350");
    assert_ne!(e.get_hold_timeout(), original);
    let library = ui.global::<ProfileLibrary>();
    assert_eq!(library.get_dialog(), LibraryDialog::Unsaved);
    assert_eq!(library.get_pending_action(), PendingProfileAction::Quit);
    assert!(
        document
            .read()
            .profile()
            .layers
            .iter()
            .any(|layer| layer.name == "Keep me")
    );
}

#[test]
fn discovery_failure_preserves_last_success_and_recovery_clears_the_error() {
    let (_dir, _document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    publish_devices(&ui, Ok(found(input_devices())));
    let previous: Vec<_> = e.get_input_devices().iter().collect();
    e.set_devices_loading(true);
    publish_devices(&ui, Err("access denied".into()));
    assert!(!e.get_devices_loading());
    assert_eq!(e.get_input_devices().iter().collect::<Vec<_>>(), previous);
    assert_eq!(
        e.get_device_discovery_error().id,
        if cfg!(target_os = "linux") {
            "input-discovery-failed"
        } else {
            "mapper-unsupported"
        }
    );
    publish_devices(&ui, Ok(found(Vec::new())));
    assert_eq!(e.get_input_devices().row_count(), 0);
    assert_eq!(e.get_device_discovery_error().id, "");
}

#[test]
fn partially_denied_discovery_lists_open_devices_and_reports_access() {
    let (_dir, _document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    let mut discovery = found(input_devices());
    discovery
        .unavailable
        .push(lhc_core::mapper::devices::UnavailableDevice {
            path: "/dev/input/event3".into(),
            error: "open /dev/input/event3: Permission denied".into(),
            denied: true,
        });
    publish_devices(&ui, Ok(discovery));
    assert_eq!(e.get_input_devices().row_count(), 3);
    assert_eq!(
        e.get_device_discovery_error().id,
        "input-access-input-denied"
    );
}

#[test]
fn mapper_start_saves_valid_drafts_and_routes_invalid_drafts_to_settings() {
    let (_dir, document, ui) = bound_settings();
    let e = ui.global::<SettingsEditor>();
    e.set_hold_timeout("350".into());
    assert!(super::super::prepare_mapper_start(&ui));
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
    e.set_hold_timeout("".into());
    ui.set_page(crate::ui::Page::Profiles);
    assert!(!super::super::prepare_mapper_start(&ui));
    assert_eq!(ui.get_page(), crate::ui::Page::Settings);
    assert_eq!(e.get_tab(), crate::ui::SettingsTab::KeyBehavior);
    assert_eq!(document.read().settings().default_hold_timeout_ms, 350);
}
