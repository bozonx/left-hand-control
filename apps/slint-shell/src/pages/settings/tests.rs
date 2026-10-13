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
        },
    )
    .collect()
}

#[test]
fn pickers_suggest_matching_devices_and_keep_other_inputs_available() {
    let devices = input_devices();
    for (mouse, expected_path) in [(false, "/dev/input/event2"), (true, "/dev/input/event1")] {
        let choices = device_choices(&devices, mouse);
        assert_eq!(choices[0].group, DeviceGroup::Unselected);
        assert_eq!(choices[1].group, DeviceGroup::Suggested);
        assert_eq!(choices[1].path, expected_path);
        assert_eq!(choices[2].group, DeviceGroup::Other);
        assert_eq!(choices[3].group, DeviceGroup::Other);
        assert_eq!(choices[4].group, DeviceGroup::Manual);
        assert!(
            devices
                .iter()
                .all(|device| choices.iter().any(|choice| choice.path == device.path))
        );
    }
}

#[test]
fn search_keeps_original_selection_indices_and_matches_names_and_paths() {
    let choices = device_choices(&input_devices(), false);
    let model = ModelRc::new(VecModel::from(choices.clone()));
    for query in [" КЛАВИАТУРА ", "event2"] {
        let filtered = filtered_devices(model.clone(), query);
        assert_eq!(filtered.row_count(), 3);
        assert_eq!(filtered.row_data(1), Some(choices[1].clone()));
    }
    let filtered = filtered_devices(model.clone(), "mouse");
    assert_eq!(filtered.row_data(1), Some(choices[3].clone()));
    let filtered = filtered_devices(model, "no matching device");
    assert_eq!(filtered.row_count(), 2);
    assert_eq!(filtered.row_data(0).unwrap().group, DeviceGroup::Unselected);
    assert_eq!(filtered.row_data(1).unwrap().group, DeviceGroup::Manual);
}

#[test]
fn saved_unavailable_paths_use_manual_input_and_explicit_manual_mode_survives_refresh() {
    let choices = device_choices(&input_devices(), false);
    let manual = choices.len() as i32 - 1;
    assert_eq!(device_selection(&choices, "", false), (0, false));
    assert_eq!(
        device_selection(&choices, "/dev/input/event2", false),
        (1, false)
    );
    assert_eq!(
        device_selection(&choices, "/dev/input/missing", false),
        (manual, true)
    );
    assert_eq!(device_selection(&choices, "", true), (manual, true));
    assert_eq!(
        device_selection(&choices, "/dev/input/event2", true),
        (manual, true)
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
    assert_eq!(form.apply(&base, &mut settings), Err(Msg::TimeoutInvalid));
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
