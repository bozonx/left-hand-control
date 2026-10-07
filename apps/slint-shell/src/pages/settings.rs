use super::{APPEARANCES, LOCALES, choice, choice_index, strings};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        CapabilityRow, DeviceChoice, DeviceChoices, DeviceGroup, ProcessRow, SettingsEditor,
        SettingsWindow,
    },
};
use lhc_core::profile::model::{AppSettings, GameModeProcessMatcher};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Linux text injection backends, in the order the page lists them.
const TEXT_MODES: [&str; 6] = [
    "libei",
    "libei-pure",
    "keycode",
    "clipboard",
    "ydotool",
    "xdotool",
];
const TAP_DECISIONS: [&str; 2] = ["permissiveHold", "holdOnOtherKeyPress"];

/// The form as the user sees it. Numbers stay text until saved.
#[derive(Clone, Debug, PartialEq)]
struct Form {
    launch_on_startup: bool,
    appearance: i32,
    locale: i32,
    tap_decision: i32,
    hold: String,
    double_tap: String,
    macro_pause: String,
    modifier_delay: String,
    use_gamemoded: bool,
    use_fullscreen: bool,
    matchers: Vec<GameModeProcessMatcher>,
    text_mode: i32,
    ydotool: String,
    xdotool: String,
    keyboard: String,
    mouse: String,
}

fn parse_ms(value: &str) -> Result<u64, Msg> {
    value.trim().parse().map_err(|_| Msg::TimeoutInvalid)
}

fn path(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

impl Form {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            launch_on_startup: settings.launch_on_startup,
            appearance: choice_index(&APPEARANCES, &settings.appearance),
            locale: choice_index(&LOCALES, &settings.locale),
            tap_decision: choice_index(&TAP_DECISIONS, &settings.tap_decision.as_str()),
            hold: settings.default_hold_timeout_ms.to_string(),
            double_tap: settings.default_double_tap_timeout_ms.to_string(),
            macro_pause: settings.default_macro_step_pause_ms.to_string(),
            modifier_delay: settings.default_macro_modifier_delay_ms.to_string(),
            use_gamemoded: settings.game_mode.use_gamemoded,
            use_fullscreen: settings.game_mode.use_fullscreen,
            matchers: settings.game_mode.process_matchers.clone(),
            text_mode: choice_index(
                &TEXT_MODES,
                &settings
                    .linux_wayland_text_mode
                    .as_deref()
                    .unwrap_or(TEXT_MODES[0]),
            ),
            ydotool: settings.linux_ydotool_path.clone(),
            xdotool: settings.linux_xdotool_path.clone(),
            keyboard: settings.input_device_path.clone().unwrap_or_default(),
            mouse: settings.input_mouse_device_path.clone().unwrap_or_default(),
        }
    }

    fn read(ui: &SettingsWindow, matchers: &[GameModeProcessMatcher]) -> Self {
        let e = ui.global::<SettingsEditor>();
        Self {
            launch_on_startup: e.get_launch_on_startup(),
            appearance: e.get_appearance_index(),
            locale: e.get_locale_index(),
            tap_decision: e.get_tap_decision_index(),
            hold: e.get_hold_timeout().into(),
            double_tap: e.get_double_tap_timeout().into(),
            macro_pause: e.get_macro_pause().into(),
            modifier_delay: e.get_modifier_delay().into(),
            use_gamemoded: e.get_use_gamemoded(),
            use_fullscreen: e.get_use_fullscreen(),
            matchers: matchers.to_vec(),
            text_mode: e.get_text_mode_index(),
            ydotool: e.get_ydotool_path().into(),
            xdotool: e.get_xdotool_path().into(),
            keyboard: e.get_keyboard_device_path().into(),
            mouse: e.get_mouse_device_path().into(),
        }
    }

    fn show(&self, ui: &SettingsWindow) {
        let e = ui.global::<SettingsEditor>();
        e.set_launch_on_startup(self.launch_on_startup);
        e.set_appearance_index(self.appearance);
        e.set_locale_index(self.locale);
        e.set_tap_decision_index(self.tap_decision);
        e.set_hold_timeout(self.hold.clone().into());
        e.set_double_tap_timeout(self.double_tap.clone().into());
        e.set_macro_pause(self.macro_pause.clone().into());
        e.set_modifier_delay(self.modifier_delay.clone().into());
        e.set_use_gamemoded(self.use_gamemoded);
        e.set_use_fullscreen(self.use_fullscreen);
        e.set_process_matchers(rows(&self.matchers));
        e.set_text_mode_index(self.text_mode);
        e.set_ydotool_path(self.ydotool.clone().into());
        e.set_xdotool_path(self.xdotool.clone().into());
        if e.get_keyboard_device_path() != self.keyboard {
            e.set_keyboard_manual(false);
        }
        if e.get_mouse_device_path() != self.mouse {
            e.set_mouse_manual(false);
        }
        e.set_keyboard_device_path(self.keyboard.clone().into());
        e.set_mouse_device_path(self.mouse.clone().into());
    }

    /// Write the fields that differ from `base` into `settings`.
    fn apply(&self, base: &Form, settings: &mut AppSettings) -> Result<(), Msg> {
        let numbers = [
            parse_ms(&self.hold)?,
            parse_ms(&self.double_tap)?,
            parse_ms(&self.macro_pause)?,
            parse_ms(&self.modifier_delay)?,
        ];
        macro_rules! changed {
            ($field:ident => $apply:expr) => {
                if self.$field != base.$field {
                    $apply;
                }
            };
        }
        changed!(launch_on_startup => settings.launch_on_startup = self.launch_on_startup);
        changed!(appearance => settings.appearance = choice(&APPEARANCES, self.appearance));
        changed!(locale => settings.locale = choice(&LOCALES, self.locale));
        changed!(tap_decision => settings.tap_decision = choice(&TAP_DECISIONS, self.tap_decision).into());
        changed!(hold => settings.default_hold_timeout_ms = numbers[0]);
        changed!(double_tap => settings.default_double_tap_timeout_ms = numbers[1]);
        changed!(macro_pause => settings.default_macro_step_pause_ms = numbers[2]);
        changed!(modifier_delay => settings.default_macro_modifier_delay_ms = numbers[3]);
        changed!(use_gamemoded => settings.game_mode.use_gamemoded = self.use_gamemoded);
        changed!(use_fullscreen => settings.game_mode.use_fullscreen = self.use_fullscreen);
        changed!(matchers => settings.game_mode.process_matchers = self.matchers.clone());
        changed!(text_mode => settings.linux_wayland_text_mode = Some(choice(&TEXT_MODES, self.text_mode).into()));
        changed!(ydotool => settings.linux_ydotool_path = self.ydotool.trim().into());
        changed!(xdotool => settings.linux_xdotool_path = self.xdotool.trim().into());
        changed!(keyboard => settings.input_device_path = path(&self.keyboard));
        changed!(mouse => settings.input_mouse_device_path = path(&self.mouse));
        Ok(())
    }
}

