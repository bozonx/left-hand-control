//! Settings page. The form is a draft until "Save settings"; saving writes
//! only the fields the user changed, so changes another process or page
//! made to other fields meanwhile are kept.

use super::{APPEARANCES, LOCALES, choice, choice_index, strings};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{CapabilityRow, ProcessRow, SettingsEditor, SettingsWindow},
};
use lhc_core::profile::model::{AppSettings, GameModeProcessMatcher};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
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
                &settings.linux_wayland_text_mode.as_deref().unwrap_or(TEXT_MODES[0]),
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
    /// Device paths in picker order.
    devices: Vec<String>,
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
    refresh_devices(ui, document, state);
}

/// Fill the keyboard and mouse pickers; the saved devices stay listed even
/// when they are not readable right now.
fn refresh_devices(ui: &SettingsWindow, document: &Document, state: &mut State) {
    let e = ui.global::<SettingsEditor>();
    let mut devices = lhc_core::mapper::runtime::list_keyboards().unwrap_or_else(|error| {
        log::warn!("keyboard discovery: {error}");
        Vec::new()
    });
    let saved = document.read().input_device().map(str::to_owned);
    if let Some(path) = &saved
        && !devices.iter().any(|device| &device.path == path)
    {
        devices.insert(
            0,
            lhc_core::mapper_types::KeyboardDevice {
                path: path.clone(),
                name: String::new(),
            },
        );
    }
    let selected = saved
        .and_then(|path| devices.iter().position(|device| device.path == path))
        .map_or(-1, |index| index as i32);
    e.set_input_devices(strings(devices.iter().map(|device| {
        if device.name.is_empty() {
            device.path.clone()
        } else {
            format!("{} · {}", device.name, device.path)
        }
    })));
    e.set_selected_device(selected);
    state.devices = devices.into_iter().map(|device| device.path).collect();

    let mice = lhc_core::mapper::runtime::list_mice().unwrap_or_else(|error| {
        log::warn!("mouse discovery: {error}");
        Vec::new()
    });
    let mut labels = vec![SharedString::from("—")];
    let mut paths = vec![SharedString::new()];
    for mouse in mice {
        labels.push(format!("{} · {}", mouse.name, mouse.path).into());
        paths.push(mouse.path.into());
    }
    let current = e.get_mouse_device_path();
    if !current.is_empty() && !paths.contains(&current) {
        labels.push(current.clone());
        paths.push(current.clone());
    }
    let selected = paths.iter().position(|path| *path == current).unwrap_or(0);
    e.set_mouse_devices(ModelRc::new(VecModel::from(labels)));
    e.set_mouse_paths(ModelRc::new(VecModel::from(paths)));
    e.set_selected_mouse(selected as i32);
}

fn save(ui: &SettingsWindow, document: &Document, state: &mut State) -> Msg {
    let Some(base) = state.base.clone() else {
        return Msg::LoadConfigFirst;
    };
    let form = Form::read(ui, &state.matchers);
    let mut result = Ok(());
    let saved = document.edit(View::Settings, |config| {
        config.update_settings(|settings| result = form.apply(&base, settings))
    });
    if let Err(error) = result {
        return error;
    }
    match saved {
        Ok(saved) => {
            state.base = None;
            refresh(ui, document, state, true);
            saved.message(Msg::SettingsSaved)
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
            let message = save(&ui, &doc, &mut shared.borrow_mut());
            ui.global::<SettingsEditor>().set_message(message.to_ui());
        }
    });

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_refresh_devices(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_devices(&ui, &doc, &mut shared.borrow_mut());
        }
    });

    let weak = ui.as_weak();
    let (doc, shared) = (document.clone(), state.clone());
    e.on_select_device(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|index| shared.borrow().devices.get(index).cloned())
        else {
            return;
        };
        let e = ui.global::<SettingsEditor>();
        e.set_keyboard_device_path(path.clone().into());
        let message = match doc.edit(View::Settings, |config| config.set_input_device(&path)) {
            Ok(saved) => {
                if let Some(base) = shared.borrow_mut().base.as_mut() {
                    base.keyboard = path.clone();
                }
                saved.message(Msg::DeviceSaved(path))
            }
            Err(error) => Msg::from(&error),
        };
        e.set_message(message.to_ui());
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
        e.set_message(Msg::None.to_ui());
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(settings.input_device_path.as_deref(), Some("/dev/input/event3"));
        assert_eq!(settings.input_mouse_device_path, None);
    }
}
