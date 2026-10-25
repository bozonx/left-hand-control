use super::{LOCALES, choice, choice_index, condition_list, strings};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        AppearanceChoice, CommandAssignmentRow, CommandPlaceRow, DeviceChoice, DeviceChoices,
        DeviceGroup, DeviceMatch, GameAppEntry, SettingsEditor, SettingsWindow,
    },
};
use lhc_core::profile::menus::PlaceKind;
use lhc_core::profile::model::{
    AppSettings, Appearance, GameModeProcessMatcher, PasteShortcut, TapDecision, TextMode,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Delay that batches quick setting changes into one save.
const AUTOSAVE_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

/// Order of the tap decision choices in the settings page.
const TAP_DECISIONS: [TapDecision; 2] = [
    TapDecision::PermissiveHold,
    TapDecision::HoldOnOtherKeyPress,
];

/// Order of the terminal paste shortcut choices; `None` follows the main one.
const TERMINAL_PASTE_SHORTCUTS: [Option<PasteShortcut>; 4] = [
    None,
    Some(PasteShortcut::ShiftInsert),
    Some(PasteShortcut::CtrlV),
    Some(PasteShortcut::CtrlShiftV),
];

fn appearance_choice(appearance: Appearance) -> AppearanceChoice {
    match appearance {
        Appearance::System => AppearanceChoice::System,
        Appearance::Light => AppearanceChoice::Light,
        Appearance::Dark => AppearanceChoice::Dark,
        Appearance::EInk => AppearanceChoice::EInk,
    }
}

fn appearance(choice: AppearanceChoice) -> Appearance {
    match choice {
        AppearanceChoice::System => Appearance::System,
        AppearanceChoice::Light => Appearance::Light,
        AppearanceChoice::Dark => Appearance::Dark,
        AppearanceChoice::EInk => Appearance::EInk,
    }
}

/// The form as the user sees it. Numbers stay text until saved.
#[derive(Clone, Debug, PartialEq)]
struct Form {
    in_development: bool,
    launch_on_startup: bool,
    start_mapper_on_launch: bool,
    appearance: AppearanceChoice,
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
    terminal_paste_shortcut: i32,
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
            in_development: settings.in_development,
            launch_on_startup: settings.launch_on_startup,
            start_mapper_on_launch: settings.start_mapper_on_launch,
            appearance: appearance_choice(settings.appearance),
            high_contrast: settings.high_contrast,
            reduce_motion: settings.reduce_motion,
            layout_osd: settings.layout_osd,
            layer_hint: settings.layer_hint,
            locale: choice_index(&LOCALES, &settings.locale),
            tap_decision: choice_index(&TAP_DECISIONS, &settings.tap_decision),
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
            terminal_paste_shortcut: choice_index(
                &TERMINAL_PASTE_SHORTCUTS,
                &settings.linux_terminal_paste_shortcut,
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
            in_development: e.get_in_development(),
            launch_on_startup: e.get_launch_on_startup(),
            start_mapper_on_launch: e.get_start_mapper_on_launch(),
            appearance: e.get_appearance(),
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
            terminal_paste_shortcut: e.get_terminal_paste_shortcut_index(),
            ydotool: e.get_ydotool_path().into(),
            xdotool: e.get_xdotool_path().into(),
            keyboard: e.get_keyboard_device_path().into(),
            mouse: e.get_mouse_device_path().into(),
        }
    }

    fn show(&self, ui: &SettingsWindow) {
        let e = ui.global::<SettingsEditor>();
        e.set_in_development(self.in_development);
        e.set_launch_on_startup(self.launch_on_startup);
        e.set_start_mapper_on_launch(self.start_mapper_on_launch);
        e.set_appearance(self.appearance);
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
        e.set_terminal_paste_shortcut_index(self.terminal_paste_shortcut);
        e.set_ydotool_path(self.ydotool.clone().into());
        e.set_xdotool_path(self.xdotool.clone().into());
        e.set_keyboard_device_path(self.keyboard.clone().into());
        e.set_mouse_device_path(self.mouse.clone().into());
    }

    /// Write the fields that differ from `base` into `settings`.
    fn apply(&self, base: &Form, settings: &mut AppSettings) -> Result<(), Msg> {
        use lhc_core::profile::timing;
        let ms = |text: &str, range: timing::Range| {
            parse_ms(text).and_then(|value| {
                range
                    .contains(value)
                    .then_some(value)
                    .ok_or(Msg::SettingsTimingInvalid)
            })
        };
        // Validate every number before changing any field.
        let hold = ms(&self.hold, timing::TAP_HOLD)?;
        let double_tap = ms(&self.double_tap, timing::TAP_HOLD)?;
        let macro_pause = ms(&self.macro_pause, timing::MACRO_DELAY)?;
        let modifier_delay = ms(&self.modifier_delay, timing::MACRO_DELAY)?;
        let combo = ms(&self.combo, timing::COMBO)?;
        let macro_timeout = ms(&self.macro_timeout, timing::MACRO_TIMEOUT)?;
        macro_rules! changed {
            ($field:ident => $apply:expr) => {
                if self.$field != base.$field {
                    $apply;
                }
            };
        }
        changed!(in_development => settings.in_development = self.in_development);
        changed!(launch_on_startup => settings.launch_on_startup = self.launch_on_startup);
        changed!(start_mapper_on_launch => settings.start_mapper_on_launch = self.start_mapper_on_launch);
        changed!(appearance => settings.appearance = appearance(self.appearance));
        changed!(high_contrast => settings.high_contrast = self.high_contrast);
        changed!(reduce_motion => settings.reduce_motion = self.reduce_motion);
        changed!(layout_osd => settings.layout_osd = self.layout_osd);
        changed!(layer_hint => settings.layer_hint = self.layer_hint);
        changed!(locale => settings.locale = choice(&LOCALES, self.locale));
        changed!(tap_decision => settings.tap_decision = choice(&TAP_DECISIONS, self.tap_decision));
        changed!(long_hold => settings.default_long_hold_timeout_ms = super::rules::parse_long_hold(&self.long_hold)?);
        changed!(hold => settings.default_hold_timeout_ms = hold);
        changed!(double_tap => settings.default_double_tap_timeout_ms = double_tap);
        changed!(macro_pause => settings.default_macro_step_pause_ms = macro_pause);
        changed!(modifier_delay => settings.default_macro_modifier_delay_ms = modifier_delay);
        changed!(combo => settings.combo_timeout_ms = combo);
        changed!(macro_timeout => settings.macro_timeout_ms = macro_timeout);
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
        changed!(terminal_paste_shortcut => settings.linux_terminal_paste_shortcut = choice(&TERMINAL_PASTE_SHORTCUTS, self.terminal_paste_shortcut));
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

/// Picker rows for `devices`; a `saved` path that is not connected is kept
/// as its own row so the choice is not lost.
fn device_choices(
    devices: &[lhc_core::mapper::devices::InputDevice],
    mouse: bool,
    saved: &str,
) -> Vec<DeviceChoice> {
    let mut choices = Vec::new();
    let saved = saved.trim();
    if !saved.is_empty() && !devices.iter().any(|device| device.path == saved) {
        choices.push(DeviceChoice {
            label: device_label("", saved).into(),
            detail: short_path(saved).into(),
            path: saved.into(),
            group: DeviceGroup::Missing,
            ..Default::default()
        });
    }
    // Virtual devices (ydotoold, other remappers) also pass the keyboard
    // heuristic, but are never the keyboard the user types on.
    let group_of = |device: &lhc_core::mapper::devices::InputDevice| {
        if device.is_virtual {
            DeviceGroup::VirtualDevice
        } else if if mouse {
            device.is_mouse
        } else {
            device.is_keyboard
        } {
            DeviceGroup::Suggested
        } else {
            DeviceGroup::Other
        }
    };
    for group in [
        DeviceGroup::Suggested,
        DeviceGroup::Other,
        DeviceGroup::VirtualDevice,
    ] {
        for device in devices.iter().filter(|device| group_of(device) == group) {
            choices.push(DeviceChoice {
                label: device_label(&device.name, &device.path).into(),
                detail: short_path(&device.path).into(),
                path: device.path.clone().into(),
                group,
                ..Default::default()
            });
        }
    }
    // The suggested tab hides paths, so rows sharing a name keep theirs.
    let shown = |choice: &DeviceChoice| {
        matches!(choice.group, DeviceGroup::Missing | DeviceGroup::Suggested)
    };
    let labels: Vec<_> = choices
        .iter()
        .filter(|choice| shown(choice))
        .map(|choice| choice.label.clone())
        .collect();
    for choice in choices.iter_mut().filter(|choice| shown(choice)) {
        choice.ambiguous = labels
            .iter()
            .filter(|label| **label == choice.label)
            .count()
            > 1;
    }
    choices
}

const INPUT_PREFIX: &str = "/dev/input/";
/// Longest path shown in a row; longer ones keep their distinctive end.
const MAX_DETAIL_CHARS: usize = 60;

/// Device name, or one read from its `by-id` link when the kernel gives none.
fn device_label(name: &str, path: &str) -> String {
    let name = name.trim();
    if !name.is_empty() && name != "(unknown)" {
        return name.to_owned();
    }
    path.strip_prefix("/dev/input/by-id/")
        .map(|link| {
            let link = strip_event_suffix(link);
            let link = ["usb-", "bluetooth-", "i2c-", "serio-", "platform-"]
                .iter()
                .find_map(|bus| link.strip_prefix(bus))
                .unwrap_or(link);
            let link = link.split(':').next().unwrap_or(link);
            let link = match link.rsplit_once("-if") {
                Some((head, tail))
                    if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) =>
                {
                    head
                }
                _ => link,
            };
            link.replace('_', " ").trim().to_owned()
        })
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| short_path(path))
}

