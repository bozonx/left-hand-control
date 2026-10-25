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

// Windows offers no manual device path.
#[cfg(not(target_os = "windows"))]
#[test]
fn pickers_suggest_matching_devices_and_fold_other_inputs() {
    let devices = input_devices();
    for (mouse, expected_path) in [(false, "/dev/input/event2"), (true, "/dev/input/event1")] {
        let choices = device_choices(&devices, mouse, "");
        let groups: Vec<_> = choices.iter().map(|choice| choice.group).collect();
        assert_eq!(
            groups,
            [
                DeviceGroup::Unselected,
                DeviceGroup::More,
                DeviceGroup::Suggested,
                DeviceGroup::Other,
                DeviceGroup::Other,
                DeviceGroup::Manual,
            ]
        );
        assert_eq!(choices[1].count, 2);
        assert_eq!(choices[1].source_index, -1);
        assert_eq!(choices[2].path, expected_path);
        assert!(
            devices
                .iter()
                .all(|device| choices.iter().any(|choice| choice.path == device.path))
        );
        let model = ModelRc::new(VecModel::from(choices));
        assert_eq!(filtered_devices(model.clone(), "", false).row_count(), 4);
        let expanded = filtered_devices(model, "", true);
        assert_eq!(expanded.row_count(), 5);
        assert!(
            expanded
                .iter()
                .all(|choice| choice.group != DeviceGroup::More)
        );
    }
}

#[test]
fn virtual_devices_get_their_own_group() {
    let mut devices = input_devices();
    devices[0].is_virtual = true;
    let choices = device_choices(&devices, false, "");
    let power = choices.iter().find(|c| c.label == "Power button").unwrap();
    assert_eq!(power.group, DeviceGroup::VirtualDevice);
}

// Windows offers no manual device path.
#[cfg(not(target_os = "windows"))]
#[test]
fn search_keeps_original_selection_indices_and_matches_names_and_paths() {
    let choices = device_choices(&input_devices(), false, "");
    let model = ModelRc::new(VecModel::from(choices.clone()));
    for query in [" КЛАВИАТУРА ", "event2"] {
        let filtered = filtered_devices(model.clone(), query, false);
        assert_eq!(filtered.row_count(), 3);
        assert_eq!(filtered.row_data(1), Some(choices[2].clone()));
    }
    // Searching looks into folded groups too.
    let filtered = filtered_devices(model.clone(), "mouse", false);
    assert_eq!(filtered.row_data(1), Some(choices[4].clone()));
    let filtered = filtered_devices(model, "no matching device", false);
    assert_eq!(filtered.row_count(), 2);
    assert_eq!(filtered.row_data(0).unwrap().group, DeviceGroup::Unselected);
    assert_eq!(filtered.row_data(1).unwrap().group, DeviceGroup::Manual);
}

// Windows offers no manual device path.
#[cfg(not(target_os = "windows"))]
#[test]
fn saved_unavailable_paths_stay_selected_and_explicit_manual_mode_survives_refresh() {
    let choices = device_choices(&input_devices(), false, "/dev/input/missing");
    let manual = choices.len() as i32 - 1;
    assert_eq!(choices[1].group, DeviceGroup::Missing);
    assert_eq!(choices[1].path, "/dev/input/missing");
    assert_eq!(device_selection(&choices, "", false), (0, false));
    assert_eq!(
        device_selection(&choices, "/dev/input/event2", false),
        (3, false)
    );
    assert_eq!(
        device_selection(&choices, "/dev/input/missing", false),
        (1, false)
    );
    assert_eq!(device_selection(&choices, "", true), (manual, true));
    assert_eq!(
        device_selection(&choices, "/dev/input/other", false),
        (manual, true)
    );
}

#[test]
fn device_rows_show_readable_names_and_short_paths() {
    let sofle = "/dev/input/by-id/usb-MTKB_W-SOFLE_vial:f64c2b3c-event-kbd";
    assert_eq!(device_label("", sofle), "MTKB W-SOFLE vial");
    assert_eq!(device_label("(unknown)", sofle), "MTKB W-SOFLE vial");
    assert_eq!(device_label("Logitech", sofle), "Logitech");
    assert_eq!(
        device_label(
            "",
            "/dev/input/by-id/usb-Logitech_USB_Receiver-if02-event-mouse"
        ),
        "Logitech USB Receiver"
    );
    assert_eq!(short_path(sofle), "by-id · usb-MTKB_W-SOFLE_vial:f64c2b3c");
    assert_eq!(short_path("/dev/input/event18"), "event18");
    assert_eq!(device_label("", "/dev/input/event18"), "event18");
    assert_eq!(short_path("windows:keyboard"), "windows:keyboard");
    let long = short_path(
        "/dev/input/by-path/pci-0000:00:14.3-platform-PNP0C0D:00-very-long-name-for-a-device-event",
    );
    assert!(long.starts_with('…') && long.ends_with("long-name-for-a-device"));
    assert_eq!(long.chars().count(), MAX_DETAIL_CHARS);
}

#[test]
fn without_manual_entry_unknown_paths_are_unselected() {
    let mut choices = device_choices(&input_devices(), false, "");
    if choices
        .last()
        .is_some_and(|choice| choice.group == DeviceGroup::Manual)
    {
        choices.pop();
    }
    assert_eq!(
        device_selection(&choices, "/dev/input/event2", false),
        (2, false)
    );
    assert_eq!(
        device_selection(&choices, "/dev/input/missing", false),
        (0, false)
    );
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
            3 => form.combo = value.into(),
            _ => form.macro_timeout = value.into(),
        }
        assert_eq!(
            form.apply(&base, &mut settings),
            Err(Msg::SettingsTimingInvalid)
        );
        assert_eq!(settings, original);
    }
}
