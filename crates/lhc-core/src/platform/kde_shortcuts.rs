//! Global shortcuts registered with KDE's kglobalaccel, offered as
//! `kde:component/shortcut` actions. The list is read from the running
//! session, so it covers KWin, Plasma and third-party applications alike.
//! The list leaves out what cannot run or is offered elsewhere: components
//! of applications that are not running (stale entries in
//! `kglobalshortcutsrc`), shortcuts a plain `sys:` action already runs, and
//! desktop-numbered shortcuts above the `sys:` range.

use super::dbus;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

/// One kglobalaccel action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdeShortcut {
    /// Last segment of the component's object path, e.g. `kwin`.
    pub component: String,
    /// Shortcut name passed to `invokeShortcut`, e.g. `Window Close`.
    pub name: String,
    /// Human-readable component name, e.g. `KWin`.
    pub component_label: String,
    /// Human-readable shortcut name, e.g. `Close Window`.
    pub label: String,
}

impl KdeShortcut {
    /// The `kde:` id of this shortcut, without the prefix.
    pub fn id(&self) -> String {
        format!("{}/{}", self.component, self.name)
    }
}

/// `allShortcutInfos` entry: unique name, friendly name, component unique
/// and friendly names, context unique and friendly names, keys, defaults.
type ShortcutInfo = (
    String,
    String,
    String,
    String,
    String,
    String,
    Vec<i32>,
    Vec<i32>,
);

/// Every usable global shortcut of the running KDE session, sorted by
/// component and label; unnamed shortcuts go last within a component.
/// Components that fail to answer are skipped.
pub fn list() -> Result<Vec<KdeShortcut>, String> {
    let conn = Connection::session().map_err(|e| format!("session bus: {e}"))?;
    let accel = Proxy::new(
        &conn,
        dbus::KGLOBALACCEL_SERVICE,
        dbus::KGLOBALACCEL_PATH,
        dbus::KGLOBALACCEL_INTERFACE,
    )
    .map_err(|e| format!("kglobalaccel: {e}"))?;
    let components: Vec<OwnedObjectPath> = accel
        .call("allComponents", &())
        .map_err(|e| format!("kglobalaccel allComponents: {e}"))?;
    let mut out = Vec::new();
    for path in components {
        let Some(component) = path.as_str().rsplit('/').next().map(str::to_owned) else {
            continue;
        };
        let proxy = match Proxy::new(
            &conn,
            dbus::KGLOBALACCEL_SERVICE,
            path.as_str(),
            dbus::KGLOBALACCEL_COMPONENT_INTERFACE,
        ) {
            Ok(proxy) => proxy,
            Err(e) => {
                log::debug!("[kde-shortcuts] {component}: {e}");
                continue;
            }
        };
        // kglobalaccel starts `.desktop` components on demand, so they
        // run even when inactive; any other inactive component belongs to
        // an application that is not running and ignores the call.
        if !launched_on_demand(&component)
            && !proxy.call::<_, _, bool>("isActive", &()).unwrap_or(false)
        {
            log::debug!("[kde-shortcuts] {component}: inactive, skipped");
            continue;
        }
        let infos: Result<Vec<ShortcutInfo>, _> = proxy.call("allShortcutInfos", &());
        let infos = match infos {
            Ok(infos) => infos,
            Err(e) => {
                log::debug!("[kde-shortcuts] {component}: {e}");
                continue;
            }
        };
        out.extend(
            infos
                .into_iter()
                .filter(|(name, ..)| offered(&component, name))
                .map(|(name, label, _, component_label, _, _, _, _)| {
                    (
                        label.is_empty(),
                        KdeShortcut {
                            component: component.clone(),
                            label: if label.is_empty() {
                                name.clone()
                            } else {
                                label
                            },
                            component_label: if component_label.is_empty() {
                                component.clone()
                            } else {
                                component_label
                            },
                            name,
                        },
                    )
                }),
        );
    }
    // A shortcut without a friendly name shows its technical one; those
    // go after the named ones of their component.
    out.sort_by_cached_key(|(unnamed, s)| {
        (
            s.component_label.to_lowercase(),
            *unnamed,
            s.label.to_lowercase(),
        )
    });
    Ok(out.into_iter().map(|(_, shortcut)| shortcut).collect())
}

/// Components named after a `.desktop` file (`org_kde_konsole_desktop`).
fn launched_on_demand(component: &str) -> bool {
    component.ends_with("_desktop")
}

/// Whether the shortcut belongs in the `kde:` list.
fn offered(component: &str, name: &str) -> bool {
    if crate::mapper::system::kde_runs_as_system_action(component, name) {
        return false;
    }
    if component != "kwin" {
        return true;
    }
    // KWin registers these for up to 20+ desktops; `sys:` covers 1…10.
    ["Switch to Desktop ", "Window to Desktop "]
        .iter()
        .find_map(|prefix| name.strip_prefix(prefix))
        .and_then(|n| n.parse::<u32>().ok())
        .is_none_or(|n| n <= crate::config::actions::SYSTEM_NUMBERED_COUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_duplicates_and_desktops_above_ten() {
        assert!(!offered("kwin", "Window Close"));
        assert!(!offered("ksmserver", "Lock Session"));
        assert!(!offered("org_kde_powerdevil", "Turn Off Screen"));
        assert!(offered("kwin", "Switch to Desktop 10"));
        assert!(!offered("kwin", "Switch to Desktop 11"));
        assert!(!offered("kwin", "Window to Desktop 20"));
        assert!(offered("kwin", "Window Fullscreen"));
        assert!(offered("plasmashell", "activate task manager entry 10"));
        assert!(launched_on_demand("org_kde_konsole_desktop"));
        assert!(!launched_on_demand("kmix"));
    }
}
