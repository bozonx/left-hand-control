//! Action picker: one dialog that chooses a key, shortcut or action for
//! every page and hands the value to the page that opened it.

use super::{keys, layers, rules};
use crate::{
    document::Document,
    i18n::Msg,
    ui::{
        ActionPicker, LayersEditor, Locale, MacroEditor, MenuEditor, PickerBehavior,
        PickerCategory, PickerCategoryCount, PickerItem, PickerTarget, PickerTitle, SettingsWindow,
        ShortcutDesktop,
    },
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions::{self, Action, ActionName},
        key_catalog::CATEGORIES,
    },
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

/// Picker category of each group of [`CATEGORIES`], in the same order.
const KEY_CATEGORIES: [PickerCategory; CATEGORIES.len()] = [
    PickerCategory::Letters,
    PickerCategory::Modifiers,
    PickerCategory::ControlKeys,
    PickerCategory::Navigation,
    PickerCategory::Media,
    PickerCategory::SystemKeys,
    PickerCategory::Mouse,
    PickerCategory::OtherKeys,
];

/// Sidebar order of the non-key categories. Commands go last: one
/// command per action, not a catalog.
const ACTION_CATEGORIES: [PickerCategory; 8] = [
    PickerCategory::Macros,
    PickerCategory::BuiltinMacros,
    PickerCategory::App,
    PickerCategory::SystemActions,
    PickerCategory::DesktopShortcuts,
    PickerCategory::Text,
    PickerCategory::Pause,
    PickerCategory::Commands,
];

fn is_key_category(category: PickerCategory) -> bool {
    KEY_CATEGORIES.contains(&category)
}

/// Whether `value` is the `cmd:` action of the command `id`.
fn assigns_command(value: &str, id: &str) -> bool {
    matches!(Action::parse(Some(value)), Action::Command(command) if command == id)
}

pub(crate) fn category_for_value(value: &str) -> PickerCategory {
    match Action::parse(Some(value)) {
        Action::Macro(_) => return PickerCategory::Macros,
        Action::BuiltinMacro(_) => return PickerCategory::BuiltinMacros,
        Action::Command(_) => return PickerCategory::Commands,
        Action::App(_) => return PickerCategory::App,
        Action::System(_) => return PickerCategory::SystemActions,
        Action::KdeShortcut(_) => return PickerCategory::DesktopShortcuts,
        Action::Text(_) => return PickerCategory::Text,
        Action::Pause(_) => return PickerCategory::Pause,
        Action::Native | Action::Swallow | Action::Keys(_) | Action::Unknown(_) => {}
    }
    // The last key of a chord is the main one; modifiers come first.
    for key in value.split('+').rev() {
        if let Some(index) = CATEGORIES.iter().position(|keys| keys.contains(&key)) {
            return KEY_CATEGORIES[index];
        }
    }
    PickerCategory::OtherKeys
}

/// Longest pause a macro step may wait, in milliseconds.
pub const MAX_PAUSE_MS: u32 = 10_000;

/// Whether `ms` is a valid macro pause.
pub fn valid_pause(ms: &str) -> bool {
    ms.trim()
        .parse::<u32>()
        .is_ok_and(|n| (1..=MAX_PAUSE_MS).contains(&n))
}

/// Display name of a catalog action in the current UI language.
pub(crate) fn action_label(ui: &SettingsWindow, name: ActionName) -> Option<String> {
    match name {
        ActionName::Verbatim(name) if !name.is_empty() => Some(name),
        ActionName::BuiltinMacro { id } => Some(
            ui.global::<Locale>()
                .invoke_text(Msg::BuiltinMacro(id.into()).to_ui())
                .to_string(),
        ),
        ActionName::System { id, n } | ActionName::App { id, n } => Some(
            ui.global::<Locale>()
                .invoke_text(Msg::PickerAction(id.into(), n).to_ui())
                .to_string(),
        ),
        _ => None,
    }
}

/// A native global shortcut of the desktop environment as the picker and
/// key caps show it.
pub(crate) struct DesktopShortcutItem {
    /// Backend-prefixed id, e.g. `kde:component/shortcut`.
    pub value: String,
    pub component: String,
    pub label: String,
}

thread_local! {
    /// Global shortcuts of the running desktop session; empty until read
    /// and on desktops without a shortcut backend.
    pub(crate) static DESKTOP_SHORTCUTS: RefCell<Vec<DesktopShortcutItem>> = const { RefCell::new(Vec::new()) };
}

