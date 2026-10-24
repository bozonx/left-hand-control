use super::Engine;
use super::model::{ActionDef, HoldMode, MacroDef, RuleEntry, TapMode};
use crate::mapper::action::{MacroStepItem, parse_action, pause_duration};
use crate::mapper::builtin_macros::BUILTIN_MACROS;
use crate::mapper::config::{ActionSpec, AppConfig};
use crate::mapper::key_code::KeyCode;
use crate::mapper::keys::code_to_key;
use crate::mapper::system::{self, SysCommand};
use crate::profile::actions::Action;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

impl Engine {
    pub fn new(cfg: &AppConfig) -> Self {
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_double_tap =
            Duration::from_millis(cfg.settings.default_double_tap_timeout_ms.max(1));
        let default_step_pause = Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay = Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

        // Build the macro table first so tap / keymap actions can reference
        // macros by id. Built-in macros are seeded first; user macros with the
        // same id override them.
        let mut macros: HashMap<String, MacroDef> = HashMap::new();
        let mut commands: HashMap<String, SysCommand> = HashMap::new();
        // Commands of the layout without a script for this OS.
        let mut foreign_commands: HashSet<String> = HashSet::new();

        if cfg.settings.commands_enabled {
            for c in &cfg.commands {
                let id = c.id.trim();
                if id.is_empty() {
                    log::debug!("[mapper] skipping command with empty id");
                    continue;
                }
                let Some(command) =
                    crate::mapper::commands::shell_command(c, c.working_directory.clone())
                else {
                    log::debug!("[mapper] command {id} has no script for this OS — skipped");
                    foreign_commands.insert(id.to_string());
                    continue;
                };
                commands.insert(id.to_string(), command);
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
            foreign_commands: &HashSet<String>,
            depth: usize,
            remaining: &mut usize,
        ) -> Vec<MacroStepItem> {
            let mut steps: Vec<MacroStepItem> = Vec::new();
            if depth > 10 {
                log::debug!("[mapper] macro {} exceeded max nesting depth", id);
                return steps;
            }
            for raw in raw_steps {
                if *remaining == 0 {
                    break;
                }
                let raw = raw.trim_start();
                if raw.is_empty() {
                    continue;
                }
                *remaining -= 1;
                let unavailable = || MacroStepItem::Unavailable(raw.trim().into());
                match Action::parse(Some(raw)) {
                    Action::Native | Action::Swallow => {}
                    Action::System(name) => steps.push(
                        system::resolve(&name).map_or_else(unavailable, MacroStepItem::System),
                    ),
                    Action::KdeShortcut(shortcut) => steps.push(
                        system::resolve_kde_shortcut(&shortcut)
                            .map_or_else(unavailable, MacroStepItem::System),
                    ),
                    // Like `sys:`: a step that cannot run here makes the
                    // whole macro unavailable instead of silently shorter.
                    Action::App(name) => steps.push(
                        system::resolve_app(&name).map_or_else(unavailable, MacroStepItem::System),
                    ),
                    Action::Macro(nested_id) => {
                        let nested_raw: Option<Vec<&str>> = BUILTIN_MACROS
                            .iter()
                            .find(|s| s.id == nested_id)
                            .map(|sys_m| sys_m.steps.to_vec())
                            .or_else(|| {
                                raw_user_macros.get(nested_id.as_str()).map(|user_m| {
                                    user_m.steps.iter().map(|s| s.action.as_str()).collect()
                                })
                            });
                        match nested_raw {
                            Some(nested_raw) => steps.extend(resolve_macro_steps(
                                &nested_id,
                                nested_raw,
                                raw_user_macros,
                                commands,
                                foreign_commands,
                                depth + 1,
                                remaining,
                            )),
                            None => log::debug!(
                                "[mapper] macro {}: unknown nested macro ref {:?}",
                                id,
                                nested_id
                            ),
                        }
                    }
                    Action::Command(cmd_id) => match commands.get(&cmd_id) {
                        Some(cmd) => steps.push(MacroStepItem::Command(cmd.clone())),
                        None if foreign_commands.contains(&cmd_id) => steps.push(unavailable()),
                        // Disabled commands are skipped; the other steps still run.
                        None => log::debug!(
                            "[mapper] macro {} step: command {:?} disabled or unknown — skipped",
                            id,
                            cmd_id
                        ),
                    },
                    Action::Text(text) => steps.push(MacroStepItem::Literal(text)),
                    Action::Unknown(_) => steps.push(unavailable()),
                    Action::Pause(ms) => match pause_duration(&ms) {
                        Some(duration) => steps.push(MacroStepItem::Pause(duration)),
                        None => log::debug!("[mapper] macro {} step: invalid pause {:?}", id, raw),
                    },
                    Action::Keys(keys) => match parse_action(&keys) {
                        Some(ks) => steps.push(MacroStepItem::Stroke(ks)),
                        None => {
                            log::debug!("[mapper] macro {} step: unknown keystroke {:?}", id, raw)
                        }
                    },
                }
            }
            steps
        }

        for sys in BUILTIN_MACROS {
            let steps = resolve_macro_steps(
                sys.id,
                sys.steps.to_vec(),
                &raw_user_macros,
                &commands,
                &foreign_commands,
                0,
                &mut 1000,
            );
            if steps.is_empty() {
                log::debug!(
                    "[mapper] built-in macro {} has no usable steps — skipped",
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
            let steps = resolve_macro_steps(
                &m.id,
                raw_steps,
                &raw_user_macros,
                &commands,
                &foreign_commands,
                0,
                &mut 1000,
            );
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

        let resolve =
            |action: &str, where_: &str| resolve_action(action, &macros, &commands, where_);

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
            if matches!(
                key,
                KeyCode::BTN_LEFT | KeyCode::BTN_RIGHT | KeyCode::BTN_MIDDLE
            ) {
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
                    .clamp(1, crate::profile::timing::LONG_HOLD.max),
            );
            let hold_timeout = r
                .hold_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_hold);
            let double_tap_window = r
                .double_tap_timeout_ms
                .map(|ms| Duration::from_millis(ms.max(1)))
                .unwrap_or(default_double_tap);

            let entry = RuleEntry {
                tap_decision: r.tap_decision.unwrap_or(cfg.settings.tap_decision),
                trigger_keys: keys.clone(),
                long_hold,
                long_hold_timeout,
                tap,
                layer_id,
                hold,
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

        let mut layer_maps: HashMap<String, HashMap<KeyCode, ActionDef>> = HashMap::new();
        let mut layer_hold_mods: HashMap<String, HashSet<KeyCode>> = HashMap::new();
        for (layer_id, km) in &cfg.layer_keymaps {
            let hold_mods: HashSet<KeyCode> = km
                .hold_modifiers
                .iter()
                .filter_map(|code| code_to_key(code))
                .collect();
            if !hold_mods.is_empty() {
                layer_hold_mods.insert(layer_id.clone(), hold_mods);
            }
            let mut m = HashMap::new();
            for (code, action) in &km.keys {
                let Some(key) = code_to_key(code) else {
                    log::debug!("[mapper] unknown key code in keymap {layer_id}: {code}");
                    continue;
                };
                let def = match action.as_deref() {
                    None => ActionDef::Swallow,
                    // An empty entry leaves the key to the base layout.
                    Some(s) if s.trim().is_empty() => continue,
                    // An entry that does nothing here (an empty macro, a key
                    // this OS lacks) still covers the key, as a tap action
                    // does: typing the physical key instead would surprise.
                    Some(s) => resolve(s, &format!("keymap {layer_id}.{code}"))
                        .unwrap_or(ActionDef::Swallow),
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
            layer_hold_mods,
            default_hold,
            default_mod_delay,
            decision_buffer: Vec::new(),
            combo_buffer: Vec::new(),
            combo_term: Duration::from_millis(cfg.settings.combo_timeout_ms),
            active_layers: Vec::new(),
            pending: HashMap::new(),
            emitted: HashMap::new(),
            oneshot_consumed: HashSet::new(),
            mod_refs: HashMap::new(),
            layer_triggers: HashMap::new(),
            active_macro: None,
            macro_queue: VecDeque::new(),
            // Hand-edited configs may hold 0 or a value that overflows `Instant`.
            macro_timeout: Duration::from_millis(cfg.settings.macro_timeout_ms.clamp(
                crate::profile::timing::MACRO_TIMEOUT.min,
                crate::profile::timing::MACRO_TIMEOUT.max,
            )),
            text_pending: false,
            layer_presses: 0,
            layer_hold_id: 0,
            layer_used: false,
        }
    }
}

/// What an action bound outside a macro does on this system. `None` means
/// it does nothing (empty, `pause:`, an unknown key or an empty macro); an
/// action that exists but cannot run here becomes
/// [`ActionDef::Unavailable`], so firing it reports why nothing happened.
pub(super) fn resolve_action(
    action: &str,
    macros: &HashMap<String, MacroDef>,
    commands: &HashMap<String, SysCommand>,
    where_: &str,
) -> Option<ActionDef> {
    let unavailable = || Some(ActionDef::Unavailable(action.trim().into()));
    match Action::parse(Some(action)) {
        Action::Native | Action::Swallow => None,
        Action::Macro(id) => {
            let found = macros.get(&id).cloned().map(ActionDef::Macro);
            if found.is_none() {
                log::debug!("[mapper] macro {id:?} unknown or empty ({where_})");
            }
            found
        }
        Action::Command(id) => match commands.get(&id) {
            Some(cmd) => Some(ActionDef::Command(cmd.clone())),
            None => {
                log::debug!(
                    "[mapper] command {id:?} disabled, without a script for this OS or unknown ({where_})"
                );
                unavailable()
            }
        },
        Action::System(name) => match system::resolve(&name) {
            Some(cmd) => Some(ActionDef::System(cmd)),
            None => {
                log::debug!("[mapper] system fn {name:?} not available on this OS/DE ({where_})");
                unavailable()
            }
        },
        Action::KdeShortcut(shortcut) => match system::resolve_kde_shortcut(&shortcut) {
            Some(cmd) => Some(ActionDef::System(cmd)),
            None => {
                log::debug!("[mapper] KDE shortcut {shortcut:?} not available ({where_})");
                unavailable()
            }
        },
        Action::App(name) => match system::resolve_app(&name) {
            Some(cmd) => Some(ActionDef::System(cmd)),
            None => {
                log::debug!("[mapper] app action {name:?} not available ({where_})");
                unavailable()
            }
        },
        Action::Text(text) => Some(ActionDef::Literal(text)),
        Action::Pause(_) => {
            log::debug!("[mapper] pause is only allowed in macros ({where_})");
            None
        }
        Action::Unknown(value) => {
            log::debug!("[mapper] unknown action type {value:?} ({where_})");
            unavailable()
        }
        Action::Keys(keys) => {
            let stroke = parse_action(&keys).map(ActionDef::Stroke);
            if stroke.is_none() {
                log::debug!("[mapper] unknown action {keys:?} ({where_})");
            }
            stroke
        }
    }
}
