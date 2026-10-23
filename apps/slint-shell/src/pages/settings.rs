use super::{APPEARANCES, LOCALES, choice, choice_index, condition_list, strings};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        CommandAssignmentRow, CommandPlaceRow, DeviceChoice, DeviceChoices, DeviceGroup,
        GameAppEntry, SettingsEditor, SettingsWindow,
    },
};
use lhc_core::profile::menus::PlaceKind;
use lhc_core::profile::model::{AppSettings, GameModeProcessMatcher, PasteShortcut, TextMode};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

const TAP_DECISIONS: [&str; 2] = ["permissiveHold", "holdOnOtherKeyPress"];

/// The form as the user sees it. Numbers stay text until saved.
#[derive(Clone, Debug, PartialEq)]
struct Form {
    launch_on_startup: bool,
    start_mapper_on_launch: bool,
    appearance: i32,
    high_contrast: bool,
    reduce_motion: bool,
    layout_osd: bool,
    layer_hint: bool,
    locale: i32,
    tap_decision: i32,
    hold: String,
    long_hold: String,
    double_tap: String,
    combo: String,
    macro_pause: String,
    modifier_delay: String,
    macro_timeout: String,
    commands_enabled: bool,
    use_gamemoded: bool,
    use_fullscreen: bool,
    use_steam: bool,
    use_lutris: bool,
    use_heroic: bool,
    use_umu: bool,
    use_gamescope: bool,
    use_app_category: bool,
    use_game_bar: bool,
    matchers: Vec<GameModeProcessMatcher>,
    text_mode: i32,
    paste_shortcut: i32,
    ydotool: String,
    xdotool: String,
    keyboard: String,
    mouse: String,
}

fn parse_ms(value: &str) -> Result<u64, Msg> {
    value.trim().parse().map_err(|_| Msg::SettingsTimingInvalid)
}