/// Desktop environment whose native shortcuts this session can list.
fn shortcut_desktop() -> ShortcutDesktop {
    #[cfg(target_os = "linux")]
    {
        use lhc_core::platform::linux;
        if matches!(linux::detect().desktop, linux::Desktop::Kde) {
            return ShortcutDesktop::Kde;
        }
    }
    ShortcutDesktop::None
}

/// Re-read the desktop's global shortcuts in the background and refresh
/// an open picker when they arrive.
fn load_desktop_shortcuts(ui: slint::Weak<SettingsWindow>) {
    let read: fn() -> Result<Vec<DesktopShortcutItem>, String> = match shortcut_desktop() {
        ShortcutDesktop::None => return,
        ShortcutDesktop::Kde => read_kde_shortcuts,
    };
    std::thread::spawn(move || {
        let shortcuts = match read() {
            Ok(shortcuts) => shortcuts,
            Err(error) => {
                log::warn!("[picker] cannot read desktop global shortcuts: {error}");
                return;
            }
        };
        let _ = slint::invoke_from_event_loop(move || {
            DESKTOP_SHORTCUTS.with(|cache| *cache.borrow_mut() = shortcuts);
            if let Some(ui) = ui.upgrade()
                && ui.global::<ActionPicker>().get_opened()
            {
                ui.global::<ActionPicker>().invoke_refresh();
            }
        });
    });
}

fn read_kde_shortcuts() -> Result<Vec<DesktopShortcutItem>, String> {
    #[cfg(target_os = "linux")]
    {
        Ok(lhc_core::platform::kde_shortcuts::list()?
            .into_iter()
            .map(|shortcut| DesktopShortcutItem {
                value: Action::KdeShortcut(shortcut.id())
                    .format()
                    .unwrap_or_default(),
                component: shortcut.component_label,
                label: shortcut.label,
            })
            .collect())
    }
    #[cfg(not(target_os = "linux"))]
    Ok(Vec::new())
}

fn catalog(ui: &SettingsWindow, config: &ConfigDocument) -> Vec<PickerItem> {
    let in_development = config.settings().in_development;
    let mut items: Vec<_> = CATEGORIES
        .iter()
        .enumerate()
        .filter(|(category, _)| {
            in_development || KEY_CATEGORIES[*category] != PickerCategory::Mouse
        })
        .flat_map(|(category, keys)| {
            keys.iter().map(move |key| PickerItem {
                value: (*key).into(),
                label: (*key).into(),
                category: KEY_CATEGORIES[category],
                notice: Msg::None.to_ui(),
            })
        })
        .collect();
    let enabled = config.settings().commands_enabled;
    let config = config.config();
    items.extend(actions::catalog(&config).into_iter().filter_map(|entry| {
        let category = match &entry.action {
            Action::Macro(_) if in_development => PickerCategory::Macros,
            Action::Macro(_) => return None,
            Action::BuiltinMacro(_) => PickerCategory::BuiltinMacros,
            Action::Command(_) if enabled && in_development => PickerCategory::Commands,
            Action::Command(_) => return None,
            Action::App(id)
                if !in_development
                    && (id.starts_with("gameMode")
                        || id.starts_with("showQuickMenu")
                        || id.starts_with("showEmojiMenu")) =>
            {
                return None;
            }
            Action::App(_) => PickerCategory::App,
            Action::System(_) => PickerCategory::SystemActions,
            _ => return None,
        };
        let notice = execution_notice(&entry.action, &config);
        let value = entry.action.format()?;
        let label = action_label(ui, entry.name).unwrap_or_else(|| value.clone());
        Some(PickerItem {
            value: value.into(),
            label: label.into(),
            category,
            notice: notice.to_ui(),
        })
    }));
    if in_development {
        DESKTOP_SHORTCUTS.with(|cache| {
            items.extend(cache.borrow().iter().map(|shortcut| PickerItem {
                value: shortcut.value.as_str().into(),
                label: format!("{}: {}", shortcut.component, shortcut.label).into(),
                category: PickerCategory::DesktopShortcuts,
                notice: Msg::None.to_ui(),
            }));
        });
    }
    items
}

