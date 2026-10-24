//! Global shortcuts registered with KDE's kglobalaccel, offered as
//! `kde:component/shortcut` actions. The list is read from the running
//! session, so it covers KWin, Plasma and third-party applications alike.

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

/// Every global shortcut of the running KDE session, sorted by component
/// and label. Components that fail to answer are skipped.
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
        let infos: Result<Vec<ShortcutInfo>, _> = Proxy::new(
            &conn,
            dbus::KGLOBALACCEL_SERVICE,
            path.as_str(),
            dbus::KGLOBALACCEL_COMPONENT_INTERFACE,
        )
        .and_then(|proxy| proxy.call("allShortcutInfos", &()));
        let infos = match infos {
            Ok(infos) => infos,
            Err(e) => {
                log::debug!("[kde-shortcuts] {component}: {e}");
                continue;
            }
        };
        out.extend(
            infos.into_iter().map(
                |(name, label, _, component_label, _, _, _, _)| KdeShortcut {
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
            ),
        );
    }
    out.sort_by(|a, b| {
        (a.component_label.to_lowercase(), a.label.to_lowercase())
            .cmp(&(b.component_label.to_lowercase(), b.label.to_lowercase()))
    });
    Ok(out)
}
