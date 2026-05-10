#![cfg(target_os = "linux")]
#![allow(clippy::too_many_arguments)]

use super::action::{explicit_text, parse_action};
use super::config::{ActionSpec, AppConfig};
use super::keys::code_to_key;
use super::system;
use super::system_macros::SYSTEM_MACROS;
use std::collections::{HashMap, HashSet};

pub fn validate_config(cfg: &AppConfig) -> Result<(), String> {
    let mut errors = Vec::new();
    let system_macro_ids: HashSet<&str> = SYSTEM_MACROS.iter().map(|m| m.id).collect();

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
        if system_macro_ids.contains(id) {
            errors.push(format!("Macro ID \"{id}\" conflicts with a system macro"));
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
        if c.linux.trim().is_empty() {
            errors.push(format!("Command \"{id}\" has an empty Linux script"));
        }
        command_ids.insert(id.to_string());
    }

    let commands_trusted = cfg.settings.commands_trusted(&cfg.commands);
    let layer_ids: HashSet<&str> = cfg.layer_keymaps.keys().map(String::as_str).collect();

    for r in &cfg.rules {
        let where_key = if r.key.trim().is_empty() {
            "Rule trigger key cannot be empty".to_string()
        } else {
            format!("Rule \"{}\"", r.key)
        };
        if code_to_key(&r.key).is_none() {
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
            &system_macro_ids,
            &command_ids,
            commands_trusted,
            &mut errors,
        );
        validate_action_spec(
            &r.hold_action,
            &format!("{where_key} hold"),
            ActionKind::HoldKeystroke,
            &user_macro_ids,
            &system_macro_ids,
            &command_ids,
            commands_trusted,
            &mut errors,
        );
        validate_optional_action(
            &r.double_tap_action,
            &format!("{where_key} double-tap"),
            &user_macro_ids,
            &system_macro_ids,
            &command_ids,
            commands_trusted,
            &mut errors,
        );
    }

    for (layer_id, km) in &cfg.layer_keymaps {
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
                    &system_macro_ids,
                    &command_ids,
                    commands_trusted,
                    &mut errors,
                );
            }
        }
        for extra in &km.extras {
            if code_to_key(&extra.key).is_none() {
                errors.push(format!(
                    "Keymap \"{layer_id}\" extra: unknown key \"{}\"",
                    extra.key
                ));
            }
            if let crate::mapper::config::ActionSpec::Action(action) = &extra.action {
                validate_action(
                    action,
                    &format!("Keymap \"{layer_id}\" extra \"{}\"", extra.key),
                    ActionKind::Any,
                    &user_macro_ids,
                    &system_macro_ids,
                    &command_ids,
                    commands_trusted,
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
                &system_macro_ids,
                &command_ids,
                commands_trusted,
                &mut errors,
            );
        }
    }
    validate_macro_cycles(cfg, &mut errors);

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
    system_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,
    commands_trusted: bool,
    errors: &mut Vec<String>,
) {
    if let ActionSpec::Action(action) = spec {
        validate_action(
            action,
            where_,
            kind,
            user_macro_ids,
            system_macro_ids,
            command_ids,
            commands_trusted,
            errors,
        );
    }
}

fn validate_optional_action(
    action: &str,
    where_: &str,
    user_macro_ids: &HashSet<String>,
    system_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,
    commands_trusted: bool,
    errors: &mut Vec<String>,
) {
    if !action.trim().is_empty() {
        validate_action(
            action,
            where_,
            ActionKind::Any,
            user_macro_ids,
            system_macro_ids,
            command_ids,
            commands_trusted,
            errors,
        );
    }
}

fn validate_macro_step(
    action: &str,
    where_: &str,
    user_macro_ids: &HashSet<String>,
    system_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,
    commands_trusted: bool,
    errors: &mut Vec<String>,
) {
    let action = action.trim();
    if action.is_empty() {
        return;
    }
    validate_action(
        action,
        where_,
        ActionKind::Any,
        user_macro_ids,
        system_macro_ids,
        command_ids,
        commands_trusted,
        errors,
    );
}

