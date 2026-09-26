use crate::{
    i18n::{Language, Msg},
    ui::{ProcessRow, SettingsWindow},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        auto_switch::AutoSwitchContext,
        model::{Appearance, GameModeProcessMatcher, LocalePreference},
    },
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::{cell::RefCell, rc::Rc};

fn parse_ms(value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| "Enter a non-negative integer for each timeout".into())
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

pub(super) fn refresh_mouse_devices(ui: &SettingsWindow) {
    let mice = lhc_core::mapper::runtime::list_mice().unwrap_or_default();
    let mut labels = vec![SharedString::from("—")];
    let mut paths = vec![SharedString::new()];
    for mouse in mice {
        labels.push(format!("{} · {}", mouse.name, mouse.path).into());
        paths.push(mouse.path.into());
    }
    let saved = ui.get_mouse_device_path();
    if !saved.is_empty() && !paths.contains(&saved) {
        labels.push(saved.clone());
        paths.push(saved);
    }
    ui.set_mouse_devices(ModelRc::new(VecModel::from(labels)));
    let selected = paths
        .iter()
        .position(|path| path == &ui.get_mouse_device_path())
        .unwrap_or(0);
    ui.set_mouse_paths(ModelRc::new(VecModel::from(paths)));
    ui.set_selected_mouse(selected as i32);
}

