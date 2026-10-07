//! Choosing the active layout in auto mode (`utils/layoutAutoSwitch.ts`).

use super::model::{AppSettings, LayoutConditionRule, LayoutConditionSet};

/// Observed system state the layout conditions are evaluated against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoSwitchContext {
    /// Short id of the system keyboard layout; `None` when undetectable.
    pub system_layout: Option<String>,
    pub game_mode_active: bool,
    pub game_mode_detection_enabled: bool,
    pub window_title: Option<String>,
    pub window_app_id: Option<String>,
}

impl AutoSwitchContext {
    /// Snapshot of the state cached by the core watchers.
    pub fn current() -> Self {
        let window = crate::runtime_state::active_window();
        let (game_mode_active, game_mode_detection_enabled) = crate::runtime_state::game_mode();
        Self {
            system_layout: crate::runtime_state::layout_short(),
            game_mode_active,
            game_mode_detection_enabled,
            window_title: window.as_ref().map(|window| window.title.clone()),
            window_app_id: window.map(|window| window.app_id),
        }
    }
}

pub fn matches_condition_set(set: &LayoutConditionSet, ctx: &AutoSwitchContext) -> bool {
    if set.game_mode.is_some() && !ctx.game_mode_detection_enabled {
        return false;
    }
    if ctx.game_mode_detection_enabled {
        match set.game_mode.as_deref() {
            Some("on") if !ctx.game_mode_active => return false,
            Some("off") if ctx.game_mode_active => return false,
            _ => {}
        }
    }
    if !set.layouts.is_empty() {
        match &ctx.system_layout {
            Some(layout) if set.layouts.contains(layout) => {}
            _ => return false,
        }
    }
    set.apps.is_empty() || matches_active_window(&set.apps, ctx)
}

fn matches_active_window(needles: &[String], ctx: &AutoSwitchContext) -> bool {
    let title = ctx.window_title.as_deref().unwrap_or("").to_lowercase();
    let app_id = ctx.window_app_id.as_deref().unwrap_or("").to_lowercase();
    if title.is_empty() && app_id.is_empty() {
        return false;
    }
    needles
        .iter()
        .map(|needle| needle.trim().to_lowercase())
        .filter(|needle| !needle.is_empty())
        .any(|needle| title.contains(&needle) || app_id.contains(&needle))
}

/// Whether a layout's rules may fire under `ctx`.
pub fn layout_allowed(rule: Option<&LayoutConditionRule>, ctx: &AutoSwitchContext) -> bool {
    let Some(rule) = rule else {
        return true;
    };
    if rule
        .blacklist
        .as_ref()
        .is_some_and(|blacklist| matches_condition_set(blacklist, ctx))
    {
        return false;
    }
    rule.whitelist
        .as_ref()
        .is_none_or(|whitelist| matches_condition_set(whitelist, ctx))
}

/// `available` sorted by `order`; ids missing from `order` keep their
/// relative position at the end.
pub fn order_layout_ids(available: &[String], order: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in order.iter().chain(available) {
        if available.contains(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out
}

/// First layout enabled in auto mode whose conditions allow it.
pub fn pick_active_layout(
    available: &[String],
    settings: &AppSettings,
    ctx: &AutoSwitchContext,
) -> Option<String> {
    order_layout_ids(available, &settings.layout_order)
        .into_iter()
        .find(|id| {
            let rule = settings.layout_conditions.get(id);
            rule.is_some_and(|rule| rule.enabled_in_auto) && layout_allowed(rule, ctx)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(game_mode: Option<&str>, layouts: &[&str], apps: &[&str]) -> LayoutConditionSet {
        LayoutConditionSet {
            game_mode: game_mode.map(str::to_owned),
            layouts: layouts.iter().map(|s| s.to_string()).collect(),
            apps: apps.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn ctx(layout: Option<&str>, game: bool, title: &str) -> AutoSwitchContext {
        AutoSwitchContext {
            system_layout: layout.map(str::to_owned),
            game_mode_active: game,
            game_mode_detection_enabled: true,
            window_title: Some(title.into()),
            window_app_id: None,
        }
    }

    #[test]
    fn condition_sets_match_like_the_frontend() {
        assert!(matches_condition_set(
            &set(None, &[], &[]),
            &ctx(None, false, "")
        ));
        assert!(!matches_condition_set(
            &set(None, &["us"], &[]),
            &ctx(None, false, "")
        ));
        assert!(matches_condition_set(
            &set(Some("on"), &["us"], &[]),
            &ctx(Some("us"), true, "")
        ));
        assert!(!matches_condition_set(
            &set(Some("off"), &[], &[]),
            &ctx(None, true, "")
        ));
        assert!(matches_condition_set(
            &set(None, &[], &[" FIRE"]),
            &ctx(None, false, "Firefox")
        ));
        let mut no_detection = ctx(None, true, "");
        no_detection.game_mode_detection_enabled = false;
        assert!(!matches_condition_set(
            &set(Some("on"), &[], &[]),
            &no_detection
        ));
    }

    #[test]
    fn blacklist_wins_over_whitelist() {
        let rule = LayoutConditionRule {
            enabled_in_auto: true,
            whitelist: Some(set(None, &["us"], &[])),
            blacklist: Some(set(Some("on"), &[], &[])),
        };
        assert!(layout_allowed(Some(&rule), &ctx(Some("us"), false, "")));
        assert!(!layout_allowed(Some(&rule), &ctx(Some("us"), true, "")));
        assert!(!layout_allowed(Some(&rule), &ctx(Some("ru"), false, "")));
    }

    #[test]
    fn picks_first_allowed_layout_in_priority_order() {
        let ids: Vec<String> = ["user:A", "user:B", "user:C"].map(String::from).to_vec();
        let mut settings = AppSettings {
            layout_order: vec!["user:C".into(), "user:B".into()],
            ..AppSettings::default()
        };
        assert_eq!(
            order_layout_ids(&ids, &settings.layout_order),
            vec!["user:C", "user:B", "user:A"]
        );
        for (id, layouts) in [
            ("user:A", vec![]),
            ("user:B", vec!["ru"]),
            ("user:C", vec!["de"]),
        ] {
            settings.layout_conditions.insert(
                id.into(),
                LayoutConditionRule {
                    enabled_in_auto: true,
                    whitelist: (!layouts.is_empty()).then(|| set(None, &layouts, &[])),
                    blacklist: None,
                },
            );
        }
        assert_eq!(
            pick_active_layout(&ids, &settings, &ctx(Some("ru"), false, "")).as_deref(),
            Some("user:B")
        );
        assert_eq!(
            pick_active_layout(&ids, &settings, &ctx(Some("us"), false, "")).as_deref(),
            Some("user:A")
        );
        settings
            .layout_conditions
            .get_mut("user:A")
            .unwrap()
            .enabled_in_auto = false;
        assert_eq!(
            pick_active_layout(&ids, &settings, &ctx(Some("us"), false, "")),
            None
        );
    }
}