fn path(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

impl Form {
    fn from_settings(settings: &AppSettings) -> Self {
        Self {
            launch_on_startup: settings.launch_on_startup,
            start_mapper_on_launch: settings.start_mapper_on_launch,
            appearance: choice_index(&APPEARANCES, &settings.appearance),
            high_contrast: settings.high_contrast,
            reduce_motion: settings.reduce_motion,
            layout_osd: settings.layout_osd,
            layer_hint: settings.layer_hint,
            locale: choice_index(&LOCALES, &settings.locale),
            tap_decision: choice_index(&TAP_DECISIONS, &settings.tap_decision.as_str()),
            hold: settings.default_hold_timeout_ms.to_string(),
            long_hold: settings.default_long_hold_timeout_ms.to_string(),
            double_tap: settings.default_double_tap_timeout_ms.to_string(),
            combo: settings.combo_timeout_ms.to_string(),
            macro_pause: settings.default_macro_step_pause_ms.to_string(),
            modifier_delay: settings.default_macro_modifier_delay_ms.to_string(),
            macro_timeout: settings.macro_timeout_ms.to_string(),
            commands_enabled: settings.commands_enabled,
            use_gamemoded: settings.game_mode.use_gamemoded,
            use_fullscreen: settings.game_mode.use_fullscreen,
            use_steam: settings.game_mode.use_steam,
            use_lutris: settings.game_mode.use_lutris,
            use_heroic: settings.game_mode.use_heroic,
            use_umu: settings.game_mode.use_umu,
            use_gamescope: settings.game_mode.use_gamescope,
            use_app_category: settings.game_mode.use_app_category,
            use_game_bar: settings.game_mode.use_game_bar,
            matchers: settings.game_mode.process_matchers.clone(),
            text_mode: choice_index(&TextMode::ALL, &settings.linux_wayland_text_mode),
            paste_shortcut: choice_index(&PasteShortcut::ALL, &settings.linux_paste_shortcut),
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
            start_mapper_on_launch: e.get_start_mapper_on_launch(),
            appearance: e.get_appearance_index(),
            high_contrast: e.get_high_contrast(),
            reduce_motion: e.get_reduce_motion(),
            layout_osd: e.get_layout_osd(),
            layer_hint: e.get_layer_hint(),
            locale: e.get_locale_index(),
            tap_decision: e.get_tap_decision_index(),
            hold: e.get_hold_timeout().into(),
            long_hold: e.get_long_hold_timeout().into(),
            double_tap: e.get_double_tap_timeout().into(),
            combo: e.get_combo_timeout().into(),
            macro_pause: e.get_macro_pause().into(),
            modifier_delay: e.get_modifier_delay().into(),
            macro_timeout: e.get_macro_timeout().into(),
            commands_enabled: e.get_commands_enabled(),
            use_gamemoded: e.get_use_gamemoded(),
            use_fullscreen: e.get_use_fullscreen(),
            use_steam: e.get_use_steam(),
            use_lutris: e.get_use_lutris(),
            use_heroic: e.get_use_heroic(),
            use_umu: e.get_use_umu(),
            use_gamescope: e.get_use_gamescope(),
            use_app_category: e.get_use_app_category(),
            use_game_bar: e.get_use_game_bar(),
            matchers: matchers.to_vec(),
            text_mode: e.get_text_mode_index(),
            paste_shortcut: e.get_paste_shortcut_index(),
            ydotool: e.get_ydotool_path().into(),
            xdotool: e.get_xdotool_path().into(),
            keyboard: e.get_keyboard_device_path().into(),
            mouse: e.get_mouse_device_path().into(),
        }
    }

    fn show(&self, ui: &SettingsWindow) {
        let e = ui.global::<SettingsEditor>();
        e.set_launch_on_startup(self.launch_on_startup);
        e.set_start_mapper_on_launch(self.start_mapper_on_launch);
        e.set_appearance_index(self.appearance);
        e.set_high_contrast(self.high_contrast);
        e.set_reduce_motion(self.reduce_motion);
        e.set_layout_osd(self.layout_osd);
        e.set_layer_hint(self.layer_hint);
        e.set_locale_index(self.locale);
        e.set_tap_decision_index(self.tap_decision);
        e.set_hold_timeout(self.hold.clone().into());
        e.set_long_hold_timeout(self.long_hold.clone().into());
        e.set_double_tap_timeout(self.double_tap.clone().into());
        e.set_combo_timeout(self.combo.clone().into());
        e.set_macro_pause(self.macro_pause.clone().into());
        e.set_modifier_delay(self.modifier_delay.clone().into());
        e.set_macro_timeout(self.macro_timeout.clone().into());
        e.set_commands_enabled(self.commands_enabled);
        e.set_use_gamemoded(self.use_gamemoded);
        e.set_use_fullscreen(self.use_fullscreen);
        e.set_use_steam(self.use_steam);
        e.set_use_lutris(self.use_lutris);
        e.set_use_heroic(self.use_heroic);
        e.set_use_umu(self.use_umu);
        e.set_use_gamescope(self.use_gamescope);
        e.set_use_app_category(self.use_app_category);
        e.set_use_game_bar(self.use_game_bar);
        show_matchers(&e, &self.matchers);
        e.set_text_mode_index(self.text_mode);
        e.set_paste_shortcut_index(self.paste_shortcut);
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
            parse_ms(&self.combo)?,
            parse_ms(&self.macro_timeout)?,
        ];
        use lhc_core::profile::timing;
        for (value, range) in numbers.iter().zip([
            timing::TAP_HOLD,
            timing::TAP_HOLD,
            timing::MACRO_DELAY,
            timing::MACRO_DELAY,
            timing::COMBO,
            timing::MACRO_TIMEOUT,
        ]) {
            if !range.contains(*value) {
                return Err(Msg::SettingsTimingInvalid);
            }
        }
        macro_rules! changed {
            ($field:ident => $apply:expr) => {
                if self.$field != base.$field {
                    $apply;
                }
            };
        }
        changed!(launch_on_startup => settings.launch_on_startup = self.launch_on_startup);
        changed!(start_mapper_on_launch => settings.start_mapper_on_launch = self.start_mapper_on_launch);
        changed!(appearance => settings.appearance = choice(&APPEARANCES, self.appearance));
        changed!(high_contrast => settings.high_contrast = self.high_contrast);
        changed!(reduce_motion => settings.reduce_motion = self.reduce_motion);
        changed!(layout_osd => settings.layout_osd = self.layout_osd);
        changed!(layer_hint => settings.layer_hint = self.layer_hint);
        changed!(locale => settings.locale = choice(&LOCALES, self.locale));
        changed!(tap_decision => settings.tap_decision = choice(&TAP_DECISIONS, self.tap_decision).into());
        changed!(long_hold => settings.default_long_hold_timeout_ms = super::rules::parse_long_hold(&self.long_hold)?);
        changed!(hold => settings.default_hold_timeout_ms = numbers[0]);
        changed!(double_tap => settings.default_double_tap_timeout_ms = numbers[1]);
        changed!(macro_pause => settings.default_macro_step_pause_ms = numbers[2]);
        changed!(modifier_delay => settings.default_macro_modifier_delay_ms = numbers[3]);
        changed!(combo => settings.combo_timeout_ms = numbers[4]);
        changed!(macro_timeout => settings.macro_timeout_ms = numbers[5]);
        changed!(commands_enabled => settings.commands_enabled = self.commands_enabled);
        changed!(use_gamemoded => settings.game_mode.use_gamemoded = self.use_gamemoded);
        changed!(use_fullscreen => settings.game_mode.use_fullscreen = self.use_fullscreen);
        changed!(use_steam => settings.game_mode.use_steam = self.use_steam);
        changed!(use_lutris => settings.game_mode.use_lutris = self.use_lutris);
        changed!(use_heroic => settings.game_mode.use_heroic = self.use_heroic);
        changed!(use_umu => settings.game_mode.use_umu = self.use_umu);
        changed!(use_gamescope => settings.game_mode.use_gamescope = self.use_gamescope);
        changed!(use_app_category => settings.game_mode.use_app_category = self.use_app_category);
        changed!(use_game_bar => settings.game_mode.use_game_bar = self.use_game_bar);
        changed!(matchers => settings.game_mode.process_matchers = self.matchers.clone());
        changed!(text_mode => settings.linux_wayland_text_mode = choice(&TextMode::ALL, self.text_mode));
        changed!(paste_shortcut => settings.linux_paste_shortcut = choice(&PasteShortcut::ALL, self.paste_shortcut));
        changed!(ydotool => settings.linux_ydotool_path = self.ydotool.trim().into());
        changed!(xdotool => settings.linux_xdotool_path = self.xdotool.trim().into());
        changed!(keyboard => settings.input_device_path = path(&self.keyboard));
        changed!(mouse => settings.input_mouse_device_path = path(&self.mouse));
        Ok(())
    }
}

