//! Validation of a mapper configuration before it reaches the engine: actions,
//! macro sizes and cycles.

#![allow(clippy::too_many_arguments)]

use super::action::{explicit_pause, parse_action};
use super::builtin_macros::BUILTIN_MACROS;
use super::config::{ActionSpec, MapperConfig};
use super::keys::code_to_key;
use super::system;
use crate::profile::actions::{Action, MAX_MACRO_DEPTH, MAX_MACRO_STEPS};
use crate::profile::macro_graph::{MacroGraph, MacroRef};
use std::collections::HashSet;

pub fn validate_config(cfg: &MapperConfig) -> Result<(), String> {
    let mut errors = Vec::new();
    let builtin_macro_ids: HashSet<&str> = BUILTIN_MACROS.iter().map(|m| m.id).collect();

    let mut user_macro_ids = HashSet::new();
    let mut seen_macro_ids = HashSet::new();
    for m in &cfg.macros {
        let id = m.id.trim();
        if id.is_empty() {
            errors.push("Macro ID cannot be empty".to_string());
            continue;
        }
        if !seen_macro_ids.insert(id.to_string()) {
            errors.push(format!("Duplicate macro ID \"{id}\""));
        }
        user_macro_ids.insert(id.to_string());
    }

    let mut command_ids = HashSet::new();
    let mut seen_command_ids = HashSet::new();
    for c in &cfg.commands {
        let id = c.id.trim();
        if id.is_empty() {
            errors.push("Command ID cannot be empty".to_string());
            continue;
        }
        if !seen_command_ids.insert(id.to_string()) {
            errors.push(format!("Duplicate command ID \"{id}\""));
        }
        command_ids.insert(id.to_string());
    }

    let layer_ids: HashSet<&str> = cfg.layer_keymaps.keys().map(String::as_str).collect();

    for r in &cfg.rules {
        let where_key = if r.key.trim().is_empty() {
            "Rule trigger key cannot be empty".to_string()
        } else {
            format!("Rule \"{}\"", r.key)
        };
        if !crate::profile::key_catalog::valid_trigger(&r.key) {
            errors.push(format!("{where_key}: unknown physical key \"{}\"", r.key));
        }
        if !r.layer_id.is_empty() && !layer_ids.contains(r.layer_id.as_str()) {
            errors.push(format!("{where_key}: unknown layer \"{}\"", r.layer_id));
        }
        validate_action_spec(
            &r.tap_action,
            &format!("{where_key} tap"),
            ActionKind::Any,
            &user_macro_ids,
            &builtin_macro_ids,
            &command_ids,
            &mut errors,
        );
        validate_action_spec(
            &r.hold_action,
            &format!("{where_key} hold"),
            ActionKind::HoldKeystroke,
            &user_macro_ids,
            &builtin_macro_ids,
            &command_ids,
            &mut errors,
        );
        validate_optional_action(
            &r.long_hold_action,
            &format!("{where_key} long hold"),
            &user_macro_ids,
            &builtin_macro_ids,
            &command_ids,
            &mut errors,
        );
        validate_optional_action(
            &r.double_tap_action,
            &format!("{where_key} double-tap"),
            &user_macro_ids,
            &builtin_macro_ids,
            &command_ids,
            &mut errors,
        );
    }

    for (layer_id, km) in &cfg.layer_keymaps {
        for code in km
            .hold_modifiers
            .iter()
            .filter(|code| code_to_key(code).is_none())
        {
            errors.push(format!(
                "Keymap \"{layer_id}\" holdModifiers: unknown key \"{code}\""
            ));
        }
        for (code, action) in &km.keys {
            if code_to_key(code).is_none() {
                errors.push(format!("Keymap \"{layer_id}\": unknown key \"{code}\""));
            }
            if let Some(action) = action {
                validate_action(
                    action,
                    &format!("Keymap \"{layer_id}\" key \"{code}\""),
                    ActionKind::Any,
                    &user_macro_ids,
                    &builtin_macro_ids,
                    &command_ids,
                    &mut errors,
                );
            }
        }
    }

    for m in &cfg.macros {
        for (idx, step) in m.steps.iter().enumerate() {
            validate_macro_step(
                &step.action,
                &format!("Macro \"{}\" step #{}", m.id, idx + 1),
                &user_macro_ids,
                &builtin_macro_ids,
                &command_ids,
                &mut errors,
            );
        }
    }
    let graph = MacroGraph::new(cfg.macros.iter().map(|item| {
        (
            item.id.as_str(),
            item.steps.iter().map(|step| step.action.as_str()).collect(),
        )
    }));
    validate_macro_cycles(cfg, &graph, &mut errors);
    validate_macro_sizes(cfg, &graph, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Config has {} error(s):\n{}",
            errors.len(),
            errors
                .into_iter()
                .map(|e| format!("- {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }
}

fn validate_macro_sizes(cfg: &MapperConfig, graph: &MacroGraph<'_>, errors: &mut Vec<String>) {
    let issues = graph.expansion_issues();
    for item in &cfg.macros {
        if issues.contains_key(&MacroRef::User(item.id.trim().into())) {
            errors.push(format!(
                "Macro \"{}\": expansion exceeds {MAX_MACRO_STEPS} entries or {MAX_MACRO_DEPTH} nesting levels",
                item.id
            ));
        }
    }
}

#[derive(Clone, Copy)]
enum ActionKind {
    Any,
    HoldKeystroke,
}

fn validate_action_spec(
    spec: &ActionSpec,
    where_: &str,
    kind: ActionKind,
    user_macro_ids: &HashSet<String>,
    builtin_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,

    errors: &mut Vec<String>,
) {
    if let ActionSpec::Action(action) = spec {
        validate_action(
            action,
            where_,
            kind,
            user_macro_ids,
            builtin_macro_ids,
            command_ids,
            errors,
        );
    }
}

fn validate_optional_action(
    action: &str,
    where_: &str,
    user_macro_ids: &HashSet<String>,
    builtin_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,

    errors: &mut Vec<String>,
) {
    if !action.trim().is_empty() {
        validate_action(
            action,
            where_,
            ActionKind::Any,
            user_macro_ids,
            builtin_macro_ids,
            command_ids,
            errors,
        );
    }
}

fn validate_macro_step(
    action: &str,
    where_: &str,
    user_macro_ids: &HashSet<String>,
    builtin_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,

    errors: &mut Vec<String>,
) {
    let action = action.trim();
    if action.is_empty() {
        return;
    }
    if matches!(Action::parse(Some(action)), Action::Pause(_)) {
        if explicit_pause(action).is_none() {
            errors.push(format!("{where_}: invalid pause step \"{action}\""));
        }
        return;
    }
    validate_action(
        action,
        where_,
        ActionKind::Any,
        user_macro_ids,
        builtin_macro_ids,
        command_ids,
        errors,
    );
}

fn validate_action(
    action: &str,
    where_: &str,
    kind: ActionKind,
    user_macro_ids: &HashSet<String>,
    builtin_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,

    errors: &mut Vec<String>,
) {
    let action = action.trim();
    if action.is_empty() {
        return;
    }
    if matches!(kind, ActionKind::HoldKeystroke) {
        if parse_action(action).is_none() {
            errors.push(format!(
                "{where_}: hold action must be a key or chord (got \"{action}\")"
            ));
        }
        return;
    }
    match Action::parse(Some(action)) {
        Action::Native | Action::Swallow | Action::Text(_) => {}
        Action::Macro(id) => {
            if !user_macro_ids.contains(&id) {
                errors.push(format!("{where_}: unknown macro \"{id}\""));
            }
        }
        Action::BuiltinMacro(id) => {
            if !builtin_macro_ids.contains(id.as_str()) {
                errors.push(format!("{where_}: unknown built-in macro \"{id}\""));
            }
        }
        Action::Command(id) => {
            if !command_ids.contains(&id) {
                errors.push(format!("{where_}: unknown command \"{id}\""));
            }
        }
        Action::System(id) => {
            if !system::is_known(&id) {
                errors.push(format!("{where_}: unknown system action \"{id}\""));
            }
        }
        Action::App(id) => {
            if !system::is_known_app(&id) {
                errors.push(format!("{where_}: unknown app action \"{id}\""));
            }
        }
        Action::KdeShortcut(id) => {
            if crate::profile::actions::kde_shortcut_parts(&id).is_none() {
                errors.push(format!(
                    "{where_}: invalid KDE shortcut \"{id}\", expected component/shortcut"
                ));
            }
        }
        Action::Pause(_) => {
            errors.push(format!("{where_}: pause is only allowed inside macros"));
        }
        Action::Unknown(value) => {
            errors.push(format!(
                "{where_}: unknown action type \"{value}\" (it may come from a newer version of the application)"
            ));
        }
        Action::Keys(keys) => {
            if parse_action(&keys).is_none() {
                errors.push(format!("{where_}: unknown action \"{action}\""));
            }
        }
    }
}

fn validate_macro_cycles(cfg: &MapperConfig, graph: &MacroGraph<'_>, errors: &mut Vec<String>) {
    for item in &cfg.macros {
        let id = item.id.trim();
        if !id.is_empty() && graph.has_cycle(&MacroRef::User(id.into())) {
            errors.push(format!(
                "Macro \"macro:{id}\": macro references cannot form a cycle"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::config::{Command, LayerKeymap, Macro, MacroStep, Rule, Settings};
    use std::collections::HashMap;

    fn empty_cfg() -> MapperConfig {
        MapperConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            commands: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn editor_and_mapper_agree_on_macro_cycles_and_missing_references() {
        use crate::profile::{macros, model};
        for steps in [
            ["macro:b", "macro:a"],
            ["macro:a", "KeyA"],
            [" macro: b ", "KeyA"],
            ["macro:missing", "KeyA"],
            ["builtin:copyLineText", "text:macro:a"],
        ] {
            let layout = model::LayoutPreset {
                macros: ["a", "b"]
                    .into_iter()
                    .zip(steps)
                    .map(|(id, action)| model::Macro {
                        id: id.into(),
                        name: id.into(),
                        steps: vec![model::MacroStep {
                            id: "s".into(),
                            action: action.into(),
                        }],
                        step_pause_ms: None,
                        modifier_delay_ms: None,
                    })
                    .collect(),
                ..Default::default()
            };
            let mapper: MapperConfig =
                serde_json::from_value(serde_json::to_value(&layout).unwrap()).unwrap();
            let editor =
                model::ResolvedAppConfig::from_parts(model::AppSettings::default(), layout, None);
            assert_eq!(
                editor
                    .macros
                    .iter()
                    .all(|item| macros::validate(&editor, item).is_ok()),
                validate_config(&mapper).is_ok(),
                "{steps:?}",
            );
        }
    }

    #[test]
    fn mapper_and_editor_accept_the_same_actions() {
        let check = |tap: &str, step: &str| {
            let cfg: MapperConfig = serde_json::from_value(serde_json::json!({
                "macros": [{"id": "m", "steps": [{"action": step}]}],
                "rules": [{"key": "KeyQ", "tapAction": tap}]
            }))
            .unwrap();
            validate_config(&cfg)
        };
        assert!(check("  text:hi", "pause:1").is_ok());
        assert!(
            check("sys:switchDesktop10", "macro:m2").is_err(),
            "unknown macro"
        );
        for (tap, step) in [
            ("sys:switchDesktop11", "KeyA"),
            ("sys:windowToDesktop20", "KeyA"),
            ("KeyA", "pause:0"),
            ("KeyA", "pause:+5"),
            ("pause:10", "KeyA"),
        ] {
            assert!(check(tap, step).is_err(), "{tap} / {step}");
        }
    }

    #[test]
    fn rejects_unknown_macro_reference() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "CapsLock".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("macro:missing".into()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown macro \"missing\""));
    }

    #[test]
    fn rejects_unknown_hold_modifier_keys() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "CapsLock".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Native,
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        cfg.layer_keymaps.insert(
            "nav".into(),
            LayerKeymap {
                keys: HashMap::new(),
                hold_modifiers: vec!["Super".into()],
            },
        );

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("Keymap \"nav\" holdModifiers: unknown key \"Super\""));
    }

    #[test]
    fn accepts_disabled_command_reference_without_enabling_execution() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "music".into(),
            posix_script: "playerctl play-pause".into(),
            powershell_script: "playerctl play-pause".into(),
            working_directory: None,
        });
        cfg.macros.push(Macro {
            id: "m".into(),
            steps: vec![MacroStep {
                action: "cmd:music".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        validate_config(&cfg).expect("disabled commands remain inert");
    }

    #[test]
    fn accepts_enabled_command_reference() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "music".into(),
            posix_script: "playerctl play-pause".into(),
            powershell_script: "playerctl play-pause".into(),
            working_directory: None,
        });
        cfg.settings.editing_layout_id = Some("custom".into());
        cfg.settings.commands_enabled = true;
        cfg.macros.push(Macro {
            id: "m".into(),
            steps: vec![MacroStep {
                action: "cmd:music".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        validate_config(&cfg).expect("validation should pass");
    }

    #[test]
    fn accepts_separate_builtin_and_user_macro_names() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "copyLine".into(),
            steps: vec![MacroStep {
                action: "builtin:copyLineText".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        validate_config(&cfg).expect("same id in separate namespaces is valid");
    }

    #[test]
    fn accepts_nested_macro_steps() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "outer".into(),
            steps: vec![
                MacroStep {
                    action: "macro:inner".into(),
                },
                MacroStep {
                    action: "builtin:duplicateLine".into(),
                },
            ],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        cfg.macros.push(Macro {
            id: "inner".into(),
            steps: vec![MacroStep {
                action: "Ctrl+KeyC".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        validate_config(&cfg).expect("validation should pass");
    }

    #[test]
    fn accepts_pause_macro_step() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "m".into(),
            steps: vec![MacroStep {
                action: "pause:250".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        validate_config(&cfg).expect("validation should pass");
    }

    #[test]
    fn rejects_pause_outside_macro_steps() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "CapsLock".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("pause:250".into()),
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("pause is only allowed inside macros"));
    }

    #[test]
    fn rejects_macro_reference_cycles() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "outer".into(),
            steps: vec![MacroStep {
                action: "macro:inner".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        cfg.macros.push(Macro {
            id: "inner".into(),
            steps: vec![MacroStep {
                action: "macro:outer".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("macro references cannot form a cycle"));
    }

    #[test]
    fn rejects_unknown_rule_key() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "UnknownKey".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Native,
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown physical key \"UnknownKey\""));
    }

    #[test]
    fn rejects_unknown_layer_in_rule() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "CapsLock".into(),
            layer_id: "missing".into(),
            tap_action: ActionSpec::Native,
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Native,
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown layer \"missing\""));
    }

    #[test]
    fn rejects_duplicate_macro_id() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "dup".into(),
            steps: vec![MacroStep {
                action: "KeyA".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        cfg.macros.push(Macro {
            id: "dup".into(),
            steps: vec![MacroStep {
                action: "KeyB".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("Duplicate macro ID \"dup\""));
    }

    #[test]
    fn rejects_duplicate_command_id() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "dup".into(),
            posix_script: "echo 1".into(),
            powershell_script: "echo 1".into(),
            working_directory: None,
        });
        cfg.commands.push(Command {
            id: "dup".into(),
            posix_script: "echo 2".into(),
            powershell_script: "echo 2".into(),
            working_directory: None,
        });
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("Duplicate command ID \"dup\""));
    }

    #[test]
    fn accepts_empty_command_linux_script_as_draft() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "empty".into(),
            posix_script: "   ".into(),
            powershell_script: "   ".into(),
            working_directory: None,
        });
        validate_config(&cfg).expect("empty command scripts are ignored drafts");
    }

    #[test]
    fn rejects_unknown_key_in_layer_keymap() {
        let mut cfg = empty_cfg();
        let mut km = crate::mapper::config::LayerKeymap::default();
        km.keys.insert("BadKey".into(), Some("Escape".into()));
        cfg.layer_keymaps.insert("nav".into(), km);
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown key \"BadKey\""));
    }

    #[test]
    fn rejects_hold_action_that_is_not_keystroke() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_keyboard_layouts: None,
            condition_window_patterns: None,
            condition_excluded_window_patterns: None,
            key: "CapsLock".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Native,
            hold_behavior: None,
            long_hold_action: String::new(),
            long_hold_timeout_ms: None,
            hold_action: ActionSpec::Action("builtin:copyLineText".into()),
            hold_timeout_ms: None,
            tap_decision: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("hold action must be a key or chord"));
    }

    #[test]
    fn accepts_empty_config() {
        validate_config(&empty_cfg()).expect("empty config should pass");
    }

    #[test]
    fn rejects_unknown_action_in_layer_keymap() {
        let mut cfg = empty_cfg();
        let mut km = crate::mapper::config::LayerKeymap::default();
        km.keys.insert("KeyA".into(), Some("BadAction!!!".into()));
        cfg.layer_keymaps.insert("nav".into(), km);
        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown action \"BadAction!!!\""));
    }
}
