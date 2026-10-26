//! Action catalog, labels and desktop shortcut discovery.

use crate::{
    i18n::Msg,
    ui::{ActionPicker, Locale, PickerCategory, PickerItem, SettingsWindow, ShortcutDesktop},
};
use lhc_core::{
    config_document::ConfigDocument,
    profile::{
        actions::{self, Action, ActionName},
        key_catalog::CATEGORIES,
    },
};
use slint::ComponentHandle;
use std::cell::RefCell;

/// Picker category of each group of [`CATEGORIES`], in the same order.
pub(super) const KEY_CATEGORIES: [PickerCategory; CATEGORIES.len()] = [
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
pub(super) const ACTION_CATEGORIES: [PickerCategory; 8] = [
    PickerCategory::Macros,
    PickerCategory::BuiltinMacros,
    PickerCategory::App,
    PickerCategory::SystemActions,
    PickerCategory::DesktopShortcuts,
    PickerCategory::Text,
    PickerCategory::Pause,
    PickerCategory::Commands,
];

pub(super) fn is_key_category(category: PickerCategory) -> bool {
    KEY_CATEGORIES.contains(&category)
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

/// Whether `ms` is a valid macro pause.
pub fn valid_pause(ms: &str) -> bool {
    lhc_core::profile::timing::PAUSE.parse(ms).is_some()
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
pub(super) fn shortcut_desktop() -> ShortcutDesktop {
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
pub(super) fn load_desktop_shortcuts(ui: slint::Weak<SettingsWindow>) {
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

pub(super) fn catalog(ui: &SettingsWindow, config: &ConfigDocument) -> Vec<PickerItem> {
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
pub(super) fn execution_notice(
    action: &Action,
    config: &lhc_core::profile::model::AppConfig,
) -> Msg {
    match actions::execution_issue(action, config) {
        Some(actions::ExecutionIssue::CommandsDisabled) => Msg::CommandsDisabled,
        Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
        None => Msg::None,
    }
}