/// Show the games and the exclusions, and all names for the duplicate check.
fn show_matchers(e: &SettingsEditor, items: &[GameModeProcessMatcher]) {
    e.set_game_processes(entries(items, false));
    e.set_excluded_processes(entries(items, true));
    let names: Vec<String> = items.iter().map(|item| item.name.clone()).collect();
    e.set_process_value(condition_list(&names).into());
}

/// The matchers of one list, with their indices among all of them.
fn entries(items: &[GameModeProcessMatcher], blacklist: bool) -> ModelRc<GameAppEntry> {
    ModelRc::new(VecModel::from(
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_blacklist == blacklist)
            .map(|(index, item)| GameAppEntry {
                pattern: item.name.clone().into(),
                window_only: item.only_active_window,
                index: i32::try_from(index).unwrap_or(i32::MAX),
            })
            .collect::<Vec<_>>(),
    ))
}

/// Add (`index` -1) or change a matcher; a name another matcher has is
/// not added twice.
fn save_matcher(
    items: &mut Vec<GameModeProcessMatcher>,
    index: i32,
    name: &str,
    window_only: bool,
    blacklist: bool,
) -> bool {
    let name = name.trim();
    let index = usize::try_from(index).ok();
    let duplicate = items
        .iter()
        .enumerate()
        .any(|(at, item)| Some(at) != index && item.name.eq_ignore_ascii_case(name));
    if name.is_empty() || duplicate {
        return false;
    }
    let Some(index) = index else {
        items.push(GameModeProcessMatcher {
            id: lhc_core::profile::ids::generate("process-"),
            name: name.to_owned(),
            only_active_window: window_only,
            is_blacklist: blacklist,
        });
        return true;
    };
    let Some(item) = items.get_mut(index) else {
        return false;
    };
    item.name = name.to_owned();
    item.only_active_window = window_only;
    item.is_blacklist = blacklist;
    true
}

