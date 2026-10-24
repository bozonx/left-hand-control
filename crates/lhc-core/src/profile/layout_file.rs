//! Saved layout JSON files in the user library.
//!
//! The format is versioned (`"schemaVersion": 1`) and read strictly: unknown
//! fields, wrong types and unknown enum values are errors, so a file is
//! never silently rewritten with parts of it dropped. Semantic checks
//! (unknown macros, invalid key names in actions) are left to the
//! diagnostics, which report them without refusing the file.

use super::model::{
    Command, EmojiPage, HoldBehavior, LEFT_HAND_HOTKEYS, Layer, LayerKeymap, LayerRule,
    LayoutConditionSet, LayoutPreset, Macro, MacroStep, QuickAction, QuickActionPage, TapDecision,
};
use super::schema::{self, LAYOUT_SCHEMA_VERSION};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

/// Parse a layout file. Returns `Ok(None)` for an empty document.
pub fn parse(text: &str) -> Result<Option<LayoutPreset>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("parse layout: {error}"))?;
    let layout = match schema::read_version(&value, "layout", LAYOUT_SCHEMA_VERSION)? {
        1 => from_doc(
            serde_json::from_str::<LayoutDoc>(text)
                .map_err(|error| schema::read_error("parse layout", &error))?,
        )?,
        version => return Err(format!("layout: no reader for schema version {version}")),
    };
    super::timing::validate_layout(&layout)?;
    Ok(Some(layout))
}

