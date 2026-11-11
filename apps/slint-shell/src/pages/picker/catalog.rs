//! Action catalog, labels, and discovery of desktop shortcuts and
//! application actions.

use crate::{
    i18n::Msg,
    ui::{ActionPicker, Locale, PickerCategory, PickerItem, SettingsWindow, ShortcutDesktop},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicBool, Ordering},
};
use tecloid_core::config::{
    actions::{self, Action, ActionName},
    key_catalog::CATEGORIES,
    model::ResolvedAppConfig,
};

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
pub(super) const ACTION_CATEGORIES: [PickerCategory; 9] = [
    PickerCategory::Macros,
    PickerCategory::BuiltinMacros,
    PickerCategory::App,
    PickerCategory::SystemActions,
    PickerCategory::DesktopShortcuts,
    PickerCategory::AppActions,
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
        Action::DesktopEntry(_) => return PickerCategory::AppActions,
        Action::Text(_) => return PickerCategory::Text,
        Action::Pause(_) => return PickerCategory::Pause,
        Action::Native | Action::Swallow | Action::Keys(_) | Action::Unknown(_) => {}
    }
    // The last key of a chord is the main one; modifiers come first.
    for key in value.split('+').rev() {
        if tecloid_core::config::key_catalog::TRIGGER_MODIFIERS.contains(&key) {
            return PickerCategory::Modifiers;
        }
        if let Some(index) = CATEGORIES.iter().position(|keys| keys.contains(&key)) {
            return KEY_CATEGORIES[index];
        }
    }
    PickerCategory::OtherKeys
}

/// Whether `value` belongs to a feature shown only in development mode:
/// macros, commands, desktop shortcuts, application actions, mouse
/// buttons and some app actions.
pub(super) fn development_only(value: &str) -> bool {
    match Action::parse(Some(value)) {
        Action::Macro(_)
        | Action::Command(_)
        | Action::KdeShortcut(_)
        | Action::DesktopEntry(_) => true,
        Action::System(_) => true,
        Action::App(id) => id.starts_with("gameMode"),
        Action::Keys(_) | Action::Unknown(_) => category_for_value(value) == PickerCategory::Mouse,
        _ => false,
    }
}

/// Whether `ms` is a valid macro pause.
pub fn valid_pause(ms: &str) -> bool {
    tecloid_core::config::timing::PAUSE.parse(ms).is_some()
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

/// A native global shortcut of the desktop environment or an action of an
/// installed application, as the picker and key caps show it.
#[derive(Clone)]
pub(crate) struct DesktopShortcutItem {
    /// Backend-prefixed id, e.g. `kde:component/shortcut` or
    /// `xdg:desktop-id/action`.
    pub value: String,
    /// Component or application name.
    pub component: String,
    pub label: String,
    /// `DesktopShortcuts` or `AppActions`.
    pub category: PickerCategory,
}

thread_local! {
    /// Global shortcuts of the running desktop session and application
    /// actions; empty until read and off Linux.
    pub(crate) static DESKTOP_SHORTCUTS: RefCell<Vec<DesktopShortcutItem>> = const { RefCell::new(Vec::new()) };
    /// Bumped whenever [`DESKTOP_SHORTCUTS`] changes, so an open picker
    /// knows to rebuild its catalog.
    pub(super) static SHORTCUTS_GENERATION: Cell<u64> = const { Cell::new(0) };
    /// When the shortcuts were last read successfully.
    static SHORTCUTS_READ_AT: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
}

/// How long a successful read stays fresh; opening the picker again within
/// it does not query the desktop. A failed read is retried on every open.
const SHORTCUTS_FRESH: std::time::Duration = std::time::Duration::from_secs(30);

/// Whether a read made at `read_at` is still fresh at `now`.
pub(super) fn shortcuts_fresh(
    read_at: Option<std::time::Instant>,
    now: std::time::Instant,
) -> bool {
    read_at.is_some_and(|at| now.saturating_duration_since(at) < SHORTCUTS_FRESH)
}

/// A read of the desktop shortcuts is running; later requests wait for it.
static SHORTCUTS_LOADING: AtomicBool = AtomicBool::new(false);

/// Desktop environment whose native shortcuts this session can list.
pub(super) fn shortcut_desktop() -> ShortcutDesktop {
    #[cfg(target_os = "linux")]
    {
        use tecloid_core::platform::linux;
        if matches!(linux::detect().desktop, linux::Desktop::Kde) {
            return ShortcutDesktop::Kde;
        }
    }
    ShortcutDesktop::None
}

/// Re-read the desktop's global shortcuts and the application actions in
/// the background and refresh an open picker when they arrive. Only one
/// read runs at a time, so an older result never overwrites a newer one.
pub(super) fn load_desktop_shortcuts(ui: slint::Weak<SettingsWindow>) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let desktop = shortcut_desktop();
    if shortcuts_fresh(SHORTCUTS_READ_AT.with(Cell::get), std::time::Instant::now())
        || SHORTCUTS_LOADING.swap(true, Ordering::AcqRel)
    {
        return;
    }
    std::thread::spawn(move || {
        let (mut shortcuts, result) = read_desktop_items(desktop);
        SHORTCUTS_LOADING.store(false, Ordering::Release);
        if let Err(error) = &result {
            log::warn!("[picker] cannot read desktop global shortcuts: {error}");
        }
        if let Err(error) = slint::invoke_from_event_loop(move || {
            let failed = result.is_err();
            {
                if failed {
                    // Keep the shortcuts of the last successful read.
                    DESKTOP_SHORTCUTS.with(|cache| {
                        let kept = cache
                            .borrow()
                            .iter()
                            .filter(|item| item.category == PickerCategory::DesktopShortcuts)
                            .cloned()
                            .collect::<Vec<_>>();
                        shortcuts.splice(0..0, kept);
                    });
                } else {
                    SHORTCUTS_READ_AT.with(|at| at.set(Some(std::time::Instant::now())));
                }
                let changed = DESKTOP_SHORTCUTS.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    let changed = cache.len() != shortcuts.len()
                        || cache.iter().zip(&shortcuts).any(|(old, new)| {
                            old.value != new.value
                                || old.label != new.label
                                || old.component != new.component
                        });
                    *cache = shortcuts;
                    changed
                });
                if changed {
                    SHORTCUTS_GENERATION.with(|generation| generation.set(generation.get() + 1));
                }
            }
            if let Some(ui) = ui.upgrade() {
                let picker = ui.global::<ActionPicker>();
                picker.set_shortcuts_failed(failed);
                if picker.get_opened() {
                    picker.invoke_refresh();
                }
            }
        }) {
            log::warn!("[picker] desktop shortcuts arrived after the event loop ended: {error}");
        }
    });
}

