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
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "game.exe");
    assert!(items[0].only_active_window && !items[0].is_blacklist);
    assert!(items[1].only_active_window && items[1].is_blacklist);
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
        form.apply(&base, &mut settings),
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
            form.apply(&base, &mut settings),
            Err(Msg::SettingsTimingInvalid)
        );
        assert_eq!(settings, original);
    }
}
