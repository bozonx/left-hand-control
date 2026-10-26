//! Settings page: general options, appearance, timings and window matchers.

use super::{choice, choice_index, condition_list, strings};
use crate::{
    document::{Document, View},
    i18n::Msg,
    ui::{
        AppearanceChoice, CommandAssignmentRow, CommandPlace, CommandPlaceRow, DeviceChoice,
        DeviceChoices, DeviceGroup, DeviceMatch, GameAppEntry, SettingsEditor, SettingsWindow,
    },
};
use lhc_core::mapper::devices;
use lhc_core::platform::InputAccessIssue;
use lhc_core::profile::menus::PlaceKind;
use lhc_core::profile::model::{
    AppSettings, Appearance, GameModeProcessMatcher, LocalePreference, PasteShortcut, TapDecision,
    TextMode,
};
use lhc_core::profile::timing;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

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

/// Declares [`Form`] from one table. Each row names a field, its type,
/// the `SettingsEditor` getter and setter, how it is loaded from the
/// settings and how a changed value is stored back (`?` rejects it).
macro_rules! form {
    ($($field:ident: $ty:ty = $get:ident / $set:ident,
        load |$s:ident| $load:expr,
        store |$t:ident, $v:ident| $store:expr;)*) => {
        /// The form as the user sees it. Numbers stay text until saved.
        #[derive(Clone, Debug, PartialEq)]
        struct Form {
            $($field: $ty,)*
            matchers: Vec<GameModeProcessMatcher>,
        }

        impl Form {
            fn from_settings(settings: &AppSettings) -> Self {
                Self {
                    $($field: {
                        let $s = settings;
                        $load
                    },)*
                    matchers: settings.game_mode.process_matchers.clone(),
                }
            }

            fn read(ui: &SettingsWindow, matchers: &[GameModeProcessMatcher]) -> Self {
                let e = ui.global::<SettingsEditor>();
                Self {
                    $($field: e.$get().into(),)*
                    matchers: matchers.to_vec(),
                }
            }

            fn show(&self, ui: &SettingsWindow) {
                let e = ui.global::<SettingsEditor>();
                $(e.$set(self.$field.clone().into());)*
                show_matchers(&e, &self.matchers);
            }

            /// Write the fields that differ from `base` into `settings`.
            /// On an error `settings` may be partly changed and is dropped.
            fn apply(&self, base: &Form, settings: &mut AppSettings) -> Result<(), Msg> {
                $(if self.$field != base.$field {
                    let ($t, $v) = (&mut *settings, &self.$field);
                    $store;
                })*
                if self.matchers != base.matchers {
                    settings.game_mode.process_matchers = self.matchers.clone();
                }
                Ok(())
            }
        }
    };
}

/// Milliseconds typed in the form, within `range`.
fn ms(text: &str, range: lhc_core::profile::timing::Range) -> Result<u64, Msg> {
    parse_ms(text).and_then(|value| {
        range
            .contains(value)
            .then_some(value)
            .ok_or(Msg::SettingsTimingInvalid)
    })
}

