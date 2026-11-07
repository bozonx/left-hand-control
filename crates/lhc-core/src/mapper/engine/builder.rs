//! Compiles an `MapperConfig` into the engine's rule, layer and macro tables.

use super::Engine;
use super::model::{
    CompiledAction, CompiledMacro, CompiledMacroStep, HoldOutputMode, RuleEntry, TapMode,
};
use super::state::{ChordState, CompiledLayout, LayerStack, MacroRunner, VirtualKeys};
use crate::mapper::action::{MacroStepItem, parse_action, pause_duration};
use crate::mapper::builtin_macros::BUILTIN_MACROS;
use crate::mapper::config::{ActionSpec, MapperConfig};
use crate::mapper::key_code::KeyCode;
use crate::mapper::keys::code_to_key;
use crate::mapper::system::{self, ProcessInvocation};
use crate::profile::actions::{Action, MAX_MACRO_DEPTH, MAX_MACRO_STEPS};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

impl Engine {
    pub fn new(cfg: &MapperConfig) -> Self {
        let in_development = cfg.settings.in_development.unwrap_or(true);
        let default_hold = Duration::from_millis(cfg.settings.default_hold_timeout_ms.max(1));
        let default_double_tap =
            Duration::from_millis(cfg.settings.default_double_tap_timeout_ms.max(1));
        let default_step_pause = Duration::from_millis(cfg.settings.default_macro_step_pause_ms);
        let default_mod_delay = Duration::from_millis(cfg.settings.default_macro_modifier_delay_ms);

        // Build the macro table first so tap / keymap actions can reference
        // macros by their fully qualified action strings.
        let mut macros: HashMap<String, CompiledMacro> = HashMap::new();
        let mut commands: HashMap<String, ProcessInvocation> = HashMap::new();

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
                    continue;
                };
                commands.insert(id.to_string(), command);
            }
        } else {
            log::debug!("[mapper] shell commands are disabled for this layout");
        }

        let mut raw_user_macros: HashMap<String, &crate::mapper::config::Macro> = HashMap::new();
        for m in cfg.macros.iter().filter(|_| in_development) {
            if m.id.is_empty() {
                log::debug!("[mapper] skipping macro with empty id");
                continue;
            }
            raw_user_macros.insert(m.id.clone(), m);
        }

        struct MacroCompiler<'a> {
            users: &'a HashMap<String, &'a crate::mapper::config::Macro>,
            commands: &'a HashMap<String, ProcessInvocation>,
            defaults: (Duration, Duration),
        }
        impl MacroCompiler<'_> {
            fn resolve(
                &self,
                raw_steps: Vec<&str>,
                source: &str,
                timing: (Duration, Duration),
                depth: usize,
                remaining: &mut usize,
            ) -> Vec<CompiledMacroStep> {
                let mut steps = Vec::new();
                if depth > MAX_MACRO_DEPTH {
                    return steps;
                }
                for (index, raw) in raw_steps.into_iter().enumerate() {
                    if *remaining == 0 {
                        break;
                    }
                    if raw.trim().is_empty() {
                        continue;
                    }
                    *remaining -= 1;
                    let unavailable = || MacroStepItem::Unavailable(raw.trim().into());
                    let item = match Action::parse(Some(raw)) {
                        Action::Native | Action::Swallow => continue,
                        Action::System(name) => {
                            system::resolve(&name).map_or_else(unavailable, MacroStepItem::System)
                        }
                        Action::KdeShortcut(id) => system::resolve_kde_shortcut(&id)
                            .map_or_else(unavailable, MacroStepItem::System),
                        Action::DesktopEntry(id) => system::resolve_desktop_entry(&id)
                            .map_or_else(unavailable, MacroStepItem::System),
                        Action::App(name) => system::resolve_app(&name)
                            .map_or_else(unavailable, MacroStepItem::System),
                        Action::Macro(id) => {
                            if let Some(m) = self.users.get(&id) {
                                steps.extend(self.resolve(
                                    m.steps.iter().map(|s| s.action.as_str()).collect(),
                                    &format!("macro:{id}"),
                                    self.timing(m),
                                    depth + 1,
                                    remaining,
                                ));
                                continue;
                            }
                            unavailable()
                        }
                        Action::BuiltinMacro(id) => {
                            if let Some(m) = BUILTIN_MACROS.iter().find(|m| m.id == id) {
                                steps.extend(self.resolve(
                                    m.steps.to_vec(),
                                    &format!("builtin:{id}"),
                                    self.defaults,
                                    depth + 1,
                                    remaining,
                                ));
                                continue;
                            }
                            unavailable()
                        }
                        Action::Command(id) => self
                            .commands
                            .get(&id)
                            .cloned()
                            .map_or_else(unavailable, MacroStepItem::Command),
                        Action::Text(text) => MacroStepItem::Literal(text),
                        Action::Pause(ms) => {
                            pause_duration(&ms).map_or_else(unavailable, MacroStepItem::Pause)
                        }
                        Action::Keys(keys) => {
                            parse_action(&keys).map_or_else(unavailable, MacroStepItem::Stroke)
                        }
                        Action::Unknown(_) => unavailable(),
                    };
                    steps.push(CompiledMacroStep {
                        source: format!("{source}, step {}", index + 1),
                        item,
                        step_pause: timing.0,
                        mod_delay: timing.1,
                    });
                }
                steps
            }
            fn timing(&self, m: &crate::mapper::config::Macro) -> (Duration, Duration) {
                (
                    m.step_pause_ms
                        .map(Duration::from_millis)
                        .unwrap_or(self.defaults.0),
                    m.modifier_delay_ms
                        .map(Duration::from_millis)
                        .unwrap_or(self.defaults.1),
                )
            }
        }
        let compiler = MacroCompiler {
            users: &raw_user_macros,
            commands: &commands,
            defaults: (default_step_pause, default_mod_delay),
        };
        for sys in BUILTIN_MACROS {
            let steps = compiler.resolve(sys.steps.to_vec(), &format!("builtin:{}", sys.id), compiler.defaults, 0, &mut {
                MAX_MACRO_STEPS
            });
            if !steps.is_empty() {
                macros.insert(format!("builtin:{}", sys.id), CompiledMacro { steps });
            }
        }
        for m in cfg.macros.iter().filter(|_| in_development) {
            if m.id.is_empty() {
                continue;
            }
            let steps = compiler.resolve(
                m.steps.iter().map(|s| s.action.as_str()).collect(),
                &format!("macro:{}", m.id),
                compiler.timing(m),
                0,
                &mut { MAX_MACRO_STEPS },
            );
            if !steps.is_empty() {
                macros.insert(format!("macro:{}", m.id), CompiledMacro { steps });
            }
        }

        let resolve = |action: &str, where_: &str| {
            if !in_development
                && matches!(Action::parse(Some(action)), Action::App(id)
                    if id.starts_with("showEmojiMenu") || id.starts_with("showQuickMenu"))
            {
                return None;
            }
            resolve_action(action, &macros, &commands, where_)
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
            let behavior = if !in_development && behavior == HoldBehavior::Action {
                HoldBehavior::None
            } else {
                behavior
            };
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
                _ if behavior == HoldBehavior::Action => HoldOutputMode::Swallow,
                ActionSpec::Native if layer_id.is_some() || long_hold.is_some() => {
                    HoldOutputMode::Swallow
                }
                // A swallowed tap with "nothing" on hold must swallow the whole
                // press; a native hold would leak the key through the fast path.
                ActionSpec::Native if matches!(tap, TapMode::Swallow) => HoldOutputMode::Swallow,
                ActionSpec::Native if keys.len() > 1 => {
                    HoldOutputMode::Keystroke(crate::mapper::action::Keystroke {
                        mods: keys[..keys.len() - 1].to_vec(),
                        key: keys[keys.len() - 1],
                    })
                }
                ActionSpec::Native => HoldOutputMode::Native,
                ActionSpec::Swallow => HoldOutputMode::Swallow,
                ActionSpec::Action(s) => match parse_action(s) {
                    Some(ks) => HoldOutputMode::Keystroke(ks),
                    None => {
                        log::debug!(
                            "[mapper] rule {:?}: unknown hold keystroke {:?} — falling back to native hold",
                            r.key,
                            s
                        );
                        HoldOutputMode::Native
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
                && (matches!(hold, HoldOutputMode::Native)
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
                condition_keyboard_layouts: r.condition_keyboard_layouts.clone(),
                condition_window_patterns: r.condition_window_patterns.clone(),
                condition_excluded_window_patterns: r.condition_excluded_window_patterns.clone(),
            };
            if keys.len() > 1 {
                chord_rules.push((keys, entry));
            } else {
                rules.entry(key).or_insert_with(Vec::new).push(entry);
            }
        }

        let mut layer_maps: HashMap<String, HashMap<KeyCode, CompiledAction>> = HashMap::new();
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
                    None => CompiledAction::Swallow,
                    // An empty entry leaves the key to the base layout.
                    Some(s) if s.trim().is_empty() => continue,
                    // An entry that does nothing here (an empty macro, a key
                    // this OS lacks) still covers the key, as a tap action
                    // does: typing the physical key instead would surprise.
                    Some(s) => resolve(s, &format!("keymap {layer_id}.{code}"))
                        .unwrap_or(CompiledAction::Swallow),
                };
                m.insert(key, def);
            }

            layer_maps.insert(layer_id.clone(), m);
        }

        Self {
            layout: CompiledLayout {
                rules,
                chord_rules,
                macros,
                commands,
                layer_maps,
                layer_hold_mods,
                default_mod_delay,
                trigger_chord_timeout: Duration::from_millis(cfg.settings.trigger_chord_timeout_ms),
            },
            physical_down: HashSet::new(),
            press_at: HashMap::new(),
            long_holds: HashMap::new(),
            decision_buffer: Vec::new(),
            pending: HashMap::new(),
            chords: ChordState::default(),
            layers: LayerStack::default(),
            virt: VirtualKeys::default(),
            // Hand-edited configs may hold 0 or a value that overflows `Instant`.
            runner: MacroRunner::new(Duration::from_millis(cfg.settings.macro_timeout_ms.clamp(
                crate::profile::timing::MACRO_TIMEOUT.min,
                crate::profile::timing::MACRO_TIMEOUT.max,
            ))),
            context: Box::new(crate::profile::auto_switch::AutoSwitchContext::current),
        }
    }
}

