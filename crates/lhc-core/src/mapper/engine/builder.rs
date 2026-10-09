use super::Engine;
use super::model::{ActionDef, DecisionMode, HoldMode, MacroDef, RuleEntry, TapMode};
use crate::mapper::action::{MacroStepItem, explicit_pause, explicit_text, parse_action};
use crate::mapper::config::{ActionSpec, AppConfig};
use crate::mapper::keys::code_to_key;
use crate::mapper::system::{self, SysCommand};
use crate::mapper::system_macros::SYSTEM_MACROS;
use evdev::Key;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

fn parse_key_list(raw: &str) -> Vec<Key> {
    raw.split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .filter_map(|code| {
            let key = code_to_key(code);
            if key.is_none() {
                log::debug!("[mapper] unknown key code in list: {code}");
            }
            key
        })
        .collect()
}

impl Engine {
    pub fn new(cfg: &AppConfig) -> Self {
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_double_tap =
            Duration::from_millis(cfg.settings.default_double_tap_timeout_ms.max(1));
        let default_step_pause = Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay = Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

        // Build the macro table first so tap / keymap actions can reference
        // macros by id. System macros are seeded first; user macros with the
        // same id override them.
        let mut macros: HashMap<String, MacroDef> = HashMap::new();
        let mut commands: HashMap<String, SysCommand> = HashMap::new();

        if cfg.settings.commands_enabled {
            for c in &cfg.commands {
                let id = c.id.trim();
                let linux = c.linux.trim();
                if id.is_empty() {
                    log::debug!("[mapper] skipping command with empty id");
                    continue;
                }
                if linux.is_empty() {
                    log::debug!("[mapper] command {} has empty linux string — skipped", id);
                    continue;
                }
                commands.insert(
                    id.to_string(),
                    SysCommand {
                        program: "sh".into(),
                        args: vec!["-lc".into(), linux.to_string()],
                        working_directory: c.working_directory.clone(),
                    },
                );
            }
        } else {
            log::debug!("[mapper] shell commands are disabled for this layout");
        }

        let mut raw_user_macros: HashMap<String, &crate::mapper::config::Macro> = HashMap::new();
        for m in &cfg.macros {
            if m.id.is_empty() {
                log::debug!("[mapper] skipping macro with empty id");
                continue;
            }
            raw_user_macros.insert(m.id.clone(), m);
        }

        fn resolve_macro_steps(
            id: &str,
            raw_steps: Vec<&str>,
            raw_user_macros: &HashMap<String, &crate::mapper::config::Macro>,
            commands: &HashMap<String, SysCommand>,
            depth: usize,
        ) -> Vec<MacroStepItem> {
            let mut steps: Vec<MacroStepItem> = Vec::new();
            if depth > 10 {
                log::debug!("[mapper] macro {} exceeded max nesting depth", id);
                return steps;
            }
            for raw in raw_steps {
                let raw = raw.trim();
                if raw.is_empty() {
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("sys:") {
                    match system::resolve(rest.trim()) {
                        Some(cmd) => steps.push(MacroStepItem::System(cmd)),
                        None => log::debug!(
                            "[mapper] macro {} step: system fn {:?} not available",
                            id,
                            rest.trim()
                        ),
                    }
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("app:") {
                    match system::resolve_app(rest.trim()) {
                        Some(cmd) => steps.push(MacroStepItem::System(cmd)),
                        None => log::debug!(
                            "[mapper] macro {} step: app action {:?} not available",
                            id,
                            rest.trim()
                        ),
                    }
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("macro:") {
                    let nested_id = rest.trim();
                    let mut found = false;
                    if let Some(sys_m) = SYSTEM_MACROS.iter().find(|s| s.id == nested_id) {
                        let sys_steps: Vec<&str> = sys_m.steps.to_vec();
                        let nested_steps = resolve_macro_steps(
                            nested_id,
                            sys_steps,
                            raw_user_macros,
                            commands,
                            depth + 1,
                        );
                        steps.extend(nested_steps);
                        found = true;
                    } else if let Some(user_m) = raw_user_macros.get(nested_id) {
                        let nested_steps = resolve_macro_steps(
                            nested_id,
                            user_m.steps.iter().map(|s| s.action.as_str()).collect(),
                            raw_user_macros,
                            commands,
                            depth + 1,
                        );
                        steps.extend(nested_steps);
                        found = true;
                    }
                    if !found {
                        log::debug!(
                            "[mapper] macro {}: unknown nested macro ref {:?}",
                            id,
                            nested_id
                        );
                    }
                    continue;
                }
                if let Some(rest) = raw.strip_prefix("cmd:") {
                    let cmd_id = rest.trim();
                    match commands.get(cmd_id) {
                        Some(cmd) => steps.push(MacroStepItem::Command(cmd.clone())),
                        None => log::debug!(
                            "[mapper] macro {} step: unknown command ref {:?}",
                            id,
                            cmd_id
                        ),
                    }
                    continue;
                }
                if let Some(text) = explicit_text(raw) {
                    steps.push(MacroStepItem::Literal(text));
                    continue;
                }
                if let Some(duration) = explicit_pause(raw) {
                    steps.push(MacroStepItem::Pause(duration));
                    continue;
                }
                match parse_action(raw) {
                    Some(ks) => steps.push(MacroStepItem::Stroke(ks)),
                    None => log::debug!("[mapper] macro {} step: unknown keystroke {:?}", id, raw),
                }
            }
            steps
        }

        for sys in SYSTEM_MACROS {
            let steps =
                resolve_macro_steps(sys.id, sys.steps.to_vec(), &raw_user_macros, &commands, 0);
            if steps.is_empty() {
                log::debug!(
                    "[mapper] system macro {} has no usable steps — skipped",
                    sys.id
                );
                continue;
            }
            macros.insert(
                sys.id.to_string(),
                MacroDef {
                    steps,
                    step_pause: default_step_pause,
                    mod_delay: default_mod_delay,
                },
            );
        }

        for m in &cfg.macros {
            if m.id.is_empty() {
                continue;
            }
            let raw_steps: Vec<&str> = m.steps.iter().map(|s| s.action.as_str()).collect();
            let steps = resolve_macro_steps(&m.id, raw_steps, &raw_user_macros, &commands, 0);
            if steps.is_empty() {
                log::debug!("[mapper] macro {} has no usable steps — skipped", m.id);
                continue;
            }
            let step_pause = m
                .step_pause_ms
                .map(Duration::from_millis)
                .unwrap_or(default_step_pause);
            let mod_delay = m
                .modifier_delay_ms
                .map(Duration::from_millis)
                .unwrap_or(default_mod_delay);
            macros.insert(
                m.id.clone(),
                MacroDef {
                    steps,
                    step_pause,
                    mod_delay,
                },
            );
        }

        let resolve = |action: &str, where_: &str| -> Option<ActionDef> {
            let trimmed = action.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Some(rest) = trimmed.strip_prefix("macro:") {
                let id = rest.trim();
                if let Some(md) = macros.get(id) {
                    return Some(ActionDef::Macro(md.clone()));
                }
                log::debug!("[mapper] unknown macro ref {:?} ({})", trimmed, where_);
                return None;
            }
            if let Some(rest) = trimmed.strip_prefix("cmd:") {
                let id = rest.trim();
                if let Some(cmd) = commands.get(id) {
                    return Some(ActionDef::Command(cmd.clone()));
                }
                return Some(ActionDef::Swallow);
            }
            if let Some(rest) = trimmed.strip_prefix("sys:") {
                let name = rest.trim();
                match system::resolve(name) {
                    Some(cmd) => return Some(ActionDef::System(cmd)),
                    None => {
                        log::debug!(
                            "[mapper] system fn {:?} not available on this OS/DE ({})",
                            name,
                            where_
                        );
                        return None;
                    }
                }
            }
            if let Some(rest) = trimmed.strip_prefix("app:") {
                let name = rest.trim();
                match system::resolve_app(name) {
                    Some(cmd) => return Some(ActionDef::System(cmd)),
                    None => {
                        log::debug!("[mapper] app action {:?} not available ({})", name, where_);
                        return None;
                    }
                }
            }
            if let Some(text) = explicit_text(trimmed) {
                return Some(ActionDef::Literal(text));
            }
            match parse_action(trimmed) {
                Some(ks) => Some(ActionDef::Stroke(ks)),
                None => {
                    log::debug!("[mapper] unknown action {:?} ({})", trimmed, where_);
                    None
                }
            }
        };

        let mut rules = HashMap::new();
        let mut chord_rules = Vec::new();
        for r in &cfg.rules {
            if !r.enabled {
                continue;
            }

            let Some(keys) = crate::profile::key_catalog::trigger_keys(&r.key).and_then(|keys| {
                keys.into_iter()
                    .map(code_to_key)
                    .collect::<Option<Vec<_>>>()
            }) else {
                continue;
            };
            let key = keys[0];
            use crate::profile::model::HoldBehavior;
            let behavior = r.hold_behavior.unwrap_or({
                if !r.long_hold_action.is_empty() {
                    HoldBehavior::Action
                } else if !r.layer_id.is_empty() {
                    HoldBehavior::Layer
                } else {
                    HoldBehavior::None
                }
            });
            let long_hold = if behavior == HoldBehavior::Action {
                resolve(&r.long_hold_action, &format!("long hold for {}", r.key))
            } else {
                None
            };

            // Disallow left/right/middle mouse buttons as triggers —
            // remapping them would make the system unusable.
            if matches!(key, Key::BTN_LEFT | Key::BTN_RIGHT | Key::BTN_MIDDLE) {
                log::debug!(
                    "[mapper] rule key {:?}: mouse buttons 1-3 cannot be used as triggers — skipped",
                    r.key
                );
                continue;
            }

            let tap = match &r.tap_action {
                ActionSpec::Native if keys.len() > 1 => TapMode::NativeChord(keys.clone()),
                ActionSpec::Native => TapMode::Native,
                ActionSpec::Swallow => TapMode::Swallow,
                ActionSpec::Action(s) => match resolve(s, &format!("tap for {}", r.key)) {
                    Some(a) => TapMode::Action(a),
                    None => TapMode::Swallow,
                },
            };

            let layer_id = if r.layer_id.is_empty() || behavior != HoldBehavior::Layer {
                None
            } else {
                Some(r.layer_id.clone())
            };
            let hold = match &r.hold_action {
                _ if behavior == HoldBehavior::Action => HoldMode::Swallow,
                ActionSpec::Native if layer_id.is_some() || long_hold.is_some() => {
                    HoldMode::Swallow
                }
                ActionSpec::Native if keys.len() > 1 => {
                    HoldMode::Keystroke(crate::mapper::action::Keystroke {
                        mods: keys[..keys.len() - 1].to_vec(),
                        key: keys[keys.len() - 1],
                    })
                }
                ActionSpec::Native => HoldMode::Native,
                ActionSpec::Swallow => HoldMode::Swallow,
                ActionSpec::Action(s) => match parse_action(s) {
                    Some(ks) => HoldMode::Keystroke(ks),
                    None => {
                        log::debug!(
                            "[mapper] rule {:?}: unknown hold keystroke {:?} — falling back to native hold",
                            r.key,
                            s
                        );
                        HoldMode::Native
                    }
                },
            };

            let double_tap = match (keys.len(), r.double_tap_action.as_str()) {
                (1, "") => None,
                (1, s) => resolve(s, &format!("double-tap for {}", r.key)),
                _ => None,
            };

            // Fully transparent rule — skip registration so the key passes
            // straight through the kernel → uinput grab.
            if layer_id.is_none()
                && matches!(tap, TapMode::Native | TapMode::NativeChord(_))
                && (matches!(hold, HoldMode::Native)
                    || (keys.len() > 1 && matches!(r.hold_action, ActionSpec::Native)))
                && double_tap.is_none()
                && long_hold.is_none()
            {
                log::debug!(
                    "[mapper] rule {:?}: tap=native, hold=native, no double-tap — skipped (passthrough)",
                    r.key
                );
                continue;
            }

            let long_hold_timeout = Duration::from_millis(
                r.long_hold_timeout_ms
                    .unwrap_or(cfg.settings.default_long_hold_timeout_ms)
                    .clamp(1, 86_400_000),
            );
            let hold_timeout = r
                .hold_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_hold);
            let double_tap_window = r
                .double_tap_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_double_tap);
            let mut isolate_keys = parse_key_list(&r.isolate);
            if isolate_keys.is_empty() {
                if let Some(layer_id) = &layer_id {
                    if let Some(km) = cfg.layer_keymaps.get(layer_id) {
                        isolate_keys = km
                            .isolate
                            .iter()
                            .filter_map(|code| code_to_key(code))
                            .collect();
                    }
                }
            }
            let mut whitelist_keys = parse_key_list(&r.hold_for);
            if whitelist_keys.is_empty() {
                if let Some(layer_id) = &layer_id {
                    if let Some(km) = cfg.layer_keymaps.get(layer_id) {
                        whitelist_keys = km
                            .hold_for
                            .iter()
                            .filter_map(|code| code_to_key(code))
                            .collect();
                    }
                }
            }

            let entry = RuleEntry {
                trigger_keys: keys.clone(),
                long_hold,
                long_hold_timeout,
                tap,
                layer_id,
                hold,
                isolate_keys,
                whitelist_keys,
                double_tap,
                hold_timeout: if behavior == HoldBehavior::Action {
                    long_hold_timeout
                } else {
                    hold_timeout
                },
                double_tap_window,
                condition_game_mode: r.condition_game_mode.clone(),
                condition_layouts: r.condition_layouts.clone(),
                condition_apps_whitelist: r.condition_apps_whitelist.clone(),
                condition_apps_blacklist: r.condition_apps_blacklist.clone(),
            };
            if keys.len() > 1 {
                chord_rules.push((keys, entry));
            } else {
                rules.entry(key).or_insert_with(Vec::new).push(entry);
            }
        }

        let mut layer_maps: HashMap<String, HashMap<Key, ActionDef>> = HashMap::new();
        for (layer_id, km) in &cfg.layer_keymaps {
            let mut m = HashMap::new();
            for (code, action) in &km.keys {
                let Some(key) = code_to_key(code) else {
                    log::debug!("[mapper] unknown key code in keymap {layer_id}: {code}");
                    continue;
                };
                let Some(def) = action
                    .as_deref()
                    .map(|s| resolve(s, &format!("keymap {layer_id}.{code}")))
                    .unwrap_or(Some(ActionDef::Swallow))
                else {
                    continue;
                };
                m.insert(key, def);
            }

            layer_maps.insert(layer_id.clone(), m);
        }

        Self {
            rules,
            chord_rules,
            physical_down: HashSet::new(),
            active_chords: HashMap::new(),
            chord_suppressed: HashMap::new(),
            chord_consumed: HashSet::new(),
            long_holds: HashMap::new(),
            macros,
            commands,
            layer_maps,
            default_hold,
            default_mod_delay,
            decision_mode: match cfg.settings.tap_decision {
                crate::mapper::config::TapDecision::PermissiveHold => DecisionMode::PermissiveHold,
                crate::mapper::config::TapDecision::HoldOnOtherKeyPress => {
                    DecisionMode::HoldOnOtherKeyPress
                }
            },
            decision_buffer: Vec::new(),
            active_layers: Vec::new(),
            pending: HashMap::new(),
            emitted: HashMap::new(),
            oneshot_consumed: HashSet::new(),
            mod_refs: HashMap::new(),
            layer_triggers: HashMap::new(),
            isolated_holds: HashMap::new(),
            active_macro: None,
        }
    }
}