/// Why a valid action may still not run, shown as a notice.
fn execution_notice(action: &Action, config: &lhc_core::profile::model::AppConfig) -> Msg {
    match actions::execution_issue(action, config) {
        Some(actions::ExecutionIssue::CommandsDisabled) => Msg::CommandsDisabled,
        Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
        None => Msg::None,
    }
}

/// What the page opening the picker allows; built by that page from its
/// own state so the picker does not read other pages.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PickerRequest {
    pub title: PickerTitle,
    /// Only a key can be chosen (no action).
    pub key_only: bool,
    /// A rule trigger: a key or key chord.
    pub trigger_chords: bool,
    pub macro_step: bool,
    /// Assigns a layer key directly rather than a dialog draft.
    pub layer_action: bool,
    /// The tap or held-key action of a rule.
    pub rule_action: bool,
    /// The current assignment swallows the key.
    pub swallowed: bool,
    pub hold_modifiers: bool,
    pub allow_default: bool,
    pub allow_swallow: bool,
    pub inherit_default: bool,
    pub empty_default: bool,
    pub quick_action: bool,
    /// The key another key is held as: only held keys are allowed.
    pub hold_action: bool,
    /// Holding also activates a layer.
    pub hold_layer: bool,
}

impl PickerRequest {
    /// Request of a target that has no page-specific options.
    fn plain(target: PickerTarget, key_only: bool) -> Self {
        Self {
            key_only,
            macro_step: target == PickerTarget::MacroStep,
            title: if target == PickerTarget::MacroStep {
                PickerTitle::MacroStep
            } else {
                PickerTitle::Auto
            },
            quick_action: target == PickerTarget::QuickAction,
            allow_default: target == PickerTarget::BaseKey,
            ..Self::default()
        }
    }

    /// Initial behavior for a current value (`empty` when unassigned).
    fn behavior(&self, empty: bool) -> PickerBehavior {
        if self.swallowed && self.allow_swallow {
            PickerBehavior::Swallow
        } else if self.allow_default && (empty || self.swallowed) {
            PickerBehavior::Native
        } else {
            PickerBehavior::Action
        }
    }

    /// Label of the "default" behavior.
    fn native_label(&self) -> Msg {
        if self.inherit_default {
            Msg::PickerInherit
        } else if self.quick_action {
            Msg::PickerEmptyCell
        } else if self.empty_default {
            Msg::PickerNoAction
        } else if self.hold_layer {
            Msg::PickerLayerOnly
        } else {
            Msg::PickerNative
        }
    }
}

/// Ask the page owning `target` what the picker may assign.
fn request(
    ui: &SettingsWindow,
    document: &Document,
    target: PickerTarget,
    index: i32,
    key_only: bool,
) -> PickerRequest {
    let mut request = match target {
        PickerTarget::Rule => rules::picker_request(ui, key_only),
        PickerTarget::LayerAction | PickerTarget::ExtraAction | PickerTarget::ExtraKey => {
            layers::picker_request(ui, document, target, index)
        }
        PickerTarget::MacroStep | PickerTarget::QuickAction | PickerTarget::BaseKey => {
            PickerRequest::plain(target, key_only)
        }
    };
    request.key_only = key_only;
    request.allow_default &= !key_only;
    request
}

/// Which values the open picker accepts. The list and the Apply check use
/// the same rules, so a listed value can always be applied.
#[derive(Debug, Default)]
struct Filter {
    key_only: bool,
    /// Rule trigger: a key or key chord.
    trigger: bool,
    /// Only keys that can be held.
    held_key: bool,
    macro_step: bool,
    /// Macro being edited, which cannot call itself.
    excluded_macro: String,
    /// Keys that already have an extra row.
    unavailable_keys: Vec<String>,
}

impl Filter {
    /// Restrictions of the open picker; list items are checked with it.
    fn admits(&self, value: &str) -> bool {
        if self.unavailable_keys.iter().any(|key| key == value) {
            return false;
        }
        if self.trigger {
            return lhc_core::profile::key_catalog::valid_trigger(value);
        }
        if self.key_only {
            return CATEGORIES.iter().any(|keys| keys.contains(&value));
        }
        if self.held_key && !actions::valid_held_key(value) {
            return false;
        }
        !matches!(Action::parse(Some(value)),
            Action::Macro(id) if !self.excluded_macro.is_empty() && id == self.excluded_macro)
    }

