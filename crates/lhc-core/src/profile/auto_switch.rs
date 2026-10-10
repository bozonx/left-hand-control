//! Choosing the active layout in auto mode: an ordered list of rules
//! (conditions → layout or off), first match wins, otherwise the default.

use super::model::{AppSettings, LayoutConditionSet};

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

/// Outcome of the automatic choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoChoice {
    /// Index of the matching rule; `None` means the default applied.
    pub rule: Option<usize>,
    /// Layout to use; `None` means off (native passthrough).
    pub layout_id: Option<String>,
}

/// First rule whose conditions match `ctx`, or the default. Rules and the
/// default pointing at a layout missing from `available` are skipped.
pub fn choose(available: &[String], settings: &AppSettings, ctx: &AutoSwitchContext) -> AutoChoice {
    let usable = |id: &Option<String>| id.as_ref().is_none_or(|id| available.contains(id));
    settings
        .auto_rules
        .iter()
        .enumerate()
        .find(|(_, rule)| usable(&rule.layout_id) && matches_condition_set(&rule.conditions, ctx))
        .map(|(index, rule)| AutoChoice {
            rule: Some(index),
            layout_id: rule.layout_id.clone(),
        })
        .unwrap_or_else(|| AutoChoice {
            rule: None,
            layout_id: settings
                .auto_default_layout_id
                .clone()
                .filter(|id| available.contains(id)),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::model::AutoRule;

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
    fn library_order_keeps_unknown_ids_last() {
        let ids: Vec<String> = ["user:A", "user:B", "user:C"].map(String::from).to_vec();
        let order = ["user:C".to_string(), "user:B".to_string()];
        assert_eq!(
            order_layout_ids(&ids, &order),
            ["user:C", "user:B", "user:A"]
        );
    }

    #[test]
    fn first_matching_rule_wins_then_default() {
        let ids: Vec<String> = ["user:A", "user:B"].map(String::from).to_vec();
        let rule = |id: &str, layout: Option<&str>, conditions| AutoRule {
            id: id.into(),
            layout_id: layout.map(str::to_owned),
            conditions,
        };
        let mut settings = AppSettings {
            auto_rules: vec![
                rule("off", None, set(None, &[], &["blender"])),
                rule("ru", Some("user:B"), set(None, &["ru"], &[])),
                rule("gone", Some("user:X"), set(None, &[], &[])),
            ],
            auto_default_layout_id: Some("user:A".into()),
            ..AppSettings::default()
        };
        let pick = |settings: &AppSettings, layout, title| {
            choose(&ids, settings, &ctx(Some(layout), false, title))
        };
        assert_eq!(
            pick(&settings, "ru", "Blender"),
            AutoChoice {
                rule: Some(0),
                layout_id: None
            }
        );
        assert_eq!(
            pick(&settings, "ru", "Kate"),
            AutoChoice {
                rule: Some(1),
                layout_id: Some("user:B".into())
            }
        );
        assert_eq!(
            pick(&settings, "us", "Kate"),
            AutoChoice {
                rule: None,
                layout_id: Some("user:A".into())
            }
        );
        settings.auto_default_layout_id = Some("user:X".into());
        assert_eq!(pick(&settings, "us", "Kate"), AutoChoice::default());
    }
}
