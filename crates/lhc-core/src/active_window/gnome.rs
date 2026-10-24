//! GNOME window identities are scoped to the lifetime of the Shell extension.

use std::sync::Mutex;

use super::ActiveWindow;
use crate::platform::gnome;

static EXTERNAL: Mutex<Option<String>> = Mutex::new(None);

pub(super) fn current(titles: bool) -> Option<ActiveWindow> {
    let snapshot = gnome::snapshot(titles).ok()?;
    if let Some(value) = snapshot.external_window.as_ref()
        && let Some(window) = super::linux::window_from_json(value, "title", "appId")
    {
        remember(value, &window);
    }
    let value = snapshot.window?;
    let window = super::linux::window_from_json(&value, "title", "appId")?;
    remember(&value, &window);
    Some(window)
}

fn remember(value: &serde_json::Value, window: &ActiveWindow) {
    if !super::is_text_popup(window)
        && let Ok(mut external) = EXTERNAL.lock()
    {
        *external = identity(value);
    }
}

fn identity(value: &serde_json::Value) -> Option<String> {
    value
        .get("windowId")?
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

pub(super) fn text_target() -> Option<String> {
    // Enqueue runs on the grabbed-keyboard thread; only the text worker may
    // wait for Shell. A stale cached identity fails the live delivery check.
    EXTERNAL.lock().ok()?.clone()
}

pub(super) fn target_is_current(target: &str) -> bool {
    matches_target(gnome::snapshot(false), target)
}

fn matches_target(snapshot: Result<gnome::Snapshot, String>, target: &str) -> bool {
    snapshot
        .ok()
        .and_then(|snapshot| snapshot.window)
        .and_then(|value| identity(&value))
        .is_some_and(|current| current == target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_settings_are_a_destination_but_popups_are_not() {
        let exe = std::env::current_exe().unwrap();
        let mut window = ActiveWindow {
            app_id: crate::storage::APP_ID.into(),
            process_name: exe.file_name().unwrap().to_str().map(str::to_owned),
            ..ActiveWindow::default()
        };
        remember(&serde_json::json!({"windowId": "epoch:settings"}), &window);
        assert_eq!(text_target().as_deref(), Some("epoch:settings"));
        window.app_id = crate::storage::POPUP_APP_ID.into();
        remember(&serde_json::json!({"windowId": "epoch:quick"}), &window);
        assert_eq!(text_target().as_deref(), Some("epoch:settings"));
        *EXTERNAL.lock().unwrap() = None;
    }

    #[test]
    fn delivery_requires_the_live_window_identity_and_bridge() {
        let snapshot = |id: Option<&str>| {
            serde_json::from_value::<gnome::Snapshot>(serde_json::json!({
                "version": 1,
                "window": id.map(|id| serde_json::json!({"appId":"editor", "title":"same", "windowId":id})),
                "layouts": [], "current": null,
            })).map_err(|error| error.to_string())
        };
        assert!(matches_target(snapshot(Some("epoch-a:1")), "epoch-a:1"));
        assert!(!matches_target(snapshot(Some("epoch-a:2")), "epoch-a:1"));
        assert!(!matches_target(snapshot(Some("epoch-b:1")), "epoch-a:1"));
        assert!(!matches_target(snapshot(None), "epoch-a:1"));
        assert!(!matches_target(Err("bridge disabled".into()), "epoch-a:1"));
        assert!(!matches_target(snapshot(Some("")), ""));
    }
}