    /// Whether `value` can be applied; `config` validates references.
    fn accepts(&self, value: &str, config: Option<&ConfigDocument>) -> bool {
        if value.trim().is_empty() || !self.admits(value) {
            return false;
        }
        if self.key_only || self.trigger {
            return true;
        }
        let action = Action::parse(Some(value));
        match &action {
            Action::Keys(_) if !actions::valid_held_key(value) => return false,
            Action::Text(text) if text.is_empty() => return false,
            Action::Pause(ms) => return self.macro_step && valid_pause(ms),
            _ => {}
        }
        config.is_none_or(|config| actions::validate(&action, &config.config()).is_none())
    }
}

/// Filter of the open picker.
fn filter(
    ui: &SettingsWindow,
    document: &Document,
    picker: &ActionPicker,
    target: Option<(PickerTarget, i32)>,
) -> Filter {
    let target = target.map(|(target, _)| target);
    let key_only = picker.get_key_only();
    Filter {
        key_only,
        trigger: key_only && target == Some(PickerTarget::Rule),
        held_key: picker.get_hold_action(),
        macro_step: picker.get_macro_step(),
        excluded_macro: if picker.get_macro_step() {
            super::macros::picker_macro_id(ui)
        } else {
            String::new()
        },
        unavailable_keys: if target == Some(PickerTarget::ExtraKey) {
            layers::taken_extra_keys(ui, document)
        } else {
            Vec::new()
        },
    }
}

/// Warning for the command being edited: a missing working directory or
/// a script file that cannot be executed.
fn command_notice(picker: &ActionPicker) -> Msg {
    use lhc_core::mapper::commands::{DraftIssue, draft_issue};
    if !matches!(
        Action::parse(Some(picker.get_value().as_str())),
        Action::Command(_)
    ) {
        return Msg::None;
    }
    let directory = picker.get_command_directory();
    match draft_issue(
        &picker.get_command_script(),
        Some(directory.as_str()).filter(|d| !d.trim().is_empty()),
    ) {
        Some(DraftIssue::MissingDirectory) => Msg::CommandDirectoryMissing,
        Some(DraftIssue::NotExecutable) => Msg::ScriptNotExecutable,
        None => Msg::None,
    }
}

/// The open picker: who receives the value and the catalog built on open.
#[derive(Default)]
struct Session {
    target: Option<(PickerTarget, i32)>,
    catalog: Vec<PickerItem>,
    command: Option<lhc_core::profile::model::Command>,
}