/// What an action bound outside a macro does on this system. `None` means
/// it does nothing (empty, `pause:`, an unknown key or an empty macro); an
/// action that exists but cannot run here becomes
/// [`CompiledAction::Unavailable`], so firing it reports why nothing happened.
pub(super) fn resolve_action(
    action: &str,
    macros: &HashMap<String, CompiledMacro>,
    commands: &HashMap<String, ProcessInvocation>,
    where_: &str,
) -> Option<CompiledAction> {
    let unavailable = || Some(CompiledAction::Unavailable(action.trim().into()));
    match Action::parse(Some(action)) {
        Action::Native | Action::Swallow => None,
        action @ (Action::Macro(_) | Action::BuiltinMacro(_)) => {
            let id = action.format()?;
            let found = macros.get(&id).cloned().map(CompiledAction::Macro);
            if found.is_none() {
                log::debug!("[mapper] macro {id:?} unknown or empty ({where_})");
            }
            found
        }
        Action::Command(id) => match commands.get(&id) {
            Some(cmd) => Some(CompiledAction::Command(cmd.clone())),
            None => {
                log::debug!(
                    "[mapper] command {id:?} disabled, without a script for this OS or unknown ({where_})"
                );
                unavailable()
            }
        },
        Action::System(name) => match system::resolve(&name) {
            Some(cmd) => Some(CompiledAction::System(cmd)),
            None => {
                log::debug!("[mapper] system fn {name:?} not available on this OS/DE ({where_})");
                unavailable()
            }
        },
        Action::KdeShortcut(shortcut) => match system::resolve_kde_shortcut(&shortcut) {
            Some(cmd) => Some(CompiledAction::System(cmd)),
            None => {
                log::debug!("[mapper] KDE shortcut {shortcut:?} not available ({where_})");
                unavailable()
            }
        },
        Action::DesktopEntry(id) => match system::resolve_desktop_entry(&id) {
            Some(cmd) => Some(CompiledAction::System(cmd)),
            None => {
                log::debug!("[mapper] application action {id:?} not installed ({where_})");
                unavailable()
            }
        },
        Action::App(name) => match system::resolve_app(&name) {
            Some(cmd) => Some(CompiledAction::System(cmd)),
            None => {
                log::debug!("[mapper] app action {name:?} not available ({where_})");
                unavailable()
            }
        },
        Action::Text(text) => Some(CompiledAction::Literal(text)),
        Action::Pause(_) => {
            log::debug!("[mapper] pause is only allowed in macros ({where_})");
            None
        }
        Action::Unknown(value) => {
            log::debug!("[mapper] unknown action type {value:?} ({where_})");
            unavailable()
        }
        Action::Keys(keys) => {
            let stroke = parse_action(&keys).map(CompiledAction::Stroke);
            if stroke.is_none() {
                log::debug!("[mapper] unknown action {keys:?} ({where_})");
            }
            stroke
        }
    }
}
