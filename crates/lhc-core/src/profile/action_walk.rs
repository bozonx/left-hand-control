//! Typed traversal of action fields, shared by references and edits.

use super::model::{LayoutPreset, ResolvedAppConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuleActionField {
    Tap,
    Hold,
    LongHold,
    DoubleTap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionLocation<'a> {
    Rule {
        index: usize,
        field: RuleActionField,
    },
    Layer {
        layer: &'a str,
        key: &'a str,
    },
    Macro {
        index: usize,
        step: usize,
    },
    QuickAction {
        page: usize,
        key: &'a str,
    },
}

// Generate read and edit traversal from the same field list. Metadata, command
// scripts and emoji text must never become references just by resembling actions.
macro_rules! walk {
    ($layout:ident, $visit:ident $(, $mut:tt)?) => {
        for (index, rule) in (& $($mut)? $layout.rules).into_iter().enumerate() {
            for (field, action) in [
                (RuleActionField::Tap, & $($mut)? rule.tap_action),
                (RuleActionField::Hold, & $($mut)? rule.hold_action),
            ] {
                if let Some(action) = action {
                    $visit(ActionLocation::Rule { index, field }, action);
                }
            }
            $visit(ActionLocation::Rule { index, field: RuleActionField::LongHold }, & $($mut)? rule.long_hold_action);
            $visit(ActionLocation::Rule { index, field: RuleActionField::DoubleTap }, & $($mut)? rule.double_tap_action);
        }
        for (layer, map) in & $($mut)? $layout.layer_keymaps {
            for (key, action) in & $($mut)? map.keys {
                if let Some(action) = action {
                    $visit(ActionLocation::Layer { layer, key }, action);
                }
            }
        }
        for (index, item) in (& $($mut)? $layout.macros).into_iter().enumerate() {
            for (step, item) in (& $($mut)? item.steps).into_iter().enumerate() {
                $visit(ActionLocation::Macro { index, step }, & $($mut)? item.action);
            }
        }
        for (page, item) in (& $($mut)? $layout.quick_action_pages).into_iter().enumerate() {
            for (key, cell) in & $($mut)? item.cells {
                $visit(ActionLocation::QuickAction { page, key }, & $($mut)? cell.action);
            }
        }
    };
}

macro_rules! read_actions {
    ($model:ty) => {
        impl $model {
            pub(crate) fn for_each_action(&self, mut visit: impl FnMut(ActionLocation<'_>, &str)) {
                walk!(self, visit);
            }
        }
    };
}
read_actions!(LayoutPreset);
read_actions!(ResolvedAppConfig);

impl LayoutPreset {
    /// Visit every action string: rule actions, layer keys, macro steps and
    /// quick-action cells. Absent actions and metadata are skipped.
    pub fn for_each_action_mut(&mut self, mut fix: impl FnMut(&mut String)) {
        let mut visit = |_, action: &mut String| fix(action);
        walk!(self, visit, mut);
    }
}

#[cfg(test)]
mod tests {
    use crate::profile::model::*;

    #[test]
    fn edits_and_reads_visit_all_action_fields_and_skip_metadata() {
        let mut layout = LayoutPreset::initial();
        layout.description = Some("cmd:metadata".into());
        let mut rule = KeyRule::new("r".into(), "KeyA");
        rule.tap_action = Some("cmd:tap".into());
        rule.hold_action = Some("cmd:hold".into());
        rule.long_hold_action = "cmd:long".into();
        rule.double_tap_action = "cmd:double".into();
        layout.rules.push(rule);
        layout
            .layer_keymaps
            .entry("l".into())
            .or_default()
            .keys
            .extend([
                ("KeyA".into(), Some("cmd:layer".into())),
                ("KeyB".into(), None),
            ]);
        layout.macros.push(Macro {
            id: "m".into(),
            name: "cmd:metadata".into(),
            steps: vec![MacroStep {
                id: "s".into(),
                action: "cmd:macro".into(),
            }],
            step_pause_ms: None,
            modifier_delay_ms: None,
        });
        layout.quick_action_pages[0].cells.clear();
        layout.quick_action_pages[0].cells.insert(
            "KeyA".into(),
            QuickAction {
                action: "cmd:quick".into(),
                name: "cmd:metadata".into(),
                ..Default::default()
            },
        );
        let mut before = Vec::new();
        layout.for_each_action(|location, action| {
            before.push((format!("{location:?}"), action.to_owned()))
        });
        assert_eq!(before.len(), 7);
        layout.for_each_action_mut(|action| action.push_str("_edited"));
        let mut after = Vec::new();
        layout.for_each_action(|location, action| {
            after.push((format!("{location:?}"), action.to_owned()))
        });
        for ((place, old), (new_place, new)) in before.iter().zip(&after) {
            assert_eq!(place, new_place);
            assert_eq!(new, &format!("{old}_edited"));
        }
        let config = ResolvedAppConfig::from_parts(AppSettings::default(), layout.clone(), None);
        let mut resolved = Vec::new();
        config.for_each_action(|location, action| {
            resolved.push((format!("{location:?}"), action.to_owned()))
        });
        assert_eq!(resolved, after);
        layout.commands = [
            "tap_edited",
            "hold_edited",
            "long_edited",
            "double_edited",
            "layer_edited",
            "macro_edited",
            "quick_edited",
            "metadata",
        ]
        .map(|id| Command {
            id: id.into(),
            name: "cmd:metadata".into(),
            posix_script: "cmd:metadata".into(),
            powershell_script: String::new(),
            working_directory: None,
        })
        .to_vec();
        assert_eq!(layout.used_command_count(), 7);
        layout.remove_unused_commands();
        assert_eq!(layout.commands.len(), 7);
        assert!(
            layout
                .commands
                .iter()
                .all(|command| command.id != "metadata")
        );
    }
}
