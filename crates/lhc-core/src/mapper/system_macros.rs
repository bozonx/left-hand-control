// Built-in "system macros" — globally available macros that resolve
// `macro:<id>` at runtime regardless of the active layout preset.
//
// The engine builds these first, then overlays user macros from the
// config (same id → user wins). Keep this catalog in sync with
// `utils/systemMacros.ts` on the frontend.

pub struct SysMacro {
    pub id: &'static str,
    /// English display name.
    pub name: &'static str,
    pub steps: &'static [&'static str],
}

pub const SYSTEM_MACROS: &[SysMacro] = &[
    // nav layer
    SysMacro {
        id: "moveLineDown",
        name: "Move line down",
        steps: &["Home", "Enter", "ArrowUp"],
    },
    SysMacro {
        id: "downEnd",
        name: "Down + End",
        steps: &["ArrowDown", "End"],
    },
    SysMacro {
        id: "upEnd",
        name: "Up + End",
        steps: &["ArrowUp", "End"],
    },
    SysMacro {
        id: "up5Times",
        name: "Up 5 times",
        steps: &["ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp"],
    },
    SysMacro {
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
    SysMacro {
        id: "rightSpace",
        name: "Right + Space",
        steps: &["ArrowRight", "Space"],
    },
    SysMacro {
        id: "emptyLineBelow",
        name: "Empty line below",
        steps: &["End", "Enter"],
    },
    SysMacro {
        id: "cutWordRightCenter",
        name: "Cut word right from center",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "copyWordAfterCenter",
        name: "Copy word right from center",
        steps: &[
            "Ctrl+ArrowRight",
            "Ctrl+Shift+ArrowLeft",
            "Ctrl+KeyC",
            "ArrowLeft",
        ],
    },
    SysMacro {
        id: "pasteAtLineAbove",
        name: "Paste at line above",
        steps: &["Home", "Enter", "ArrowUp", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceWordWidthBuffer",
        name: "Replace word with buffer",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyC"],
    },
    SysMacro {
        id: "downHome",
        name: "Down + Home",
        steps: &["ArrowDown", "Home"],
    },
    SysMacro {
        id: "upHome",
        name: "Up + Home",
        steps: &["ArrowUp", "Home"],
    },
    SysMacro {
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
    SysMacro {
        id: "pasteAtLineBottom",
        name: "Paste at line bottom",
        steps: &["End", "Enter", "Ctrl+KeyV"],
    },
    // select layer
    SysMacro {
        id: "cutToStart",
        name: "Cut to start",
        steps: &["Shift+Home", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutToEnd",
        name: "Cut to end",
        steps: &["Shift+End", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutLineContent",
        name: "Cut line content",
        steps: &["Home", "Shift+End", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutAndRemoveLine",
        name: "Cut and remove line",
        steps: &["Home", "Shift+End", "Ctrl+KeyX", "Delete"],
    },
    SysMacro {
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
    SysMacro {
        id: "copyToStart",
        name: "Copy to start",
        steps: &["Shift+Home", "Ctrl+KeyC", "Home"],
    },
    SysMacro {
        id: "copyToEnd",
        name: "Copy to end",
        steps: &["Shift+End", "Ctrl+KeyC", "End"],
    },
    SysMacro {
        id: "copyLine",
        name: "Copy line",
        steps: &["Home", "Shift+End", "Ctrl+KeyC", "Home"],
    },
    SysMacro {
        id: "selectWholeLine",
        name: "Select whole line",
        steps: &["Home", "Shift+End"],
    },
    SysMacro {
        id: "selectWordRightCenter",
        name: "Select word right from center",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft"],
    },
    SysMacro {
        id: "replaceToStartWithBuffer",
        name: "Replace to start with buffer",
        steps: &["Shift+Home", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceToEndWithBuffer",
        name: "Replace to end with buffer",
        steps: &["Shift+End", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceLineWidthBuffer",
        name: "Replace line with buffer",
        steps: &["Home", "Shift+End", "Ctrl+KeyV"],
    },
    SysMacro {
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
    use super::SYSTEM_MACROS;
    use crate::mapper::action::parse_action;

    #[test]
    fn all_system_macros_have_valid_keystrokes() {
        for m in SYSTEM_MACROS {
            for step in m.steps {
                assert!(
                    parse_action(step).is_some(),
                    "system macro {} has invalid step: {}",
                    m.id,
                    step
                );
            }
        }
    }

    #[test]
    fn all_system_macro_ids_are_non_empty_and_unique() {
        let mut ids = std::collections::HashSet::new();
        for m in SYSTEM_MACROS {
            assert!(!m.id.is_empty(), "system macro id cannot be empty");
            assert!(ids.insert(m.id), "duplicate system macro id: {}", m.id);
        }
    }
}
