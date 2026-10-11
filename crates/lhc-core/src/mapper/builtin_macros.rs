// Built-in macros — globally available macros that resolve
// `macro:<id>` at runtime regardless of the active layout preset.
//
// The engine builds these first, then overlays user macros from the
// config (same id → user wins). The action catalog derives its entries
// from these definitions.

pub struct BuiltinMacro {
    pub id: &'static str,
    /// English display name.
    pub name: &'static str,
    pub steps: &'static [&'static str],
}

pub const BUILTIN_MACROS: &[BuiltinMacro] = &[
    // nav layer
    BuiltinMacro {
        id: "moveLineDown",
        name: "Move line down",
        steps: &["Home", "Enter", "ArrowUp"],
    },
    BuiltinMacro {
        id: "downEnd",
        name: "Down + End",
        steps: &["ArrowDown", "End"],
    },
    BuiltinMacro {
        id: "upEnd",
        name: "Up + End",
        steps: &["ArrowUp", "End"],
    },
    BuiltinMacro {
        id: "up5Times",
        name: "Up 5 times",
        steps: &["ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp"],
    },
    BuiltinMacro {
        id: "duplicateLine",
        name: "Duplicate line",
        steps: &[
            "End",
            "Shift+Home",
            "Ctrl+KeyC",
            "End",
            "Enter",
            "Ctrl+KeyV",
        ],
    },
    BuiltinMacro {
        id: "rightSpace",
        name: "Right + Space",
        steps: &["ArrowRight", "Space"],
    },
    BuiltinMacro {
        id: "emptyLineBelow",
        name: "Empty line below",
        steps: &["End", "Enter"],
    },
    BuiltinMacro {
        id: "cutWordRightCenter",
        name: "Cut word right from center",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "copyWordAfterCenter",
        name: "Copy word right from center",
        steps: &[
            "Ctrl+ArrowRight",
            "Ctrl+Shift+ArrowLeft",
            "Ctrl+KeyC",
            "ArrowLeft",
        ],
    },
    BuiltinMacro {
        id: "pasteAtLineAbove",
        name: "Paste at line above",
        steps: &["Home", "Enter", "ArrowUp", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceWordWidthBuffer",
        name: "Replace word with buffer",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyC"],
    },
    BuiltinMacro {
        id: "downHome",
        name: "Down + Home",
        steps: &["ArrowDown", "Home"],
    },
    BuiltinMacro {
        id: "upHome",
        name: "Up + Home",
        steps: &["ArrowUp", "Home"],
    },
    BuiltinMacro {
        id: "down5Times",
        name: "Down 5 times",
        steps: &[
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
        ],
    },
    BuiltinMacro {
        id: "pasteAtLineBottom",
        name: "Paste at line bottom",
        steps: &["End", "Enter", "Ctrl+KeyV"],
    },
    // select layer
    BuiltinMacro {
        id: "cutToStart",
        name: "Cut to start",
        steps: &["Shift+Home", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutToEnd",
        name: "Cut to end",
        steps: &["Shift+End", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutLineContent",
        name: "Cut line content",
        steps: &["Home", "Shift+End", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutAndRemoveLine",
        name: "Cut and remove line",
        steps: &["Home", "Shift+End", "Ctrl+KeyX", "Delete"],
    },
    BuiltinMacro {
        id: "select5LinesUp",
        name: "Select 5 lines up",
        steps: &[
            "Shift+ArrowUp",
            "Shift+ArrowUp",
            "Shift+ArrowUp",
            "Shift+ArrowUp",
            "Shift+ArrowUp",
        ],
    },
    BuiltinMacro {
        id: "copyToStart",
        name: "Copy to start",
        steps: &["Shift+Home", "Ctrl+KeyC", "Home"],
    },
    BuiltinMacro {
        id: "copyToEnd",
        name: "Copy to end",
        steps: &["Shift+End", "Ctrl+KeyC", "End"],
    },
    BuiltinMacro {
        id: "copyLine",
        name: "Copy line",
        steps: &["Home", "Shift+End", "Ctrl+KeyC", "Home"],
    },
    BuiltinMacro {
        id: "selectWholeLine",
        name: "Select whole line",
        steps: &["Home", "Shift+End"],
    },
    BuiltinMacro {
        id: "selectWordRightCenter",
        name: "Select word right from center",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft"],
    },
    BuiltinMacro {
        id: "replaceToStartWithBuffer",
        name: "Replace to start with buffer",
        steps: &["Shift+Home", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceToEndWithBuffer",
        name: "Replace to end with buffer",
        steps: &["Shift+End", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceLineWidthBuffer",
        name: "Replace line with buffer",
        steps: &["Home", "Shift+End", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "select5LinesDown",
        name: "Select 5 lines down",
        steps: &[
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::BUILTIN_MACROS;
    use crate::mapper::action::parse_action;

    #[test]
    fn all_builtin_macros_have_valid_keystrokes() {
        for m in BUILTIN_MACROS {
            for step in m.steps {
                assert!(
                    parse_action(step).is_some(),
                    "built-in macro {} has invalid step: {}",
                    m.id,
                    step
                );
            }
        }
    }

    #[test]
    fn all_builtin_macro_ids_are_non_empty_and_unique() {
        let mut ids = std::collections::HashSet::new();
        for m in BUILTIN_MACROS {
            assert!(!m.id.is_empty(), "built-in macro id cannot be empty");
            assert!(ids.insert(m.id), "duplicate built-in macro id: {}", m.id);
        }
    }
}