form! {
    in_development: bool = get_in_development / set_in_development,
        load |s| s.in_development, store |t, v| t.in_development = *v;
    launch_on_startup: bool = get_launch_on_startup / set_launch_on_startup,
        load |s| s.launch_on_startup, store |t, v| t.launch_on_startup = *v;
    start_mapper_on_launch: bool = get_start_mapper_on_launch / set_start_mapper_on_launch,
        load |s| s.start_mapper_on_launch, store |t, v| t.start_mapper_on_launch = *v;
    appearance: AppearanceChoice = get_appearance / set_appearance,
        load |s| appearance_choice(s.appearance), store |t, v| t.appearance = appearance(*v);
    high_contrast: bool = get_high_contrast / set_high_contrast,
        load |s| s.high_contrast, store |t, v| t.high_contrast = *v;
    reduce_motion: bool = get_reduce_motion / set_reduce_motion,
        load |s| s.reduce_motion, store |t, v| t.reduce_motion = *v;
    layout_osd: bool = get_layout_osd / set_layout_osd,
        load |s| s.layout_osd, store |t, v| t.layout_osd = *v;
    layer_hint: bool = get_layer_hint / set_layer_hint,
        load |s| s.layer_hint, store |t, v| t.layer_hint = *v;
    locale: i32 = get_locale_index / set_locale_index,
        load |s| choice_index(&LocalePreference::ALL, &s.locale), store |t, v| t.locale = choice(&LocalePreference::ALL, *v);
    tap_decision: i32 = get_tap_decision_index / set_tap_decision_index,
        load |s| choice_index(&TAP_DECISIONS, &s.tap_decision),
        store |t, v| t.tap_decision = choice(&TAP_DECISIONS, *v);
    hold: String = get_hold_timeout / set_hold_timeout,
        load |s| s.default_hold_timeout_ms.to_string(),
        store |t, v| t.default_hold_timeout_ms = ms(v, timing::TAP_HOLD)?;
    long_hold: String = get_long_hold_timeout / set_long_hold_timeout,
        load |s| s.default_long_hold_timeout_ms.to_string(),
        store |t, v| t.default_long_hold_timeout_ms = super::rules::parse_long_hold(v)?;
    double_tap: String = get_double_tap_timeout / set_double_tap_timeout,
        load |s| s.default_double_tap_timeout_ms.to_string(),
        store |t, v| t.default_double_tap_timeout_ms = ms(v, timing::TAP_HOLD)?;
    combo: String = get_combo_timeout / set_combo_timeout,
        load |s| s.combo_timeout_ms.to_string(),
        store |t, v| t.combo_timeout_ms = ms(v, timing::COMBO)?;
    macro_pause: String = get_macro_pause / set_macro_pause,
        load |s| s.default_macro_step_pause_ms.to_string(),
        store |t, v| t.default_macro_step_pause_ms = ms(v, timing::MACRO_DELAY)?;
    modifier_delay: String = get_modifier_delay / set_modifier_delay,
        load |s| s.default_macro_modifier_delay_ms.to_string(),
        store |t, v| t.default_macro_modifier_delay_ms = ms(v, timing::MACRO_DELAY)?;
    macro_timeout: String = get_macro_timeout / set_macro_timeout,
        load |s| s.macro_timeout_ms.to_string(),
        store |t, v| t.macro_timeout_ms = ms(v, timing::MACRO_TIMEOUT)?;
    commands_enabled: bool = get_commands_enabled / set_commands_enabled,
        load |s| s.commands_enabled, store |t, v| t.commands_enabled = *v;
    use_gamemoded: bool = get_use_gamemoded / set_use_gamemoded,
        load |s| s.game_mode.use_gamemoded, store |t, v| t.game_mode.use_gamemoded = *v;
    use_fullscreen: bool = get_use_fullscreen / set_use_fullscreen,
        load |s| s.game_mode.use_fullscreen, store |t, v| t.game_mode.use_fullscreen = *v;
    use_steam: bool = get_use_steam / set_use_steam,
        load |s| s.game_mode.use_steam, store |t, v| t.game_mode.use_steam = *v;
    use_lutris: bool = get_use_lutris / set_use_lutris,
        load |s| s.game_mode.use_lutris, store |t, v| t.game_mode.use_lutris = *v;
    use_heroic: bool = get_use_heroic / set_use_heroic,
        load |s| s.game_mode.use_heroic, store |t, v| t.game_mode.use_heroic = *v;
    use_umu: bool = get_use_umu / set_use_umu,
        load |s| s.game_mode.use_umu, store |t, v| t.game_mode.use_umu = *v;
    use_gamescope: bool = get_use_gamescope / set_use_gamescope,
        load |s| s.game_mode.use_gamescope, store |t, v| t.game_mode.use_gamescope = *v;
    use_app_category: bool = get_use_app_category / set_use_app_category,
        load |s| s.game_mode.use_app_category, store |t, v| t.game_mode.use_app_category = *v;
    use_game_bar: bool = get_use_game_bar / set_use_game_bar,
        load |s| s.game_mode.use_game_bar, store |t, v| t.game_mode.use_game_bar = *v;
    text_mode: i32 = get_text_mode_index / set_text_mode_index,
        load |s| choice_index(&TextMode::ALL, &s.linux_wayland_text_mode),
        store |t, v| t.linux_wayland_text_mode = choice(&TextMode::ALL, *v);
    paste_shortcut: i32 = get_paste_shortcut_index / set_paste_shortcut_index,
        load |s| choice_index(&PasteShortcut::ALL, &s.linux_paste_shortcut),
        store |t, v| t.linux_paste_shortcut = choice(&PasteShortcut::ALL, *v);
    terminal_paste_shortcut: i32 = get_terminal_paste_shortcut_index / set_terminal_paste_shortcut_index,
        load |s| choice_index(&TERMINAL_PASTE_SHORTCUTS, &s.linux_terminal_paste_shortcut),
        store |t, v| t.linux_terminal_paste_shortcut = choice(&TERMINAL_PASTE_SHORTCUTS, *v);
    ydotool: String = get_ydotool_path / set_ydotool_path,
        load |s| s.linux_ydotool_path.clone(), store |t, v| t.linux_ydotool_path = v.trim().into();
    xdotool: String = get_xdotool_path / set_xdotool_path,
        load |s| s.linux_xdotool_path.clone(), store |t, v| t.linux_xdotool_path = v.trim().into();
    keyboard: String = get_keyboard_device_path / set_keyboard_device_path,
        load |s| s.input_device_path.clone().unwrap_or_default(),
        store |t, v| t.input_device_path = path(v);
    mouse: String = get_mouse_device_path / set_mouse_device_path,
        load |s| s.input_mouse_device_path.clone().unwrap_or_default(),
        store |t, v| t.input_mouse_device_path = path(v);
}