pub fn serialize(preset: &LayoutPreset) -> String {
    serde_json::to_string_pretty(&to_doc(preset)).unwrap_or_default()
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayoutDoc {
    schema_version: u64,
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    schema: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    meta: Option<super::model::LayoutMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layers: Vec<LayerDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<RuleDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    macros: Vec<MacroDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    commands: Vec<CommandDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    quick_action_pages: Vec<PageDoc<QuickCellDoc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    emoji_pages: Vec<PageDoc<String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LayerDoc {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    /// Saved display order of additional keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    extra_key_order: Vec<String>,
    /// Key code → action; `null` swallows the key, a missing key is
    /// transparent.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    keys: BTreeMap<String, Option<String>>,
    /// Keys of `keys` whose keystroke modifiers stay pressed until the
    /// layer is released.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hold_modifiers: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuleDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    key: String,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    enabled: bool,
    #[serde(default, skip_serializing_if = "WhenDoc::is_empty")]
    when: WhenDoc,
    /// Missing: native key, `null`: swallowed, string: action.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    tap: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    double_tap: Option<TimedActionDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hold: Option<HoldDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    long_hold: Option<TimedActionDoc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tap_decision: Option<TapDecision>,
}

/// Conditions of a layout rule or an auto-mode rule; every present
/// condition must hold. Both places share this one object.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WhenDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    game_mode: Option<OnOff>,
    /// Short ids of the system keyboard layout, e.g. `us`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keyboard_layouts: Vec<String>,
    /// Window patterns, see [`super::app_match`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    windows: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exclude_windows: Vec<String>,
}

impl WhenDoc {
    pub(crate) fn is_empty(&self) -> bool {
        self.game_mode.is_none()
            && self.keyboard_layouts.is_empty()
            && self.windows.is_empty()
            && self.exclude_windows.is_empty()
    }

    fn check(&self) -> Result<(), String> {
        non_empty_items(&self.keyboard_layouts, "keyboard layout")?;
        non_empty_items(&self.windows, "window pattern")?;
        non_empty_items(&self.exclude_windows, "window pattern")
    }

    pub(crate) fn into_set(self) -> Result<LayoutConditionSet, String> {
        self.check()?;
        Ok(LayoutConditionSet {
            game_mode: self.game_mode.map(|mode| mode.as_str().to_owned()),
            layouts: self.keyboard_layouts,
            apps: self.windows,
            excluded_apps: self.exclude_windows,
        })
    }

    pub(crate) fn from_set(set: &LayoutConditionSet) -> Self {
        let list = |items: &[String]| -> Vec<String> {
            items
                .iter()
                .filter(|item| !item.trim().is_empty())
                .cloned()
                .collect()
        };
        Self {
            game_mode: OnOff::parse(set.game_mode.as_deref()),
            keyboard_layouts: list(&set.layouts),
            windows: list(&set.apps),
            exclude_windows: list(&set.excluded_apps),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OnOff {
    On,
    Off,
}

impl OnOff {
    pub(crate) fn parse(value: Option<&str>) -> Option<Self> {
        match value {
            Some("on") => Some(Self::On),
            Some("off") => Some(Self::Off),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TimedActionDoc {
    action: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    timeout_ms: Option<u64>,
}

/// What holding the key does: activate a layer and/or hold other keys.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HoldDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layer: Option<String>,
    /// Keys held down while the key is held. Missing: the key itself (or
    /// nothing while a layer is active), `null`: nothing.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    keys: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    timeout_ms: Option<u64>,
}

impl HoldDoc {
    fn is_empty(&self) -> bool {
        self.layer.is_none() && self.keys.is_none() && self.timeout_ms.is_none()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MacroDoc {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    step_pause_ms: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "schema::non_null"
    )]
    modifier_delay_ms: Option<u64>,
    #[serde(default)]
    steps: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandDoc {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// POSIX `sh` script, run on Linux and macOS.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    unix: String,
    /// PowerShell script, run on Windows.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    windows: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    working_directory: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: Deserialize<'de>"))]
struct PageDoc<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Hotkey from [`LEFT_HAND_HOTKEYS`] → cell, written in grid order.
    #[serde(default, serialize_with = "grid_order")]
    cells: BTreeMap<String, T>,
}

/// A quick-action cell: a bare action or an action with a caption/icon.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum QuickCellDoc {
    Action(String),
    Full(QuickActionDoc),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct QuickActionDoc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

/// Distinguish a missing field (`None`) from an explicit `null`
/// (`Some(None)`).
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

fn grid_order<S: Serializer, T: Serialize>(
    cells: &BTreeMap<String, T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map(
        LEFT_HAND_HOTKEYS
            .iter()
            .filter_map(|key| cells.get(*key).map(|cell| (*key, cell))),
    )
}

fn non_empty(value: &str, what: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{what} must not be empty"))
    } else {
        Ok(())
    }
}

fn check_cells<'a>(mut keys: impl Iterator<Item = &'a String>, menu: &str) -> Result<(), String> {
    keys.try_for_each(|key| {
        if LEFT_HAND_HOTKEYS.contains(&key.as_str()) {
            Ok(())
        } else {
            Err(format!("{menu} page cell {key}: not a menu hotkey"))
        }
    })
}

/// A rule trigger in its canonical spelling: sided key codes, so `Ctrl`
/// is stored as `ControlLeft`. An invalid trigger is kept for diagnostics.
fn canonical_trigger(key: &str) -> String {
    super::key_catalog::trigger_keys(key)
        .map(|keys| keys.join("+"))
        .unwrap_or_else(|| key.to_owned())
}

fn non_empty_items(items: &[String], what: &str) -> Result<(), String> {
    items.iter().try_for_each(|item| non_empty(item, what))
}

/// Three-state action of a parsed field: `Some("")` native, `None`
/// swallowed, otherwise the action.
fn three_state(value: Option<Option<String>>, what: &str) -> Result<Option<String>, String> {
    match value {
        None => Ok(Some(String::new())),
        Some(None) => Ok(None),
        Some(Some(action)) => {
            non_empty(&action, what)?;
            Ok(Some(action))
        }
    }
}

fn from_doc(mut doc: LayoutDoc) -> Result<LayoutPreset, String> {
    super::ids::unique(doc.layers.iter().map(|item| item.id.as_str()), "layers")?;
    super::ids::unique(doc.macros.iter().map(|item| item.id.as_str()), "macros")?;
    super::ids::unique(doc.commands.iter().map(|item| item.id.as_str()), "commands")?;
    macro_rules! complete {
        ($items:expr, $prefix:expr, $path:expr) => {{
            let mut ids: Vec<_> = $items.iter_mut().map(|item| item.id.take()).collect();
            super::ids::complete(&mut ids, $prefix, $path)?;
            for (item, id) in $items.iter_mut().zip(ids) {
                item.id = id;
            }
        }};
    }
    complete!(doc.rules, "rule-", "rules");
    complete!(doc.quick_action_pages, "page-", "quickActionPages");
    complete!(doc.emoji_pages, "emoji-", "emojiPages");
    // Step ids live only in memory, for the editor.
    let mut step_counter = 0u32;

    let mut layers = Vec::new();
    let mut layer_keymaps = BTreeMap::new();
    for layer in doc.layers {
        non_empty(&layer.id, "layer id")?;
        for (code, action) in &layer.keys {
            if let Some(action) = action {
                non_empty(action, &format!("action of {code} in layer {}", layer.id))?;
            }
        }
        let mut hold_modifiers = BTreeSet::new();
        for code in layer.hold_modifiers {
            if !matches!(layer.keys.get(&code), Some(Some(_))) {
                return Err(format!(
                    "holdModifiers of layer {}: {code} has no action in the layer",
                    layer.id
                ));
            }
            if !hold_modifiers.insert(code.clone()) {
                return Err(format!(
                    "holdModifiers of layer {}: duplicate key {code}",
                    layer.id
                ));
            }
        }
        layers.push(Layer {
            name: layer.name.unwrap_or_else(|| layer.id.clone()),
            description: layer.description.filter(|d| !d.is_empty()),
            id: layer.id.clone(),
        });
        layer_keymaps.insert(
            layer.id,
            LayerKeymap {
                keys: layer.keys,
                extra_key_order: layer.extra_key_order,
                hold_modifiers,
            },
        );
    }

    let mut rules = Vec::new();
    for rule in doc.rules {
        non_empty(&rule.key, "rule key")?;
        let what = |field: &str| format!("{field} of rule {}", rule.key);
        if rule.hold.is_some() && rule.long_hold.is_some() {
            return Err(format!(
                "rule {}: \"hold\" and \"longHold\" cannot be combined",
                rule.key
            ));
        }
        let when = rule.when.into_set()?;
        let tap_action = three_state(rule.tap, &what("tap"))?;
        let (double_tap_action, double_tap_timeout_ms) = match rule.double_tap {
            Some(double) => {
                non_empty(&double.action, &what("doubleTap action"))?;
                (double.action, double.timeout_ms)
            }
            None => (String::new(), None),
        };
        let (long_hold_action, long_hold_timeout_ms) = match rule.long_hold {
            Some(long) => {
                non_empty(&long.action, &what("longHold action"))?;
                (long.action, long.timeout_ms)
            }
            None => (String::new(), None),
        };
        let hold = rule.hold.unwrap_or_default();
        if let Some(layer) = &hold.layer {
            non_empty(layer, &what("hold layer"))?;
        }
        rules.push(LayerRule {
            id: rule.id.unwrap_or_default(),
            enabled: (!rule.enabled).then_some(false),
            condition_game_mode: when.game_mode,
            condition_layouts: (!when.layouts.is_empty()).then_some(when.layouts),
            condition_apps_whitelist: (!when.apps.is_empty()).then_some(when.apps),
            condition_apps_blacklist: (!when.excluded_apps.is_empty())
                .then_some(when.excluded_apps),
            key: canonical_trigger(&rule.key),
            layer_id: hold.layer.unwrap_or_default(),
            tap_action,
            hold_behavior: None,
            long_hold_action,
            long_hold_timeout_ms,
            hold_action: three_state(hold.keys, &what("hold keys"))?,
            double_tap_action,
            hold_timeout_ms: hold.timeout_ms,
            tap_decision: rule.tap_decision,
            double_tap_timeout_ms,
        });
    }

    let mut macros = Vec::new();
    for item in doc.macros {
        non_empty(&item.id, "macro id")?;
        let mut steps = Vec::new();
        for action in item.steps {
            non_empty(&action, &format!("step of macro {}", item.id))?;
            step_counter += 1;
            steps.push(MacroStep {
                id: format!("step-{step_counter}"),
                action,
            });
        }
        macros.push(Macro {
            name: item.name.unwrap_or_else(|| item.id.clone()),
            id: item.id,
            steps,
            step_pause_ms: item.step_pause_ms,
            modifier_delay_ms: item.modifier_delay_ms,
        });
    }

    let mut commands = Vec::new();
    for command in doc.commands {
        non_empty(&command.id, "command id")?;
        commands.push(Command {
            name: command.name.unwrap_or_else(|| command.id.clone()),
            id: command.id,
            linux: command.unix,
            windows: command.windows,
            working_directory: command.working_directory.filter(|dir| !dir.is_empty()),
        });
    }

    let mut quick_action_pages = Vec::new();
    for page in doc.quick_action_pages {
        check_cells(page.cells.keys(), "quick action")?;
        let mut cells = BTreeMap::new();
        for (key, cell) in page.cells {
            let item = match cell {
                QuickCellDoc::Action(action) => QuickAction {
                    action,
                    ..QuickAction::default()
                },
                QuickCellDoc::Full(item) => QuickAction {
                    name: item.name.unwrap_or_default(),
                    action: item.action,
                    icon: item.icon.filter(|icon| !icon.is_empty()),
                },
            };
            non_empty(&item.action, &format!("quick action {key}"))?;
            cells.insert(key, item);
        }
        quick_action_pages.push(QuickActionPage {
            id: page.id.unwrap_or_default(),
            name: page.name.unwrap_or_default(),
            cells,
        });
    }
    if quick_action_pages.is_empty() {
        quick_action_pages.push(QuickActionPage::default_page());
    }

    let mut emoji_pages = Vec::new();
    for page in doc.emoji_pages {
        check_cells(page.cells.keys(), "emoji")?;
        for (key, text) in &page.cells {
            non_empty(text, &format!("emoji {key}"))?;
        }
        emoji_pages.push(EmojiPage {
            id: page.id.unwrap_or_default(),
            name: page.name.unwrap_or_default(),
            cells: page.cells,
        });
    }
    if emoji_pages.is_empty() {
        emoji_pages.push(EmojiPage::default_page());
    }

    Ok(LayoutPreset {
        schema: doc.schema,
        meta: doc.meta,
        description: doc.description.filter(|d| !d.trim().is_empty()),
        layers,
        rules,
        layer_keymaps,
        macros,
        commands,
        quick_action_pages,
        emoji_pages,
    })
}

/// Build the file form. Entries the parser would reject but the editor can
/// hold transiently (a rule without a key, an empty macro step) are left
/// out, so every saved file loads again.
fn to_doc(preset: &LayoutPreset) -> LayoutDoc {
    let text = |value: &str| (!value.is_empty()).then(|| value.to_owned());
    let list = |items: &Option<Vec<String>>| -> Vec<String> {
        items
            .iter()
            .flatten()
            .filter(|item| !item.trim().is_empty())
            .cloned()
            .collect()
    };
    // `Some("")` is the native default and is omitted.
    let three_state = |action: &Option<String>| match action.as_deref() {
        Some(action) if action.trim().is_empty() => None,
        Some(action) => Some(Some(action.to_owned())),
        None => Some(None),
    };

    let layers = preset
        .layers
        .iter()
        .filter(|layer| !layer.id.is_empty())
        .map(|layer| {
            let keymap = preset.layer_keymaps.get(&layer.id);
            LayerDoc {
                id: layer.id.clone(),
                name: Some(layer.name.clone()),
                description: layer.description.clone().filter(|d| !d.is_empty()),
                extra_key_order: keymap
                    .map(|keymap| keymap.extra_key_order.clone())
                    .unwrap_or_default(),
                keys: keymap
                    .map(|keymap| {
                        keymap
                            .keys
                            .iter()
                            .map(|(code, action)| {
                                let action = action.clone().filter(|a| !a.trim().is_empty());
                                (code.clone(), action)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                // Only keys that still carry an action, so the file loads again.
                hold_modifiers: keymap
                    .map(|keymap| {
                        keymap
                            .hold_modifiers
                            .iter()
                            .filter(|code| {
                                keymap.keys.get(*code).is_some_and(|action| {
                                    action.as_ref().is_some_and(|a| !a.trim().is_empty())
                                })
                            })
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();

    let rules = preset
        .rules
        .iter()
        .filter(|rule| rule.has_trigger())
        .map(|rule| {
            let behavior = rule.hold_behavior();
            let long_hold = (behavior == HoldBehavior::Action
                && !rule.long_hold_action.trim().is_empty())
            .then(|| TimedActionDoc {
                action: rule.long_hold_action.clone(),
                timeout_ms: rule.long_hold_timeout_ms,
            });
            let hold = if behavior == HoldBehavior::Action {
                HoldDoc::default()
            } else {
                HoldDoc {
                    layer: (behavior == HoldBehavior::Layer)
                        .then(|| text(&rule.layer_id))
                        .flatten(),
                    keys: three_state(&rule.hold_action),
                    timeout_ms: rule.hold_timeout_ms,
                }
            };
            RuleDoc {
                id: text(&rule.id),
                key: rule.key.clone(),
                enabled: rule.is_enabled(),
                when: WhenDoc {
                    game_mode: OnOff::parse(rule.condition_game_mode.as_deref()),
                    keyboard_layouts: list(&rule.condition_layouts),
                    windows: list(&rule.condition_apps_whitelist),
                    exclude_windows: list(&rule.condition_apps_blacklist),
                },
                tap: three_state(&rule.tap_action),
                double_tap: (!rule.double_tap_action.trim().is_empty()).then(|| TimedActionDoc {
                    action: rule.double_tap_action.clone(),
                    timeout_ms: rule.double_tap_timeout_ms,
                }),
                hold: (!hold.is_empty()).then_some(hold),
                long_hold,
                tap_decision: rule.tap_decision,
            }
        })
        .collect();

    let macros = preset
        .macros
        .iter()
        .filter(|item| !item.id.is_empty())
        .map(|item| MacroDoc {
            id: item.id.clone(),
            name: Some(item.name.clone()),
            step_pause_ms: item.step_pause_ms,
            modifier_delay_ms: item.modifier_delay_ms,
            steps: item
                .steps
                .iter()
                .filter(|step| !step.action.trim().is_empty())
                .map(|step| step.action.clone())
                .collect(),
        })
        .collect();

    let commands = preset
        .commands
        .iter()
        .filter(|command| !command.id.is_empty())
        .map(|command| CommandDoc {
            id: command.id.clone(),
            name: Some(command.name.clone()),
            unix: command.linux.clone(),
            windows: command.windows.clone(),
            working_directory: command.working_directory.clone().filter(|d| !d.is_empty()),
        })
        .collect();

    let quick_action_pages: Vec<PageDoc<QuickCellDoc>> = preset
        .quick_action_pages
        .iter()
        .map(|page| PageDoc {
            id: text(&page.id),
            name: text(&page.name),
            cells: page
                .cells
                .iter()
                .filter(|(key, item)| {
                    LEFT_HAND_HOTKEYS.contains(&key.as_str()) && !item.action.trim().is_empty()
                })
                .map(|(key, item)| {
                    let icon = item.icon.clone().filter(|icon| !icon.is_empty());
                    let cell = if item.name.is_empty() && icon.is_none() {
                        QuickCellDoc::Action(item.action.clone())
                    } else {
                        QuickCellDoc::Full(QuickActionDoc {
                            name: text(&item.name),
                            action: item.action.clone(),
                            icon,
                        })
                    };
                    (key.clone(), cell)
                })
                .collect(),
        })
        .collect();

    let emoji_pages: Vec<PageDoc<String>> = preset
        .emoji_pages
        .iter()
        .map(|page| PageDoc {
            id: text(&page.id),
            name: text(&page.name),
            cells: page
                .cells
                .iter()
                .filter(|(key, value)| {
                    LEFT_HAND_HOTKEYS.contains(&key.as_str()) && !value.is_empty()
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        })
        .collect();

    // A single unnamed empty page is what a missing list loads as.
    fn only_default<T>(pages: &[PageDoc<T>]) -> bool {
        matches!(pages, [page] if page.name.is_none() && page.cells.is_empty())
    }
    LayoutDoc {
        schema_version: LAYOUT_SCHEMA_VERSION,
        schema: preset.schema.clone(),
        meta: preset.meta.clone(),
        description: preset.description.clone().filter(|d| !d.trim().is_empty()),
        layers,
        rules,
        macros,
        commands,
        quick_action_pages: if only_default(&quick_action_pages) {
            Vec::new()
        } else {
            quick_action_pages
        },
        emoji_pages: if only_default(&emoji_pages) {
            Vec::new()
        } else {
            emoji_pages
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn layout(body: Value) -> String {
        let mut body = body;
        body["schemaVersion"] = json!(1);
        body.to_string()
    }

    fn parsed(body: Value) -> LayoutPreset {
        parse(&layout(body)).unwrap().unwrap()
    }

    fn round_trip(preset: &LayoutPreset) -> LayoutPreset {
        parse(&serialize(preset)).unwrap().unwrap()
    }

    #[test]
    fn timing_inherits_only_when_absent() {
        for kind in ["hold", "longHold", "doubleTap"] {
            let action = if kind == "hold" {
                json!({"keys": "KeyB"})
            } else {
                json!({"action": "KeyB"})
            };
            let mut rule = json!({"key": "KeyA", kind: action});
            assert!(parse(&layout(json!({"rules": [rule.clone()]}))).is_ok());
            rule[kind]["timeoutMs"] = Value::Null;
            assert!(parse(&layout(json!({"rules": [rule]}))).is_err());
        }
        for field in ["stepPauseMs", "modifierDelayMs"] {
            assert!(parse(&layout(json!({"macros": [{"id": "m", field: null}]}))).is_err());
        }
        let preset = parsed(json!({"rules": [{"key": "KeyA", "tap": null}]}));
        assert_eq!(round_trip(&preset), preset);
    }

    #[test]
    fn portable_metadata_and_schema_survive_round_trip() {
        let body = json!({"$schema": "https://example.com/layout.schema.json",
            "meta": {"name": "Original", "author": "Ivan K", "version": "1.0"}});
        let preset = parsed(body.clone());
        assert_eq!(round_trip(&preset), preset);
        let saved: Value = serde_json::from_str(&serialize(&preset)).unwrap();
        assert_eq!(saved["meta"], body["meta"]);
        assert_eq!(saved["$schema"], body["$schema"]);
        for invalid in [
            json!({"$schema": null}),
            json!({"$schema": 1}),
            json!({"meta": null}),
            json!({"meta": {"author": null}}),
            json!({"meta": {"future": true}}),
            json!({"meta": {"version": 1}}),
        ] {
            assert!(parse(&layout(invalid)).is_err());
        }
    }

    #[test]
    fn duplicate_ids_are_rejected_before_conversion() {
        for field in [
            "layers",
            "rules",
            "macros",
            "commands",
            "quickActionPages",
            "emojiPages",
        ] {
            let item = if field == "rules" {
                serde_json::json!({"id": "same", "key": "KeyA"})
            } else {
                serde_json::json!({"id": "same"})
            };
            let text =
                serde_json::json!({"schemaVersion": 1, field: [item.clone(), item]}).to_string();
            let error = parse(&text).unwrap_err();
            assert!(error.contains("duplicate id"), "{field}: {error}");
        }
    }

    #[test]
    fn generated_ids_reserve_explicit_ids_including_later_entries() {
        let text = r#"{"schemaVersion":1,"rules":[{"key":"KeyA"},{"id":"rule-1","key":"KeyB"}],
            "quickActionPages":[{}, {"id":"page-1"}],"emojiPages":[{}, {"id":"emoji-1"}]}"#;
        let layout = parse(text).unwrap().unwrap();
        super::super::ids::validate_layout(&layout).unwrap();
        assert_eq!(layout.rules[0].id, "rule-2");
        assert_eq!(layout.quick_action_pages[0].id, "page-2");
        assert_eq!(layout.emoji_pages[0].id, "emoji-2");
        assert_eq!(parse(&serialize(&layout)).unwrap().unwrap(), layout);
        assert_eq!(parse(text).unwrap().unwrap(), layout);
    }

    #[test]
    fn explicit_invalid_ids_are_not_replaced() {
        for id in ["", " ", " x", "x ", "a b", "a.b"] {
            let text =
                serde_json::json!({"schemaVersion":1,"rules":[{"id":id,"key":"KeyA"}]}).to_string();
            assert!(parse(&text).is_err(), "{id:?}");
        }
    }

    #[test]
    fn rule_and_macro_timing_boundaries() {
        for (field, range) in [
            ("hold", super::super::timing::TAP_HOLD),
            ("doubleTap", super::super::timing::TAP_HOLD),
            ("longHold", super::super::timing::LONG_HOLD),
        ] {
            for value in [
                0,
                range.min - 1,
                range.min,
                range.max,
                range.max + 1,
                u64::MAX,
            ] {
                let timed = if field == "hold" {
                    serde_json::json!({"timeoutMs":value})
                } else {
                    serde_json::json!({"action":"KeyB", "timeoutMs":value})
                };
                let text =
                    serde_json::json!({"schemaVersion":1,"rules":[{"key":"KeyA",field:timed}]})
                        .to_string();
                assert_eq!(
                    parse(&text).is_ok(),
                    range.contains(value),
                    "{field}: {value}"
                );
            }
        }
        for field in ["stepPauseMs", "modifierDelayMs"] {
            for value in [0, 2000, 2001, u64::MAX] {
                let text = serde_json::json!({"schemaVersion":1,"macros":[{"id":"m",field:value}]})
                    .to_string();
                assert_eq!(parse(&text).is_ok(), value <= 2000);
            }
        }
    }

    #[test]
    fn version_is_required_and_checked() {
        assert!(
            parse(r#"{"rules": []}"#)
                .unwrap_err()
                .contains("schemaVersion")
        );
        assert!(parse(r#"{"version": 1}"#).is_err());
        assert!(
            parse(r#"{"schemaVersion": 2, "rules": []}"#)
                .unwrap_err()
                .contains("newer")
        );
        assert!(parse(r#"{"schemaVersion": "1"}"#).is_err());
        assert!(parse(r#"{"schemaVersion": 1}"#).unwrap().is_some());
        assert_eq!(
            serde_json::from_str::<Value>(&serialize(&LayoutPreset::default())).unwrap()["schemaVersion"],
            1
        );
    }

    #[test]
    fn unknown_fields_and_values_are_rejected() {
        for body in [
            json!({"extra": true}),
            json!({"rules": [{"key": "KeyA", "holdMs": 100}]}),
            json!({"rules": [{"key": "KeyA", "hold": {"layer": "x", "isolate": []}}]}),
            json!({"rules": [{"key": "KeyA", "when": {"layouts": ["us"]}}]}),
            json!({"rules": [{"key": "KeyA", "when": {"gameMode": "maybe"}}]}),
            json!({"rules": [{"key": "KeyA", "tapDecision": "sometimes"}]}),
            json!({"rules": [{"key": "KeyA", "tap": 5}]}),
            json!({"rules": [{"key": "KeyA", "tap": ""}]}),
            json!({"rules": [{"key": ""}]}),
            json!({"rules": [{"key": "KeyA", "hold": {"layer": "x"}, "longHold": {"action": "KeyB"}}]}),
            json!({"layers": [{"id": "nav", "keys": {"KeyA": ""}}]}),
            json!({"rules": [{"key": "KeyA", "hold": {"layer": "x", "keys": "AltLeft", "keysOnlyFor": ["Tab"]}}]}),
            json!({"rules": [{"key": "KeyA", "hold": {"layer": "x", "keys": "AltLeft", "keysExceptFor": ["Tab"]}}]}),
            json!({"layers": [{"id": "nav", "holdModifiers": ["Tab"]}]}),
            json!({"layers": [{"id": "nav", "keys": {"Tab": null}, "holdModifiers": ["Tab"]}]}),
            json!({"layers": [{"id": "nav", "keys": {"Tab": "Alt+Tab"}, "holdModifiers": ["Tab", "Tab"]}]}),
            json!({"macros": [{"id": "m", "steps": [{"action": "KeyA"}]}]}),
            json!({"macros": [{"id": "m", "steps": [" "]}]}),
            json!({"commands": [{"id": "c", "linux": "ls"}]}),
            json!({"quickActionPages": [{"cells": {"KeyP": "KeyA"}}]}),
            json!({"quickActionPages": [{"cells": {"KeyQ": {"name": "No action"}}}]}),
            json!({"emojiPages": [{"cells": {"KeyQ": 5}}]}),
        ] {
            assert!(parse(&layout(body.clone())).is_err(), "accepted {body}");
        }
        let error = parse(&layout(json!({"extra": true}))).unwrap_err();
        assert!(error.contains("newer version"), "{error}");
    }

    #[test]
    fn rule_triggers_are_stored_canonically() {
        let preset = parsed(json!({"rules": [
            {"key": "Ctrl + KeyA"}, {"key": "Shift+ControlRight"}, {"key": "NoSuchKey"},
        ]}));
        let keys: Vec<_> = preset.rules.iter().map(|rule| rule.key.as_str()).collect();
        assert_eq!(
            keys,
            ["ControlLeft+KeyA", "ShiftLeft+ControlRight", "NoSuchKey"]
        );
    }

    #[test]
    fn rule_fields_map_to_the_model() {
        let preset = parsed(json!({"rules": [
            {"id": "r1", "key": "CapsLock", "tap": "Escape",
             "hold": {"layer": "nav", "keys": "AltLeft", "timeoutMs": 180},
             "doubleTap": {"action": "Enter", "timeoutMs": 150},
             "tapDecision": "holdOnOtherKeyPress"},
            {"id": "r2", "key": "Space", "enabled": false, "tap": null, "hold": {"keys": null},
             "when": {"gameMode": "on", "keyboardLayouts": ["us"], "windows": ["code"], "excludeWindows": ["title:x"]}},
            {"id": "r3", "key": "ShiftLeft+ControlRight", "longHold": {"action": "text:hello", "timeoutMs": 1500}},
        ]}));
        let caps = &preset.rules[0];
        assert_eq!(caps.hold_behavior(), HoldBehavior::Layer);
        assert_eq!(caps.layer_id, "nav");
        assert_eq!(caps.tap_action.as_deref(), Some("Escape"));
        assert_eq!(caps.hold_action.as_deref(), Some("AltLeft"));
        assert_eq!(caps.hold_timeout_ms, Some(180));
        assert_eq!(caps.double_tap_action, "Enter");
        assert_eq!(caps.double_tap_timeout_ms, Some(150));
        assert_eq!(caps.tap_decision, Some(TapDecision::HoldOnOtherKeyPress));
        let space = &preset.rules[1];
        assert!(!space.is_enabled());
        assert_eq!(space.tap_action, None);
        assert_eq!(space.hold_action, None);
        assert_eq!(space.hold_behavior(), HoldBehavior::None);
        assert_eq!(space.condition_game_mode.as_deref(), Some("on"));
        assert_eq!(space.condition_layouts, Some(vec!["us".to_string()]));
        assert_eq!(
            space.condition_apps_whitelist,
            Some(vec!["code".to_string()])
        );
        assert_eq!(
            space.condition_apps_blacklist,
            Some(vec!["title:x".to_string()])
        );
        let chord = &preset.rules[2];
        assert_eq!(chord.hold_behavior(), HoldBehavior::Action);
        assert_eq!(chord.long_hold_action, "text:hello");
        assert_eq!(chord.long_hold_timeout_ms, Some(1500));
        assert_eq!(chord.hold_action.as_deref(), Some(""));
        assert_eq!(round_trip(&preset), preset);
        // The mapper reads the model JSON.
        let mapper: crate::mapper::config::Rule =
            serde_json::from_value(serde_json::to_value(&preset.rules[0]).unwrap()).unwrap();
        assert_eq!(mapper.tap_decision, Some(TapDecision::HoldOnOtherKeyPress));
    }

    #[test]
    fn only_the_selected_hold_behaviour_is_saved() {
        let mut rule = LayerRule::new("r1".into(), "KeyA");
        rule.layer_id = "nav".into();
        rule.long_hold_action = "KeyB".into();
        rule.hold_behavior = Some(HoldBehavior::Layer);
        let preset = LayoutPreset {
            rules: vec![rule],
            ..LayoutPreset::default()
        };
        let saved: Value = serde_json::from_str(&serialize(&preset)).unwrap();
        assert_eq!(saved["rules"][0]["hold"]["layer"], "nav");
        assert!(saved["rules"][0].get("longHold").is_none());
        let again = round_trip(&preset);
        assert_eq!(again.rules[0].hold_behavior(), HoldBehavior::Layer);
        assert_eq!(again.rules[0].long_hold_action, "");
    }

    #[test]
    fn layers_macros_commands_and_menus_round_trip() {
        let preset = parsed(json!({
            "description": "Sample",
            "layers": [
                {"id": "nav", "name": "Navigation", "description": "Arrows",
                 "extraKeyOrder": ["F15", "F13"],
                 "keys": {"KeyH": "ArrowLeft", "KeyQ": null, "Tab": "Alt+Tab"},
                 "holdModifiers": ["Tab"]},
                {"id": "sym"}
            ],
            "macros": [{"id": "copyLine", "name": "Copy line", "stepPauseMs": 10,
                        "steps": ["Home", "Shift+End", "Ctrl+KeyC"]}],
            "commands": [{"id": "music", "unix": "playerctl play-pause", "windows": "Start-Process x",
                          "workingDirectory": "/tmp"}],
            "quickActionPages": [{"id": "qap1", "name": "First", "cells": {
                "KeyW": "Ctrl+KeyC",
                "KeyQ": {"name": "Lock", "action": "kde:ksmserver/Lock Session", "icon": "lock"}
            }}, {"id": "qap2"}],
            "emojiPages": [{"id": "e1", "name": "Faces", "cells": {"KeyQ": "😀"}}]
        }));
        assert_eq!(preset.description.as_deref(), Some("Sample"));
        assert_eq!(preset.layers[1].name, "sym");
        let nav = &preset.layer_keymaps["nav"];
        assert_eq!(nav.keys["KeyH"].as_deref(), Some("ArrowLeft"));
        assert_eq!(nav.keys["KeyQ"], None);
        assert_eq!(nav.extra_key_order, ["F15", "F13"]);
        assert_eq!(nav.hold_modifiers, BTreeSet::from(["Tab".to_owned()]));
        assert_eq!(preset.macros[0].steps.len(), 3);
        assert_eq!(preset.commands[0].linux, "playerctl play-pause");
        assert_eq!(preset.commands[0].windows, "Start-Process x");
        let quick = &preset.quick_action_pages[0];
        assert_eq!(quick.cells["KeyQ"].icon.as_deref(), Some("lock"));
        assert_eq!(quick.cells["KeyW"].name, "");
        let again = round_trip(&preset);
        assert_eq!(again.layers, preset.layers);
        assert_eq!(again.layer_keymaps, preset.layer_keymaps);
        assert_eq!(again.commands, preset.commands);
        assert_eq!(again.quick_action_pages, preset.quick_action_pages);
        assert_eq!(again.emoji_pages, preset.emoji_pages);
        let text = serialize(&preset);
        assert!(
            text.find("\"KeyQ\": {").unwrap() < text.find("\"KeyW\": \"Ctrl+KeyC\"").unwrap(),
            "cells are written in grid order"
        );
    }

    #[test]
    fn transient_editor_entries_are_not_saved() {
        let mut rule = LayerRule::new("r1".into(), "");
        rule.tap_action = Some("KeyA".into());
        let mut preset = LayoutPreset::initial();
        preset.rules.push(rule);
        preset.macros.push(Macro {
            id: "m".into(),
            name: "m".into(),
            steps: vec![MacroStep {
                id: "s".into(),
                action: " ".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        // A key that lost its action keeps no holdModifiers entry.
        preset.layers.push(Layer {
            id: "nav".into(),
            name: "nav".into(),
            description: None,
        });
        let keymap = preset.layer_keymap_mut("nav");
        keymap.keys.insert("KeyQ".into(), None);
        keymap.hold_modifiers.insert("KeyQ".into());
        keymap.hold_modifiers.insert("Tab".into());
        let again = round_trip(&preset);
        assert!(again.rules.is_empty());
        assert!(again.macros[0].steps.is_empty());
        assert!(again.layer_keymaps["nav"].hold_modifiers.is_empty());
    }

    #[test]
    fn missing_menu_pages_get_one_empty_page_and_are_not_written_back() {
        let preset = parsed(json!({}));
        assert_eq!(preset.emoji_pages, vec![EmojiPage::default_page()]);
        assert_eq!(
            preset.quick_action_pages,
            vec![QuickActionPage::default_page()]
        );
        assert_eq!(serialize(&preset), "{\n  \"schemaVersion\": 1\n}");
        assert_eq!(parsed(json!({"emojiPages": []})), preset);
    }

    #[test]
    fn parsing_is_deterministic_without_ids() {
        let text = layout(
            json!({"rules": [{"key": "CapsLock"}], "quickActionPages": [{"cells": {"KeyQ": "KeyA"}}]}),
        );
        assert_eq!(parse(&text), parse(&text));
    }

    #[test]
    fn empty_and_invalid_documents() {
        assert_eq!(parse("").unwrap(), None);
        assert!(parse(r#""just text""#).is_err());
        assert!(parse("{\"layers\": [").is_err());
        assert!(parse("layers: []\nrules: []\n").is_err());
    }
}