/// KDE shortcuts (on KDE) followed by application actions; the result
/// reports whether the desktop's shortcut service answered.
fn read_desktop_items(desktop: ShortcutDesktop) -> (Vec<DesktopShortcutItem>, Result<(), String>) {
    #[cfg(target_os = "linux")]
    {
        let mut items = Vec::new();
        let mut result = Ok(());
        if desktop == ShortcutDesktop::Kde {
            match tecloid_core::platform::kde_shortcuts::list() {
                Ok(shortcuts) => items.extend(shortcuts.into_iter().map(|shortcut| {
                    DesktopShortcutItem {
                        value: Action::KdeShortcut(shortcut.id())
                            .format()
                            .unwrap_or_default(),
                        component: shortcut.component_label,
                        label: shortcut.label,
                        category: PickerCategory::DesktopShortcuts,
                    }
                })),
                Err(error) => result = Err(error),
            }
        }
        items.extend(
            tecloid_core::platform::desktop_entries::list()
                .into_iter()
                .map(|action| DesktopShortcutItem {
                    value: Action::DesktopEntry(action.id())
                        .format()
                        .unwrap_or_default(),
                    component: action.app_name,
                    label: action.label,
                    category: PickerCategory::AppActions,
                }),
        );
        (items, result)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = desktop;
        (Vec::new(), Ok(()))
    }
}

pub(super) fn catalog(ui: &SettingsWindow, config: &ResolvedAppConfig) -> Vec<PickerItem> {
    let in_development = config.settings.in_development;
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
    if ui.global::<crate::ui::ActionPicker>().get_trigger_chords() {
        items.extend(
            tecloid_core::config::key_catalog::TRIGGER_MODIFIERS
                .iter()
                .map(|key| PickerItem {
                    value: (*key).into(),
                    label: (*key).into(),
                    category: PickerCategory::Modifiers,
                    notice: Msg::None.to_ui(),
                }),
        );
    }
    let enabled = config.settings.commands_enabled;
    items.extend(actions::catalog(config).into_iter().filter_map(|entry| {
        let category = match &entry.action {
            Action::Macro(_) if in_development => PickerCategory::Macros,
            Action::Macro(_) => return None,
            Action::BuiltinMacro(_) => PickerCategory::BuiltinMacros,
            Action::Command(_) if enabled && in_development => PickerCategory::Commands,
            Action::Command(_) => return None,
            Action::App(_) => PickerCategory::App,
            Action::System(_) if in_development => PickerCategory::SystemActions,
            _ => return None,
        };
        let value = entry.action.format()?;
        if !in_development && development_only(&value) {
            return None;
        }
        let notice = execution_notice(&entry.action, config);
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
                category: shortcut.category,
                notice: Msg::None.to_ui(),
            }));
        });
    }
    items
}

/// Why a valid action may still not run, shown as a notice.
pub(super) fn execution_notice(
    action: &Action,
    config: &tecloid_core::config::model::ResolvedAppConfig,
) -> Msg {
    match actions::execution_issue(action, config) {
        Some(actions::ExecutionIssue::CommandsDisabled) => Msg::CommandsDisabled,
        Some(actions::ExecutionIssue::Unavailable) => Msg::ActionUnavailable,
        None => Msg::None,
    }
}