fn rows(items: &[GameModeProcessMatcher]) -> ModelRc<ProcessRow> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .map(|item| ProcessRow {
                name: item.name.clone().into(),
                only_active: item.only_active_window,
                blacklist: item.is_blacklist,
            })
            .collect::<Vec<_>>(),
    ))
}

/// The form as last loaded from the document, and the draft process list.
#[derive(Default)]
struct State {
    base: Option<Form>,
    matchers: Vec<GameModeProcessMatcher>,
    autosave: slint::Timer,
}

/// Load the form unless the user has unsaved edits in it.
fn refresh(ui: &SettingsWindow, document: &Document, state: &mut State, force: bool) {
    let loaded = Form::from_settings(document.read().settings());
    if let Some(base) = &state.base {
        let pending = Form::read(ui, &state.matchers) != *base;
        if (pending && !force) || *base == loaded {
            return;
        }
    }
    loaded.show(ui);
    state.matchers = loaded.matchers.clone();
    state.base = Some(loaded);
    refresh_devices(ui);
}

fn device_choices(
    devices: &[lhc_core::mapper_types::InputDevice],
    mouse: bool,
) -> Vec<DeviceChoice> {
    let mut choices = vec![DeviceChoice {
        group: DeviceGroup::Unselected,
        ..Default::default()
    }];
    for group in [DeviceGroup::Suggested, DeviceGroup::Other] {
        for device in devices {
            let suggested = if mouse {
                device.is_mouse
            } else {
                device.is_keyboard
            };
            if suggested != (group == DeviceGroup::Suggested) {
                continue;
            }
            choices.push(DeviceChoice {
                label: if device.name.is_empty() {
                    device.path.clone()
                } else {
                    format!("{} · {}", device.name, device.path)
                }
                .into(),
                path: device.path.clone().into(),
                group,
                source_index: choices.len() as i32,
            });
        }
    }
    choices.push(DeviceChoice {
        group: DeviceGroup::Manual,
        source_index: choices.len() as i32,
        ..Default::default()
    });
    choices
}

fn device_selection(choices: &[DeviceChoice], path: &str, manual: bool) -> (i32, bool) {
    if !manual && let Some(index) = choices.iter().position(|choice| choice.path == path) {
        (index as i32, false)
    } else {
        (choices.len() as i32 - 1, true)
    }
}

