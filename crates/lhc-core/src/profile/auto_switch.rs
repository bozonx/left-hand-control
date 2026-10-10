//! Choosing the active layout in auto mode: an ordered list of rules
//! (conditions → layout or off), first match wins, otherwise the default.

use super::model::{AppSettings, AutoRule, LayoutConditionSet, LayoutMode};

/// Observed system state the layout conditions are evaluated against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoSwitchContext {
    /// Short id of the system keyboard layout; `None` when undetectable.
    pub system_layout: Option<String>,
    /// Effective game-mode state; undetectable counts as off.
    pub game_mode_active: bool,
    pub window_title: Option<String>,
    pub window_app_id: Option<String>,
}

impl AutoSwitchContext {
    /// Snapshot of the state cached by the core watchers.
    pub fn current() -> Self {
        let window = crate::runtime_state::active_window();
        Self {
            system_layout: crate::runtime_state::layout_short(),
            game_mode_active: crate::runtime_state::game_mode_active(),
            window_title: window.as_ref().map(|window| window.title.clone()),
            window_app_id: window.map(|window| window.app_id),
        }
    }
}

pub fn matches_condition_set(set: &LayoutConditionSet, ctx: &AutoSwitchContext) -> bool {
    match set.game_mode.as_deref() {
        Some("on") if !ctx.game_mode_active => return false,
        Some("off") if ctx.game_mode_active => return false,
        _ => {}
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
        .find(|(_, rule)| {
            rule.enabled && usable(&rule.layout_id) && matches_condition_set(&rule.conditions, ctx)
        })
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

/// Whether `outer` matches whenever `inner` does, so a rule with `inner`
/// placed after `outer` can never apply.
pub fn covers(outer: &LayoutConditionSet, inner: &LayoutConditionSet) -> bool {
    let game = outer.game_mode.is_none() || outer.game_mode == inner.game_mode;
    let layouts = outer.layouts.is_empty()
        || (!inner.layouts.is_empty()
            && inner
                .layouts
                .iter()
                .all(|item| outer.layouts.contains(item)));
    // Apps match by substring: a window containing `inner`'s needle also
    // contains every needle that is a substring of it.
    let needles: Vec<String> = outer
        .apps
        .iter()
        .map(|needle| needle.trim().to_lowercase())
        .filter(|needle| !needle.is_empty())
        .collect();
    let apps = needles.is_empty()
        || (!inner.apps.is_empty()
            && inner.apps.iter().all(|app| {
                let app = app.trim().to_lowercase();
                needles.iter().any(|needle| app.contains(needle.as_str()))
            }));
    game && layouts && apps
}

/// Index of the first enabled rule before `index` that always matches
/// when rule `index` would, so rule `index` never applies.
pub fn shadowed_by(rules: &[AutoRule], index: usize) -> Option<usize> {
    let rule = rules.get(index)?;
    rules[..index]
        .iter()
        .position(|earlier| earlier.enabled && covers(&earlier.conditions, &rule.conditions))
}

/// Switch the layout mode. Entering auto mode for the first time, with no
/// rules and no default, keeps the single-mode layout as the default so
/// nothing changes until rules are added.
pub fn set_mode(settings: &mut AppSettings, mode: LayoutMode) {
    if mode == LayoutMode::Auto
        && settings.auto_rules.is_empty()
        && settings.auto_default_layout_id.is_none()
    {
        settings.auto_default_layout_id = settings.manual_active_layout_id.clone();
    }
    settings.layout_mode = mode;
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
        // Undetectable game mode is reported as off.
        assert!(matches_condition_set(
            &set(Some("off"), &[], &[]),
            &ctx(None, false, "")
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
            enabled: true,
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

    #[test]
    fn disabled_rules_never_match() {
        let ids = vec!["user:A".to_string()];
        let settings = AppSettings {
            auto_rules: vec![AutoRule {
                id: "a".into(),
                enabled: false,
                layout_id: None,
                conditions: LayoutConditionSet::default(),
            }],
            auto_default_layout_id: Some("user:A".into()),
            ..AppSettings::default()
        };
        assert_eq!(
            choose(&ids, &settings, &ctx(None, false, ""))
                .layout_id
                .as_deref(),
            Some("user:A")
        );
    }

    #[test]
    fn earlier_broader_rules_shadow_later_ones() {
        let rule = |enabled, conditions| AutoRule {
            id: String::new(),
            enabled,
            layout_id: None,
            conditions,
        };
        let rules = [
            rule(true, set(None, &["ru", "us"], &[])),
            rule(true, set(Some("on"), &["ru"], &["Kate"])),
            rule(true, set(None, &[], &["kate"])),
            rule(true, set(None, &["de"], &["kate-editor"])),
            rule(false, set(None, &[], &[])),
            rule(true, set(Some("off"), &[], &[])),
        ];
        assert_eq!(shadowed_by(&rules, 0), None);
        assert_eq!(shadowed_by(&rules, 1), Some(0));
        assert_eq!(shadowed_by(&rules, 2), None);
        assert_eq!(shadowed_by(&rules, 3), Some(2));
        // A disabled catch-all shadows nothing.
        assert_eq!(shadowed_by(&rules, 5), None);
        assert!(!covers(&set(None, &["ru"], &[]), &set(None, &[], &[])));
    }

    #[test]
    fn first_switch_to_auto_keeps_the_single_layout() {
        let mut settings = AppSettings {
            manual_active_layout_id: Some("user:A".into()),
            ..AppSettings::default()
        };
        set_mode(&mut settings, LayoutMode::Auto);
        assert_eq!(settings.auto_default_layout_id.as_deref(), Some("user:A"));
        // A deliberate "off" default with rules is left alone.
        settings.auto_default_layout_id = None;
        settings.auto_rules.push(AutoRule {
            id: "r".into(),
            enabled: true,
            layout_id: Some("user:A".into()),
            conditions: LayoutConditionSet::default(),
        });
        set_mode(&mut settings, LayoutMode::Manual);
        set_mode(&mut settings, LayoutMode::Auto);
        assert_eq!(settings.auto_default_layout_id, None);
        assert_eq!(settings.layout_mode, LayoutMode::Auto);
    }
}