/// The form as last loaded from the document, and the draft process list.
#[derive(Default)]
struct State {
    base: Option<Form>,
    matchers: Vec<GameModeProcessMatcher>,
    autosave: slint::Timer,
}

/// Rules and automatic layout rules that depend on game mode.
fn game_usage(ui: &SettingsWindow, document: &Document) {
    let config = document.read();
    let rules = config
        .layout()
        .rules
        .iter()
        .filter(|rule| rule.is_enabled() && rule.condition_game_mode.is_some())
        .count();
    let auto = config
        .settings()
        .auto_rules
        .iter()
        .filter(|rule| rule.enabled && rule.conditions.game_mode.is_some())
        .count();
    let e = ui.global::<SettingsEditor>();
    e.set_game_rule_count(rules as i32);
    e.set_game_auto_rule_count(auto as i32);
}

/// Load the form unless the user has unsaved edits in it.
fn refresh(ui: &SettingsWindow, document: &Document, state: &mut State, force: bool) {
    game_usage(ui, document);
    // Listed even while commands are off, so they can be reviewed first.
    let assignments = document.read().command_assignments();
    match assignments {
        Ok(rows) => {
            let mut previous = None;
            let rows: Vec<_> = rows
                .into_iter()
                .map(|row| {
                    let first = previous.as_ref() != Some(&row.layout_id);
                    previous = Some(row.layout_id.clone());
                    CommandAssignmentRow {
                        layout_name: row
                            .layout_id
                            .as_deref()
                            .and_then(lhc_core::profile::model::user_layout_name)
                            .unwrap_or_default()
                            .into(),
                        first,
                        script: row.command.script().into(),
                        name: row.command.name.into(),
                        places: ModelRc::new(VecModel::from(
                            row.places
                                .into_iter()
                                .map(|place| CommandPlaceRow {
                                    kind: match place.kind {
                                        PlaceKind::RuleTap => 0,
                                        PlaceKind::RuleHold => 1,
                                        PlaceKind::RuleLongHold => 2,
                                        PlaceKind::RuleDoubleTap => 3,
                                        PlaceKind::LayerKey => 4,
                                        PlaceKind::Macro => 5,
                                        PlaceKind::QuickAction => 6,
                                    },
                                    layer: place.layer.into(),
                                    name: place.name.into(),
                                    enabled: place.enabled,
                                })
                                .collect::<Vec<_>>(),
                        )),
                    }
                })
                .collect();
            ui.global::<SettingsEditor>()
                .set_command_assignments(ModelRc::new(VecModel::from(rows)))
        }
        Err(error) => ui
            .global::<SettingsEditor>()
            .set_message(crate::notifications::report(ui, &Msg::from(&error))),
    }
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
    devices: &[lhc_core::mapper::devices::InputDevice],
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
                    device.name.clone()
                }
                .into(),
                path: device.path.clone().into(),
                group,
                source_index: choices.len() as i32,
            });
        }
    }
    // Windows has no device paths to enter: the mapper takes the session.
    if !cfg!(target_os = "windows") {
        choices.push(DeviceChoice {
            group: DeviceGroup::Manual,
            source_index: choices.len() as i32,
            ..Default::default()
        });
    }
    choices
}