fn parse_ms(value: &str) -> Result<u64, Msg> {
    value.trim().parse().map_err(|_| Msg::SettingsTimingInvalid)
}

fn path(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
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
    autosave: super::autosave::Autosave,
}

impl super::autosave::Draft for State {
    fn autosave(&self) -> &super::autosave::Autosave {
        &self.autosave
    }

    fn save(&mut self, ui: &SettingsWindow, document: &Document) {
        self.autosave.cancel();
        let message = save(ui, document, self);
        ui.global::<SettingsEditor>()
            .set_message(crate::notifications::report(ui, &message));
    }
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
                                        PlaceKind::RuleTap => CommandPlace::RuleTap,
                                        PlaceKind::RuleHold => CommandPlace::RuleHold,
                                        PlaceKind::RuleLongHold => CommandPlace::RuleLongHold,
                                        PlaceKind::RuleDoubleTap => CommandPlace::RuleDoubleTap,
                                        PlaceKind::LayerKey => CommandPlace::LayerKey,
                                        PlaceKind::Macro => CommandPlace::Macro,
                                        PlaceKind::QuickAction => CommandPlace::QuickAction,
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
            label: devices::label("", saved).into(),
            detail: devices::short_path(saved).into(),
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
                label: devices::label(&device.name, &device.path).into(),
                detail: devices::short_path(&device.path).into(),
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
    if saved.is_ok() {
        state.base = None;
        refresh(ui, document, state, true);
    }
    super::outcome(&saved, Msg::None)
}

/// Show and return what keeps the mapper from using the input devices.
pub(crate) fn check_input_access(ui: &SettingsWindow) -> Vec<InputAccessIssue> {
    let e = ui.global::<SettingsEditor>();
    let keyboard = e.get_keyboard_device_path();
    let keyboard = keyboard.trim();
    let issues =
        lhc_core::platform::input_access_issues((!keyboard.is_empty()).then_some(keyboard));
    let messages: Vec<_> = issues
        .iter()
        .map(|issue| Msg::InputAccess(*issue).to_ui())
        .collect();
    e.set_input_issues(ModelRc::new(VecModel::from(messages)));
    issues
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

    super::autosave::subscribe(
        ui,
        document,
        View::Settings,
        &state,
        |ui, document, state| refresh(ui, document, state, false),
    );

    on!(ui, e.on_save, doc = document, shared = state => |ui| {
        super::autosave::Draft::save(&mut *shared.borrow_mut(), &ui, &doc);
    });

    on!(ui, e.on_schedule_save, doc = document, shared = state => |ui| {
        super::autosave::schedule(&ui, &doc, &shared);
    });

    on!(ui, e.on_refresh_devices => |ui| {
        refresh_devices(&ui);
    });

    on!(ui, e.on_select_device => |ui, path| {
        let e = ui.global::<SettingsEditor>();
        e.set_keyboard_device_path(path.trim().into());
        // Saving reloads the form, which rebuilds both device lists.
        e.invoke_save();
    });

    on!(ui, e.on_select_mouse => |ui, path| {
        let e = ui.global::<SettingsEditor>();
        e.set_mouse_device_path(path.trim().into());
        // Saving reloads the form, which rebuilds both device lists.
        e.invoke_save();
    });

    on!(ui, e.on_save_process, shared = state => |ui, index, name, window_only, blacklist| {
        let e = ui.global::<SettingsEditor>();
        let mut state = shared.borrow_mut();
        if !save_matcher(&mut state.matchers, index, &name, window_only, blacklist) {
            return;
        }
        show_matchers(&e, &state.matchers);
        drop(state);
        e.invoke_save();
    });

    on!(ui, e.on_delete_process, shared = state => |ui, index| {
        let e = ui.global::<SettingsEditor>();
        let mut state = shared.borrow_mut();
        let Some(index) = super::index(index, state.matchers.len())
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