pub(super) fn bind(ui: &SettingsWindow, config: Option<Rc<RefCell<ConfigDocument>>>) {
    let weak = ui.as_weak();
    ui.on_settings_saved(move |dark, english| {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_preferences(dark, english);
        }
    });
    let mut matchers = Vec::new();
    if let Some(config) = &config {
        let document = config.borrow();
        let settings = document.settings();
        ui.set_appearance_index(match settings.appearance {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        });
        ui.set_locale_index(match settings.locale {
            LocalePreference::Auto => 0,
            LocalePreference::English => 1,
            LocalePreference::Russian => 2,
        });
        ui.set_keyboard_device_path(
            settings
                .input_device_path
                .clone()
                .unwrap_or_default()
                .into(),
        );
        ui.set_tap_decision_index(i32::from(settings.tap_decision == "holdOnOtherKeyPress"));
        ui.set_hold_timeout(settings.default_hold_timeout_ms.to_string().into());
        ui.set_double_tap_timeout(settings.default_double_tap_timeout_ms.to_string().into());
        ui.set_macro_pause(settings.default_macro_step_pause_ms.to_string().into());
        ui.set_modifier_delay(settings.default_macro_modifier_delay_ms.to_string().into());
        ui.set_use_gamemoded(settings.game_mode.use_gamemoded);
        ui.set_use_fullscreen(settings.game_mode.use_fullscreen);
        ui.set_text_mode_index(
            [
                "libei",
                "libei-pure",
                "keycode",
                "clipboard",
                "ydotool",
                "xdotool",
            ]
            .iter()
            .position(|mode| settings.linux_wayland_text_mode.as_deref() == Some(*mode))
            .unwrap_or(0) as i32,
        );
        ui.set_ydotool_path(settings.linux_ydotool_path.clone().into());
        ui.set_xdotool_path(settings.linux_xdotool_path.clone().into());
        ui.set_mouse_device_path(
            settings
                .input_mouse_device_path
                .clone()
                .unwrap_or_default()
                .into(),
        );
        ui.set_settings_dir(document.paths().settings_dir().display().to_string().into());
        ui.set_layouts_dir(document.paths().layouts_dir().display().to_string().into());
        matchers = settings.game_mode.process_matchers.clone();
    }
    refresh_mouse_devices(ui);
    ui.set_is_linux(cfg!(target_os = "linux"));
    let platform = lhc_core::platform::info();
    let detail = platform.linux.map_or_else(
        || platform.os.to_owned(),
        |linux| {
            format!(
                "{} · {} · {}",
                platform.os, linux.desktop, linux.session_type
            )
        },
    );
    ui.set_platform_summary(detail.into());
    let matchers = Rc::new(RefCell::new(matchers));
    ui.set_process_matchers(rows(&matchers.borrow()));
    let weak = ui.as_weak();
    let list = matchers.clone();
    ui.on_select_process(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let Ok(index_usize) = usize::try_from(index) else {
            return;
        };
        let Some(item) = list.borrow().get(index_usize).cloned() else {
            return;
        };
        ui.set_selected_process(index);
        ui.set_process_name(item.name.into());
        ui.set_process_only_active(item.only_active_window);
        ui.set_process_blacklist(item.is_blacklist);
    });
    let weak = ui.as_weak();
    let list = matchers.clone();
    ui.on_add_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let name = ui.get_process_name().trim().to_owned();
        if name.is_empty() {
            ui.set_settings_message(Msg::ProcessNameRequired.to_ui());
            return;
        }
        let mut items = list.borrow_mut();
        let id = format!(
            "process-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |time| time.as_nanos())
        );
        items.push(GameModeProcessMatcher {
            id,
            name,
            only_active_window: ui.get_process_only_active(),
            is_blacklist: ui.get_process_blacklist(),
        });
        ui.set_process_matchers(rows(&items));
        ui.set_selected_process((items.len() - 1) as i32);
        ui.set_settings_message(Msg::None.to_ui());
    });
    let weak = ui.as_weak();
    let list = matchers.clone();
    ui.on_update_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let name = ui.get_process_name().trim().to_owned();
        if name.is_empty() {
            ui.set_settings_message(Msg::ProcessNameRequired.to_ui());
            return;
        }
        let Ok(index) = usize::try_from(ui.get_selected_process()) else {
            return;
        };
        let mut items = list.borrow_mut();
        let Some(item) = items.get_mut(index) else {
            return;
        };
        item.name = name;
        item.only_active_window = ui.get_process_only_active();
        item.is_blacklist = ui.get_process_blacklist();
        ui.set_process_matchers(rows(&items));
    });
    let weak = ui.as_weak();
    let list = matchers.clone();
    ui.on_delete_process(move || {
        let Some(ui) = weak.upgrade() else { return };
        let Ok(index) = usize::try_from(ui.get_selected_process()) else {
            return;
        };
        let mut items = list.borrow_mut();
        if index >= items.len() {
            return;
        }
        items.remove(index);
        ui.set_process_matchers(rows(&items));
        ui.set_selected_process(-1);
        ui.set_process_name("".into());
    });
    let weak = ui.as_weak();
    ui.on_save_settings(move || {
        let (Some(ui), Some(config)) = (weak.upgrade(), &config) else {
            return;
        };
        let mut saved = false;
        let result = (|| {
            let hold = parse_ms(&ui.get_hold_timeout())?;
            let double_tap = parse_ms(&ui.get_double_tap_timeout())?;
            let pause = parse_ms(&ui.get_macro_pause())?;
            let modifier = parse_ms(&ui.get_modifier_delay())?;
            let modes = [
                "libei",
                "libei-pure",
                "keycode",
                "clipboard",
                "ydotool",
                "xdotool",
            ];
            let mode = modes
                .get(ui.get_text_mode_index() as usize)
                .ok_or("Invalid text mode")?;
            config
                .borrow_mut()
                .update_settings(|settings| {
                    settings.appearance = match ui.get_appearance_index() {
                        1 => Appearance::Light,
                        2 => Appearance::Dark,
                        _ => Appearance::System,
                    };
                    settings.locale = match ui.get_locale_index() {
                        1 => LocalePreference::English,
                        2 => LocalePreference::Russian,
                        _ => LocalePreference::Auto,
                    };
                    settings.input_device_path = Some(ui.get_keyboard_device_path().trim().into());
                    settings.tap_decision = if ui.get_tap_decision_index() == 1 {
                        "holdOnOtherKeyPress"
                    } else {
                        "permissiveHold"
                    }
                    .into();
                    settings.default_hold_timeout_ms = hold;
                    settings.default_double_tap_timeout_ms = double_tap;
                    settings.default_macro_step_pause_ms = pause;
                    settings.default_macro_modifier_delay_ms = modifier;
                    settings.game_mode.use_gamemoded = ui.get_use_gamemoded();
                    settings.game_mode.use_fullscreen = ui.get_use_fullscreen();
                    settings.game_mode.process_matchers = matchers.borrow().clone();
                    settings.linux_wayland_text_mode = Some((*mode).into());
                    settings.linux_ydotool_path = ui.get_ydotool_path().into();
                    settings.linux_xdotool_path = ui.get_xdotool_path().into();
                    settings.input_mouse_device_path = Some(ui.get_mouse_device_path().into());
                })
                .map_err(|error| error.to_string())?;
            saved = true;
            let runtime = config
                .borrow()
                .runtime_config(&AutoSwitchContext::current())
                .map_err(|error| error.to_string())?;
            lhc_core::mapper::runtime::update_config_if_running(&runtime.json)?;
            Ok::<_, String>(())
        })();
        if saved {
            let dark = match ui.get_appearance_index() {
                1 => false,
                2 => true,
                _ => system_dark(&ui),
            };
            let english = match ui.get_locale_index() {
                1 => true,
                2 => false,
                _ => Language::resolve(LocalePreference::Auto) == Language::English,
            };
            ui.invoke_settings_saved(dark, english);
        }
        ui.set_settings_message(match result {
            Ok(()) => Msg::SettingsSaved.to_ui(),
            Err(error) if error.starts_with("Enter a non-negative") => Msg::TimeoutInvalid.to_ui(),
            Err(error) if saved => Msg::SavedMapperNotUpdated(error).to_ui(),
            Err(error) => Msg::Error(error).to_ui(),
        });
    });
}

pub(super) fn system_dark(ui: &SettingsWindow) -> bool {
    use slint::winit_030::WinitWindowAccessor;
    ui.window()
        .with_winit_window(|window| {
            window.theme() != Some(slint::winit_030::winit::window::Theme::Light)
        })
        .unwrap_or(true)
}