pub(super) fn bind(ui: &SettingsWindow, document: &Rc<Document>) {
    let session = Rc::new(RefCell::new(Session::default()));
    let picker = ui.global::<ActionPicker>();
    picker.set_shortcut_desktop(shortcut_desktop());
    load_desktop_shortcuts(ui.as_weak());

    let weak = ui.as_weak();
    picker.on_begin_capture(move || {
        CAPTURE.with(|state| *state.borrow_mut() = CaptureState::default());
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(!picker.get_capturing());
            picker.set_capture_keys("".into());
        }
    });

    on!(ui, picker.on_select_behavior => |ui, behavior| {
        let picker = ui.global::<ActionPicker>();
        if (behavior == PickerBehavior::Native && !picker.get_allow_default())
            || (behavior == PickerBehavior::Swallow && !picker.get_allow_swallow())
        {
            return;
        }
        picker.set_behavior(behavior);
        picker.set_ignore_key(behavior == PickerBehavior::Swallow);
        picker.set_capturing(false);
        picker.set_error(Msg::None.to_ui());
        picker.invoke_refresh();
    });

    let weak = ui.as_weak();
    let (doc, state) = (document.clone(), session.clone());
    picker.on_open(move |target, index, value, key_only| {
        let Some(ui) = weak.upgrade() else { return };
        load_desktop_shortcuts(ui.as_weak());
        let catalog = catalog(&ui, &doc.read());
        let category = catalog
            .iter()
            .find(|item| item.value == value)
            .map(|item| item.category)
            .unwrap_or(match Action::parse(Some(value.as_str())) {
                Action::Text(_) => PickerCategory::Text,
                Action::KdeShortcut(_) => PickerCategory::DesktopShortcuts,
                Action::Pause(_) if target == PickerTarget::MacroStep => PickerCategory::Pause,
                _ => PickerCategory::Modifiers,
            });
        let request = request(&ui, &doc, target, index, key_only);
        let behavior = request.behavior(value.is_empty());
        let original = match behavior {
            PickerBehavior::Swallow => ui
                .global::<Locale>()
                .invoke_text(Msg::PickerSuppress.to_ui()),
            PickerBehavior::Native => ui
                .global::<Locale>()
                .invoke_text(request.native_label().to_ui()),
            PickerBehavior::Action => catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
                .or_else(|| {
                    let Action::Command(id) = Action::parse(Some(value.as_str())) else {
                        return None;
                    };
                    let config = doc.read();
                    let command = config.layout().commands.iter().find(|c| c.id == id)?;
                    Some(command.display_name().into())
                })
                .unwrap_or_else(|| value.clone()),
        };
        *state.borrow_mut() = Session {
            target: Some((target, index)),
            catalog,
            command: None,
        };
        let picker = ui.global::<ActionPicker>();
        picker.set_command_help(if cfg!(target_os = "windows") {
            Msg::CommandHelpWindows.to_ui()
        } else if cfg!(target_os = "macos") {
            Msg::CommandHelpMacos.to_ui()
        } else {
            Msg::CommandHelpLinux.to_ui()
        });
        picker.set_commands_enabled(doc.read().settings().commands_enabled);
        picker.set_subject("".into());
        picker.set_command_name("".into());
        picker.set_command_script("".into());
        picker.set_command_directory("".into());
        picker.set_command_notice(Msg::None.to_ui());
        picker.set_dialog_title(request.title);
        picker.set_layer_action(request.layer_action);
        picker.set_rule_action(request.rule_action);
        picker.set_hold_modifiers(request.hold_modifiers);
        picker.set_allow_default(request.allow_default);
        picker.set_allow_swallow(request.allow_swallow);
        picker.set_inherit_default(request.inherit_default);
        picker.set_empty_default(request.empty_default);
        picker.set_quick_action(request.quick_action);
        picker.set_hold_action(request.hold_action);
        picker.set_hold_layer(request.hold_layer);
        picker.set_behavior(behavior);
        picker.set_ignore_key(behavior == PickerBehavior::Swallow);
        picker.set_touched(false);
        picker.set_error(Msg::None.to_ui());
        picker.set_key_only(request.key_only);
        picker.set_trigger_chords(request.trigger_chords);
        picker.set_macro_step(request.macro_step);
        picker.set_original(original);
        picker.set_value(value);
        picker.set_query("".into());
        picker.set_capturing(false);
        picker.set_category(category);
        picker.set_opened(true);
        picker.invoke_refresh();
    });

    on!(ui, picker.on_edit_command, doc = document, state = session => |ui| {
        if !doc.read().settings().commands_enabled {
            return;
        }
        let picker = ui.global::<ActionPicker>();
        let value = picker.get_value().to_string();
        let id = match Action::parse(Some(value.as_str())) {
            Action::Command(id) => id,
            _ => lhc_core::profile::ids::generate("cmd-"),
        };
        // Keep the script of the other operating systems.
        let mut command = doc
            .read()
            .layout()
            .commands
            .iter()
            .find(|command| command.id == id)
            .cloned()
            .unwrap_or_else(|| lhc_core::profile::model::Command {
                id: id.clone(),
                name: String::new(),
                linux: String::new(),
                windows: String::new(),
                working_directory: None,
            });
        command.name = picker.get_command_name().to_string();
        command.set_script(picker.get_command_script().to_string());
        command.working_directory = (!picker.get_command_directory().trim().is_empty())
            .then(|| picker.get_command_directory().trim().to_owned());
        state.borrow_mut().command = Some(command);
        picker.set_value(Action::Command(id).format().unwrap_or_default().into());
        picker.set_touched(true);
        picker.invoke_refresh();
    });

    for directory in [false, true] {
        let weak = ui.as_weak();
        let choose = move || {
            let Some(ui) = weak.upgrade() else { return };
            let title = ui
                .global::<Locale>()
                .invoke_text(
                    if directory {
                        Msg::ChooseCommandDirectory
                    } else {
                        Msg::ChooseCommandScript
                    }
                    .to_ui(),
                )
                .to_string();
            let weak = ui.as_weak();
            std::thread::spawn(move || {
                let picked = lhc_core::platform::file_chooser::pick(&title, directory);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    let picker = ui.global::<ActionPicker>();
                    match picked {
                        Ok(Some(path)) => {
                            let path = path.to_string_lossy();
                            if directory {
                                picker.set_command_directory(path.as_ref().into());
                            } else {
                                let quoted = lhc_core::mapper::commands::invoke_path(&path);
                                let script = picker.get_command_script();
                                picker.set_command_script(if script.trim().is_empty() {
                                    quoted.into()
                                } else {
                                    format!("{} {quoted}", script.trim_end()).into()
                                });
                            }
                            picker.invoke_edit_command();
                        }
                        Ok(None) => {}
                        Err(error) => picker.set_error(Msg::ActionFailed(error).to_ui()),
                    }
                });
            });
        };
        if directory {
            picker.on_choose_command_directory(choose);
        } else {
            picker.on_choose_command_file(choose);
        }
    }

    on!(ui, picker.on_open_command_settings => |ui| {
        ui.global::<ActionPicker>().invoke_dismiss_picker();
        ui.global::<crate::ui::SettingsEditor>()
            .set_tab(crate::ui::SettingsTab::Commands);
        ui.invoke_navigate(crate::ui::Page::Settings, crate::ui::MenuKind::Emoji);
    });

    on!(ui, picker.on_refresh, doc = document, state = session => |ui| {
        let picker = ui.global::<ActionPicker>();
        state.borrow_mut().catalog = catalog(&ui, &doc.read());
        let state = state.borrow();
        let filter = filter(&ui, &doc, &picker, state.target);
        let entries: Vec<&PickerItem> = state
            .catalog
            .iter()
            // Each action owns its command; existing ones are not offered.
            .filter(|entry| entry.category != PickerCategory::Commands && filter.admits(&entry.value))
            .collect();
        picker.set_categories(ModelRc::new(VecModel::from(
            KEY_CATEGORIES
                .into_iter()
                .chain(ACTION_CATEGORIES)
                .map(|category| PickerCategoryCount {
                    category,
                    count: entries.iter().filter(|e| e.category == category).count() as i32,
                })
                .collect::<Vec<_>>(),
        )));
        if picker.get_macro_step()
            && picker.get_category() == PickerCategory::Pause
            && !matches!(
                Action::parse(Some(picker.get_value().as_str())),
                Action::Pause(_)
            )
        {
            let pause = super::macros::picker_macro_pause(&ui)
            .unwrap_or_else(|| doc.read().settings().default_macro_step_pause_ms)
            .clamp(1, u64::from(MAX_PAUSE_MS));
            picker.set_value(
                Action::Pause(pause.to_string())
                    .format()
                    .unwrap_or_default()
                    .into(),
            );
            picker.set_touched(true);
        }
        let value = picker.get_value();
        if let Some(command) = doc
            .read()
            .layout()
            .commands
            .iter()
            .find(|command| assigns_command(&value, &command.id))
            && state
                .command
                .as_ref()
                .is_none_or(|draft| draft.id != command.id)
        {
            picker.set_command_name(command.name.clone().into());
            picker.set_command_script(command.script().into());
            picker.set_command_directory(
                command.working_directory.clone().unwrap_or_default().into(),
            );
        }
        picker.set_command_notice(command_notice(&picker).to_ui());
        picker.set_value_category(category_for_value(&value));
        let action = Action::parse(Some(value.as_str()));
        let text = match &action {
            Action::Text(text) => text.as_str(),
            _ => "",
        };
        picker.set_text_content(text.into());
        picker.set_text_preview(super::one_line(text).into());
        picker.set_text_lines(if text.contains('\n') {
            text.split('\n').count() as i32
        } else {
            0
        });
        picker.set_pause_content(
            match &action {
                Action::Pause(ms) => ms.as_str(),
                _ => "",
            }
            .into(),
        );
        let query = picker.get_query().to_lowercase();
        let mut category = picker.get_category();
        if query.is_empty()
            && is_key_category(category)
            && !entries.iter().any(|entry| entry.category == category)
            && let Some(entry) = entries.iter().find(|entry| is_key_category(entry.category))
        {
            category = entry.category;
            picker.set_category(category);
        }
        picker.set_items(ModelRc::new(VecModel::from(
            entries
                .into_iter()
                .filter(|entry| {
                    if query.is_empty() {
                        entry.category == category
                    } else {
                        entry.label.to_lowercase().contains(&query)
                            || entry.value.to_lowercase().contains(&query)
                    }
                })
                .cloned()
                .collect::<Vec<_>>(),
        )));
        picker.set_notice(execution_notice(&action, &doc.read().config()).to_ui());
        let valid = match picker.get_behavior() {
            PickerBehavior::Native => picker.get_allow_default(),
            PickerBehavior::Swallow => picker.get_allow_swallow(),
            PickerBehavior::Action => match state
                .command
                .as_ref()
                .filter(|command| assigns_command(&value, &command.id))
            {
                Some(command) => {
                    filter.admits(&value)
                        && doc.read().settings().commands_enabled
                        && !command.script().trim().is_empty()
                }
                None => filter.accepts(&value, Some(&doc.read())),
            },
        };
        picker.set_valid(valid);
    });

    let weak = ui.as_weak();
    let state = session.clone();
    picker.on_dismiss_picker(move || {
        state.borrow_mut().target = None;
        if let Some(ui) = weak.upgrade() {
            let picker = ui.global::<ActionPicker>();
            picker.set_capturing(false);
            picker.set_opened(false);
        }
    });

    on!(ui, picker.on_apply, doc = document => |ui| {
        let picker = ui.global::<ActionPicker>();
        picker.invoke_refresh();
        if !picker.get_valid() || picker.get_capturing() {
            return;
        }
        picker.set_error(Msg::None.to_ui());
        let value = if picker.get_behavior() == PickerBehavior::Action {
            picker.get_value()
        } else {
            "".into()
        };
        let command = session
            .borrow()
            .command
            .clone()
            .filter(|command| assigns_command(&value, &command.id));
        if let Some(command) = command
            && let Err(error) = doc.edit(crate::document::View::Shell, |config| {
                config.save_command(command)
            })
        {
            picker.set_error(Msg::from(&error).to_ui());
            return;
        }
        let (target, label) = {
            let state = session.borrow();
            let label = state
                .catalog
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.label.clone())
                .or_else(|| {
                    doc.read()
                        .layout()
                        .commands
                        .iter()
                        .find(|command| assigns_command(&value, &command.id))
                        .map(|command| command.name.clone().into())
                })
                .unwrap_or_default();
            (state.target, label)
        };
        let Some((target, index)) = target else {
            return;
        };
        let mut next_step = false;
        match target {
            PickerTarget::Rule => {
                if let Err(error) = rules::choose(&ui, &doc, &value) {
                    picker.set_error(error.to_ui());
                }
            }
            PickerTarget::MacroStep => {
                let editor = ui.global::<MacroEditor>();
                editor.invoke_set_step(editor.get_picker_macro(), index, value);
            }
            PickerTarget::QuickAction => ui
                .global::<MenuEditor>()
                .invoke_set_action(index, value, label),
            PickerTarget::LayerAction => {
                if picker.get_layer_action() {
                    if let Err(error) = layers::assign(
                        &ui,
                        &doc,
                        index,
                        &value,
                        picker.get_ignore_key() && value.is_empty(),
                        picker.get_hold_modifiers(),
                    ) {
                        picker.set_error(error.to_ui());
                    }
                } else {
                    ui.global::<LayersEditor>().set_dialog_action(value);
                }
            }
            PickerTarget::ExtraKey | PickerTarget::ExtraAction => {
                match layers::assign_extra(
                    &ui,
                    &doc,
                    index,
                    &value,
                    target == PickerTarget::ExtraKey,
                    picker.get_ignore_key() && value.is_empty(),
                    picker.get_hold_modifiers(),
                ) {
                    Ok(next) => next_step = next,
                    Err(error) => picker.set_error(error.to_ui()),
                }
            }
            PickerTarget::BaseKey => {
                if let Err(error) = keys::assign(&ui, &doc, index, &value) {
                    picker.set_error(error.to_ui());
                }
            }
        }
        if picker.get_error().id.is_empty() {
            picker.invoke_dismiss_picker();
            if next_step {
                layers::pick_new_extra_action(&ui);
            }
        }
    });
}