/// Path without the shared `/dev/input/` prefix and udev's `-event-*`
/// suffix, cut from the left when still too long.
fn short_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix(INPUT_PREFIX) else {
        return path.to_owned();
    };
    let rest = strip_event_suffix(rest);
    let rest = match rest.split_once('/') {
        Some((dir, link)) => format!("{dir} · {link}"),
        None => rest.to_owned(),
    };
    let count = rest.chars().count();
    if count <= MAX_DETAIL_CHARS {
        return rest;
    }
    let tail: String = rest.chars().skip(count - (MAX_DETAIL_CHARS - 1)).collect();
    format!("…{tail}")
}

fn strip_event_suffix(link: &str) -> &str {
    match link.rfind("-event") {
        Some(at) if !link[at + 6..].contains(['/', '.']) && at > 0 => &link[..at],
        _ => link,
    }
}

/// Rows matching `query`: the suggested devices and a missing saved one,
/// or every device when `all`.
fn filtered_devices(
    choices: ModelRc<DeviceChoice>,
    query: &str,
    all: bool,
) -> ModelRc<DeviceChoice> {
    let query = query.trim().to_lowercase();
    ModelRc::new(VecModel::from(
        choices
            .iter()
            .filter(|choice| {
                (all || matches!(choice.group, DeviceGroup::Missing | DeviceGroup::Suggested))
                    && (choice.label.to_lowercase().contains(&query)
                        || choice.path.to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>(),
    ))
}

/// Row for the device a typed `path` opens: the same path, or another
/// link to the same device.
fn find_device(choices: ModelRc<DeviceChoice>, path: &str) -> DeviceMatch {
    let path = path.trim();
    let position = |matches: &dyn Fn(&str) -> bool| {
        choices
            .iter()
            .position(|choice| matches(&choice.path))
            .map(|index| index as i32)
    };
    if path.is_empty() {
        return DeviceMatch {
            index: -1,
            exact: false,
        };
    }
    if let Some(index) = position(&|choice| choice == path) {
        return DeviceMatch { index, exact: true };
    }
    let index = position(&|choice| lhc_core::mapper::devices::same_device(choice, path));
    DeviceMatch {
        index: index.unwrap_or(-1),
        exact: false,
    }
}

/// Whether rows of `group` sit under a heading.
fn has_heading(group: DeviceGroup) -> bool {
    group == DeviceGroup::VirtualDevice
}

/// Offset of row `index`, given the row and group heading heights in pixels.
fn device_offset(choices: ModelRc<DeviceChoice>, index: i32, row: i32, heading: i32) -> i32 {
    let mut group = DeviceGroup::Missing;
    choices
        .iter()
        .take(index.max(0) as usize)
        .map(|choice| {
            let starts_group = has_heading(choice.group) && choice.group != group;
            group = choice.group;
            row + if starts_group { heading } else { 0 }
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
    let keyboards = device_choices(&devices, false, &e.get_keyboard_device_path());
    e.set_input_devices(ModelRc::new(VecModel::from(keyboards)));
    let mice = device_choices(&devices, true, &e.get_mouse_device_path());
    e.set_mouse_devices(ModelRc::new(VecModel::from(mice)));
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
    choices.on_filter(|choices, query, all| filtered_devices(choices, &query, all));
    let row = choices.get_row_height() as i32;
    let heading = choices.get_heading_height() as i32;
    choices.on_offset(move |items, index| device_offset(items, index, row, heading));
    choices.on_position(|items, path| {
        items
            .iter()
            .position(|choice| choice.path == path)
            .map_or(-1, |index| index as i32)
    });
    choices.on_find_device(|items, path| find_device(items, &path));
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
        shared
            .borrow()
            .autosave
            .start(slint::TimerMode::SingleShot, AUTOSAVE_DELAY, move || {
                if let Some(ui) = weak.upgrade() {
                    ui.global::<SettingsEditor>().invoke_save();
                }
            });
    });

    let weak = ui.as_weak();
    e.on_refresh_devices(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_devices(&ui);
        }
    });

    let weak = ui.as_weak();
    e.on_select_device(move |path| {
        if let Some(ui) = weak.upgrade() {
            let e = ui.global::<SettingsEditor>();
            e.set_keyboard_device_path(path.trim().into());
            // Saving reloads the form, which rebuilds both device lists.
            e.invoke_save();
        }
    });

    let weak = ui.as_weak();
    e.on_select_mouse(move |path| {
        if let Some(ui) = weak.upgrade() {
            let e = ui.global::<SettingsEditor>();
            e.set_mouse_device_path(path.trim().into());
            // Saving reloads the form, which rebuilds both device lists.
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