fn filtered_devices(choices: ModelRc<DeviceChoice>, query: &str) -> ModelRc<DeviceChoice> {
    let query = query.trim().to_lowercase();
    ModelRc::new(VecModel::from(
        choices
            .iter()
            .filter(|choice| {
                matches!(choice.group, DeviceGroup::Manual | DeviceGroup::Unselected)
                    || choice.label.to_lowercase().contains(&query)
                    || choice.path.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>(),
    ))
}

fn device_offset(choices: ModelRc<DeviceChoice>, index: i32) -> i32 {
    let mut group = DeviceGroup::Unselected;
    choices
        .iter()
        .take(index.max(0) as usize)
        .map(|choice| {
            let heading = choice.group != DeviceGroup::Unselected && choice.group != group;
            group = choice.group;
            36 + if heading { 28 } else { 0 }
        })
        .sum()
}

fn refresh_devices(ui: &SettingsWindow) {
    let e = ui.global::<SettingsEditor>();
    let devices = lhc_core::mapper::runtime::list_input_devices().unwrap_or_else(|error| {
        log::warn!("input device discovery: {error}");
        Vec::new()
    });
    let keyboards = device_choices(&devices, false);
    let (selected, manual) = device_selection(
        &keyboards,
        &e.get_keyboard_device_path(),
        e.get_keyboard_manual(),
    );
    e.set_input_devices(ModelRc::new(VecModel::from(keyboards)));
    e.set_selected_device(selected);
    e.set_keyboard_manual(manual);
    let mice = device_choices(&devices, true);
    let (selected, manual) =
        device_selection(&mice, &e.get_mouse_device_path(), e.get_mouse_manual());
    e.set_mouse_devices(ModelRc::new(VecModel::from(mice)));
    e.set_selected_mouse(selected);
    e.set_mouse_manual(manual);
}

fn save(ui: &SettingsWindow, document: &Document, state: &mut State) -> Msg {
    let Some(base) = state.base.clone() else {
        return Msg::LoadConfigFirst;
    };
    let form = Form::read(ui, &state.matchers);
    if form == base {
        return Msg::None;
    }
    let mut settings = document.read().settings().clone();
    if let Err(error) = form.apply(&base, &mut settings) {
        return error;
    }
    let saved = document.edit(View::Settings, |config| {
        config.update_settings(|current| *current = settings)
    });
    match saved {
        Ok(saved) => {
            state.base = None;
            refresh(ui, document, state, true);
            saved.message(Msg::None)
        }
        Err(error) => Msg::from(&error),
    }
}

fn refresh_platform(ui: &SettingsWindow) {
    let editor = ui.global::<SettingsEditor>();
    if editor.get_platform_busy() {
        return;
    }
    editor.set_platform_busy(true);
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let platform = lhc_core::platform::info();
        if let Err(error) = weak.upgrade_in_event_loop(move |ui| {
            let editor = ui.global::<SettingsEditor>();
            editor.set_platform_summary(
                platform
                    .linux
                    .map_or_else(
                        || platform.os.to_owned(),
                        |linux| {
                            format!(
                                "{} · {} · {}",
                                platform.os, linux.desktop, linux.session_type
                            )
                        },
                    )
                    .into(),
            );
            let capabilities = platform.capabilities;
            editor.set_capabilities(ModelRc::new(VecModel::from(
                [
                    capabilities.key_interception,
                    capabilities.literal_injection,
                    capabilities.layout_detection,
                    capabilities.system_actions,
                ]
                .into_iter()
                .map(|capability| CapabilityRow {
                    supported: capability.supported,
                    available: capability.available,
                    status: if !capability.supported {
                        Msg::CapabilityUnsupported
                    } else if capability.available {
                        Msg::CapabilityAvailable
                    } else {
                        Msg::CapabilityUnavailable
                    }
                    .to_ui(),
                    detail: capability.detail.unwrap_or_default().into(),
                })
                .collect::<Vec<_>>(),
            )));
            editor.set_platform_busy(false);
        }) {
            log::warn!("platform status: {error}");
        }
    });
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state = Rc::new(RefCell::new(State::default()));
    let e = ui.global::<SettingsEditor>();
    let choices = ui.global::<DeviceChoices>();
    choices.on_filter(|choices, query| filtered_devices(choices, &query));
    choices.on_offset(device_offset);
    e.set_text_modes(strings(TEXT_MODES));
    {
        let config = document.read();
        e.set_settings_dir(config.paths().settings_dir().display().to_string().into());
        e.set_layouts_dir(config.paths().layouts_dir().display().to_string().into());
    }
    refresh_platform(ui);
    let weak = ui.as_weak();
    e.on_refresh_platform(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_platform(&ui);
        }
    });
    refresh(ui, document, &mut state.borrow_mut(), true);

    let weak = ui.as_weak();
    let shared = state.clone();
    document.subscribe(View::Settings, move |document| {
        // Busy: this page's own save is running and refreshes by itself.
        if let (Some(ui), Ok(mut state)) = (weak.upgrade(), shared.try_borrow_mut()) {
            refresh(&ui, document, &mut state, false);
        }
    });

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            shared.borrow().autosave.stop();
            let message = save(&ui, &doc, &mut shared.borrow_mut());
            ui.global::<SettingsEditor>().set_message(message.to_ui());
        }
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_schedule_save(move || {
        let weak = weak.clone();
        shared.borrow().autosave.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(500),
            move || {
                if let Some(ui) = weak.upgrade() {
                    ui.global::<SettingsEditor>().invoke_save();
                }
            },
        );
    });

    let weak = ui.as_weak();
    e.on_refresh_devices(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_devices(&ui);
        }
    });

    let weak = ui.as_weak();
    e.on_select_device(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let Some(choice) = usize::try_from(index)
            .ok()
            .and_then(|index| e.get_input_devices().row_data(index))
        else {
            return;
        };
        e.set_selected_device(index);
        e.set_keyboard_manual(choice.group == DeviceGroup::Manual);
        if choice.group != DeviceGroup::Manual {
            e.set_keyboard_device_path(choice.path);
            e.invoke_save();
        }
    });

    let weak = ui.as_weak();
    e.on_select_mouse(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let Some(choice) = usize::try_from(index)
            .ok()
            .and_then(|index| e.get_mouse_devices().row_data(index))
        else {
            return;
        };
        e.set_selected_mouse(index);
        e.set_mouse_manual(choice.group == DeviceGroup::Manual);
        if choice.group != DeviceGroup::Manual {
            e.set_mouse_device_path(choice.path);
            e.invoke_save();
        }
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_select_process(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(item) = usize::try_from(index)
            .ok()
            .and_then(|index| shared.borrow().matchers.get(index).cloned())
        else {
            return;
        };
        let e = ui.global::<SettingsEditor>();
        e.set_selected_process(index);
        e.set_process_name(item.name.into());
        e.set_process_only_active(item.only_active_window);
        e.set_process_blacklist(item.is_blacklist);
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_add_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let name = e.get_process_name().trim().to_owned();
        if name.is_empty() {
            e.set_message(Msg::ProcessNameRequired.to_ui());
            return;
        }
        let mut state = shared.borrow_mut();
        state.matchers.push(GameModeProcessMatcher {
            id: lhc_core::profile::ids::generate("process-"),
            name,
            only_active_window: e.get_process_only_active(),
            is_blacklist: e.get_process_blacklist(),
        });
        e.set_process_matchers(rows(&state.matchers));
        e.set_selected_process(state.matchers.len() as i32 - 1);
        drop(state);
        e.invoke_save();
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_update_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let name = e.get_process_name().trim().to_owned();
        if name.is_empty() {
            e.set_message(Msg::ProcessNameRequired.to_ui());
            return;
        }
        let mut state = shared.borrow_mut();
        let Some(item) = usize::try_from(e.get_selected_process())
            .ok()
            .and_then(|index| state.matchers.get_mut(index))
        else {
            return;
        };
        item.name = name;
        item.only_active_window = e.get_process_only_active();
        item.is_blacklist = e.get_process_blacklist();
        e.set_process_matchers(rows(&state.matchers));
        drop(state);
        e.invoke_save();
    });

    let weak = ui.as_weak();
    e.on_delete_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let mut state = state.borrow_mut();
        let Some(index) = usize::try_from(e.get_selected_process())
            .ok()
            .filter(|index| *index < state.matchers.len())
        else {
            return;
        };
        state.matchers.remove(index);
        e.set_process_matchers(rows(&state.matchers));
        e.set_selected_process(-1);
        e.set_process_name("".into());
        drop(state);
        e.invoke_save();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_devices() -> Vec<lhc_core::mapper_types::InputDevice> {
        [
            ("Power button", "/dev/input/event0", false, false),
            ("Mouse", "/dev/input/event1", false, true),
            ("Клавиатура", "/dev/input/event2", true, false),
        ]
        .into_iter()
        .map(
            |(name, path, is_keyboard, is_mouse)| lhc_core::mapper_types::InputDevice {
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
}