/// Keyboard capture for the picker; returns `true` to withhold the event from Slint.
pub fn capture(ui: &SettingsWindow, event: &slint::winit_030::winit::event::WindowEvent) -> bool {
    use slint::winit_030::winit::event::WindowEvent;
    let picker = ui.global::<ActionPicker>();
    if !picker.get_opened() || !picker.get_capturing() {
        return false;
    }
    if let WindowEvent::Focused(false) = event {
        picker.set_capturing(false);
        return false;
    }
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    let Some(key) = physical_key_name(event.physical_key) else {
        return true;
    };
    if event.repeat {
        return true;
    }
    let value = CAPTURE.with(|state| {
        let mut state = state.borrow_mut();
        let value = state.update(
            &key,
            event.state,
            picker.get_trigger_chords(),
            picker.get_key_only(),
        );
        picker.set_capture_keys(state.held.join("+").into());
        value
    });
    if let Some(value) = value {
        picker.set_touched(true);
        picker.set_value(value.into());
        picker.set_capturing(false);
        picker.invoke_refresh();
    }
    // Slint tracks modifiers from key events, not ModifiersChanged. Swallowing
    // a release here can leave Ctrl/Shift stuck when capture ends and block typing.
    lhc_core::profile::key_catalog::modifier_key(&key).is_none()
}