fn validate_action(
    action: &str,
    where_: &str,
    kind: ActionKind,
    user_macro_ids: &HashSet<String>,
    system_macro_ids: &HashSet<&str>,
    command_ids: &HashSet<String>,
    commands_trusted: bool,
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
    if let Some(id) = action.strip_prefix("macro:") {
        let id = id.trim();
        if !user_macro_ids.contains(id) && !system_macro_ids.contains(id) {
            errors.push(format!("{where_}: unknown macro \"{id}\""));
        }
        return;
    }
    if let Some(id) = action.strip_prefix("cmd:") {
        let id = id.trim();
        if !command_ids.contains(id) {
            errors.push(format!("{where_}: unknown command \"{id}\""));
        } else if !commands_trusted {
            errors.push(format!(
                "{where_}: command \"{id}\" is not approved for this layout"
            ));
        }
        return;
    }
    if let Some(id) = action.strip_prefix("sys:") {
        let id = id.trim();
        if !system::is_known(id) {
            errors.push(format!("{where_}: unknown system action \"{id}\""));
        }
        return;
    }
    if let Some(id) = action.strip_prefix("app:") {
        let id = id.trim();
        if !system::is_known_app(id) {
            errors.push(format!("{where_}: unknown app action \"{id}\""));
        }
        return;
    }
    if explicit_text(action).is_some() {
        return;
    }
    if parse_action(action).is_none() {
        errors.push(format!("{where_}: unknown action \"{action}\""));
    }
}

fn validate_macro_cycles(cfg: &AppConfig, errors: &mut Vec<String>) {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for sys in SYSTEM_MACROS {
        graph.insert(
            sys.id.to_string(),
            sys.steps
                .iter()
                .filter_map(|step| step.trim().strip_prefix("macro:"))
                .map(|id| id.trim().to_string())
                .collect(),
        );
    }

    for m in &cfg.macros {
        graph.insert(
            m.id.trim().to_string(),
            m.steps
                .iter()
                .filter_map(|step| step.action.trim().strip_prefix("macro:"))
                .map(|id| id.trim().to_string())
                .collect(),
        );
    }

    for m in &cfg.macros {
        let source = m.id.trim();
        if source.is_empty() {
            continue;
        }
        for target in graph.get(source).into_iter().flatten() {
            let mut visited = HashSet::new();
            if macro_path_reaches(source, target, &graph, &mut visited) {
                errors.push(format!(
                    "Macro \"{source}\": macro references cannot form a cycle"
                ));
                break;
            }
        }
    }
}

fn macro_path_reaches(
    source: &str,
    current: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
) -> bool {
    if current == source {
        return true;
    }
    if !visited.insert(current.to_string()) {
        return false;
    }
    for next in graph.get(current).into_iter().flatten() {
        if macro_path_reaches(source, next, graph, visited) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapper::config::{Command, CommandTrustEntry, Macro, MacroStep, Rule, Settings};
    use std::collections::HashMap;

    fn empty_cfg() -> AppConfig {
        AppConfig {
            rules: Vec::new(),
            layer_keymaps: HashMap::new(),
            macros: Vec::new(),
            commands: Vec::new(),
            settings: Settings::default(),
        }
    }

    #[test]
    fn rejects_unknown_macro_reference() {
        let mut cfg = empty_cfg();
        cfg.rules.push(Rule {
            enabled: true,
            condition_game_mode: None,
            condition_layouts: None,
            condition_apps_whitelist: None,
            condition_apps_blacklist: None,
            key: "CapsLock".into(),
            layer_id: String::new(),
            tap_action: ActionSpec::Action("macro:missing".into()),
            hold_action: ActionSpec::Native,
            isolate: String::new(),
            hold_timeout_ms: None,
            double_tap_action: String::new(),
            double_tap_timeout_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("unknown macro \"missing\""));
    }

    #[test]
    fn rejects_unapproved_command_reference() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "music".into(),
            linux: "playerctl play-pause".into(),
        });
        cfg.macros.push(Macro {
            id: "m".into(),
            steps: vec![MacroStep {
                action: "cmd:music".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("command \"music\" is not approved"));
    }

    #[test]
    fn accepts_approved_command_reference() {
        let mut cfg = empty_cfg();
        cfg.commands.push(Command {
            id: "music".into(),
            linux: "playerctl play-pause".into(),
        });
        cfg.settings.current_layout_id = Some("custom".into());
        cfg.settings.command_trust.insert(
            "custom".into(),
            CommandTrustEntry {
                fingerprint: "3823d099".into(),
            },
        );
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
    fn rejects_system_macro_shadowing() {
        let mut cfg = empty_cfg();
        cfg.macros.push(Macro {
            id: "copyLine".into(),
            steps: vec![MacroStep {
                action: "macro:duplicateLine".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });

        let err = validate_config(&cfg).expect_err("validation should fail");
        assert!(err.contains("conflicts with a system macro"));
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
                    action: "macro:duplicateLine".into(),
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
}