fn device_selection(choices: &[DeviceChoice], path: &str, manual: bool) -> (i32, bool) {
    if !manual && let Some(index) = choices.iter().position(|choice| choice.path == path) {
        (index as i32, false)
    } else if choices
        .last()
        .is_some_and(|choice| choice.group == DeviceGroup::Manual)
    {
        (choices.len() as i32 - 1, true)
    } else {
        // Without manual entry an unknown path (e.g. from another OS) is unset.
        (0, false)
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
    let (devices, error) = match lhc_core::mapper::runtime::list_input_devices() {
        Ok(devices) => (devices, Msg::None),
        Err(error) => {
            log::warn!("input device discovery: {error}");
            (
                Vec::new(),
                if cfg!(target_os = "linux") {
                    Msg::InputDiscoveryFailed(error)
                } else {
                    Msg::MapperUnsupported
                },
            )
        }
    };
    e.set_device_discovery_error(error.to_ui());
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
    check_input_access(ui);
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

/// Show what keeps the mapper from using the input devices, if anything.
pub(crate) fn check_input_access(ui: &SettingsWindow) {
    let e = ui.global::<SettingsEditor>();
    let keyboard = e.get_keyboard_device_path();
    let keyboard = keyboard.trim();
    let issues: Vec<_> =
        lhc_core::platform::input_access_issues((!keyboard.is_empty()).then_some(keyboard))
            .into_iter()
            .map(|issue| Msg::InputAccess(issue).to_ui())
            .collect();
    e.set_input_issues(ModelRc::new(VecModel::from(issues)));
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let state = Rc::new(RefCell::new(State::default()));
    let e = ui.global::<SettingsEditor>();
    let choices = ui.global::<DeviceChoices>();
    choices.on_filter(|choices, query| filtered_devices(choices, &query));
    choices.on_offset(device_offset);
    e.set_text_modes(strings(TextMode::ALL.map(TextMode::as_str)));
    e.set_fullscreen_non_games(lhc_core::gamemode::FULLSCREEN_NON_GAMES.join(", ").into());
    e.set_game_launchers(lhc_core::gamemode::GAME_LAUNCHERS.join(", ").into());
    let defaults = Form::from_settings(&AppSettings::default());
    e.set_default_hold_timeout(defaults.hold.clone().into());
    e.set_default_long_hold_timeout(defaults.long_hold.clone().into());
    e.set_default_double_tap_timeout(defaults.double_tap.clone().into());
    e.set_default_combo_timeout(defaults.combo.clone().into());
    e.set_default_macro_pause(defaults.macro_pause.clone().into());
    e.set_default_modifier_delay(defaults.modifier_delay.clone().into());
    e.set_default_macro_timeout(defaults.macro_timeout.clone().into());
    {
        let config = document.read();
        e.set_settings_dir(config.paths().settings_dir().display().to_string().into());
        e.set_layouts_dir(config.paths().layouts_dir().display().to_string().into());
    }
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
            ui.global::<SettingsEditor>()
                .set_message(crate::notifications::report(&ui, &message));
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
    e.on_save_process(move |index, name, window_only, blacklist| {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let mut state = shared.borrow_mut();
        if !save_matcher(&mut state.matchers, index, &name, window_only, blacklist) {
            return;
        }
        show_matchers(&e, &state.matchers);
        drop(state);
        e.invoke_save();
    });

    let weak = ui.as_weak();
    let shared = state.clone();
    e.on_delete_process(move |index| {
        let Some(ui) = weak.upgrade() else { return };
        let e = ui.global::<SettingsEditor>();
        let mut state = shared.borrow_mut();
        let Some(index) = usize::try_from(index)
            .ok()
            .filter(|index| *index < state.matchers.len())
        else {
            return;
        };
        state.matchers.remove(index);
        show_matchers(&e, &state.matchers);
        drop(state);
        e.invoke_save();
    });
}

#[cfg(test)]
#[path = "settings/tests.rs"]
mod tests;