/// Config key code of a physical key. Keys winit cannot name on Linux
/// (most media, launch and laptop keys) arrive with their evdev code.
fn physical_key_name(key: slint::winit_030::winit::keyboard::PhysicalKey) -> Option<String> {
    use slint::winit_030::winit::keyboard::PhysicalKey;
    match key {
        PhysicalKey::Code(code) => Some(key_name(code)),
        #[cfg(target_os = "linux")]
        PhysicalKey::Unidentified(slint::winit_030::winit::keyboard::NativeKeyCode::Xkb(code)) => {
            u16::try_from(code)
                .ok()
                .and_then(lhc_core::mapper::keys::evdev_to_code)
                .map(str::to_owned)
        }
        PhysicalKey::Unidentified(_) => None,
    }
}

/// Config key code of a winit key code. winit names its `KeyCode` variants
/// after the W3C `KeyboardEvent.code` values the config uses, so the
/// variant name is the code except for the Meta keys.
fn key_name(code: slint::winit_030::winit::keyboard::KeyCode) -> String {
    let raw = format!("{code:?}");
    match raw.as_str() {
        "SuperLeft" => "MetaLeft".into(),
        "SuperRight" => "MetaRight".into(),
        _ => raw,
    }
}

#[derive(Default)]
struct CaptureState {
    held: Vec<String>,
    keys: Vec<String>,
}

impl CaptureState {
    fn update(
        &mut self,
        key: &str,
        state: slint::winit_030::winit::event::ElementState,
        trigger: bool,
        key_only: bool,
    ) -> Option<String> {
        use slint::winit_030::winit::event::ElementState;
        let known = if trigger {
            lhc_core::profile::key_catalog::valid_trigger(key)
        } else {
            lhc_core::profile::key_catalog::valid_key(key)
        };
        if !known {
            return None;
        }
        if state == ElementState::Pressed {
            if !self.held.iter().any(|k| k == key) {
                self.held.push(key.to_owned());
            }
            if !self.keys.iter().any(|k| k == key) {
                self.keys.push(key.to_owned());
            }
            return None;
        }
        let index = self.held.iter().position(|k| k == key)?;
        self.held.remove(index);
        if !self.held.is_empty() {
            return None;
        }
        let keys = std::mem::take(&mut self.keys);
        if trigger || key_only || keys.len() == 1 {
            return Some(keys.join("+"));
        }
        let mut modifiers = Vec::new();
        let mut ordinary = Vec::new();
        for key in &keys {
            let modifier = match key.as_str() {
                "ControlLeft" | "ControlRight" => Some("Ctrl"),
                "AltLeft" | "AltRight" => Some("Alt"),
                "ShiftLeft" | "ShiftRight" => Some("Shift"),
                "MetaLeft" | "MetaRight" => Some("Meta"),
                _ => None,
            };
            if let Some(modifier) = modifier {
                if !modifiers.contains(&modifier) {
                    modifiers.push(modifier);
                }
            } else {
                ordinary.push(key.as_str());
            }
        }
        Some(
            ["Ctrl", "Alt", "Shift", "Meta"]
                .into_iter()
                .filter(|m| modifiers.contains(m))
                .chain(ordinary)
                .collect::<Vec<_>>()
                .join("+"),
        )
    }
}

thread_local! { static CAPTURE: RefCell<CaptureState> = const { RefCell::new(CaptureState { held: Vec::new(), keys: Vec::new() }) }; }

#[cfg(test)]
#[path = "picker/tests.rs"]
mod tests;
