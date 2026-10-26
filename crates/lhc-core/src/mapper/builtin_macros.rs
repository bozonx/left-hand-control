//! Built-in macros — globally available macros that resolve
//! `builtin:<id>` at runtime regardless of the active layout preset.
//!
//! Built-in and user macros have independent action namespaces.

pub struct BuiltinMacro {
    pub id: &'static str,
    /// English display name.
    pub name: &'static str,
    pub steps: &'static [&'static str],
}

pub const BUILTIN_MACROS: &[BuiltinMacro] = &[
    // nav layer
    BuiltinMacro {
        id: "emptyLineAbove",
        name: "Empty line above",
        steps: &["Home", "Enter", "ArrowUp"],
    },
    BuiltinMacro {
        id: "nextLineEnd",
        name: "End of next line",
        steps: &["ArrowDown", "End"],
    },
    BuiltinMacro {
        id: "prevLineEnd",
        name: "End of previous line",
        steps: &["ArrowUp", "End"],
    },
    BuiltinMacro {
        id: "up5Lines",
        name: "Move up 5 lines",
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
        id: "spaceAfterNextChar",
        name: "Space after next character",
        steps: &["ArrowRight", "Space"],
    },
    BuiltinMacro {
        id: "emptyLineBelow",
        name: "Empty line below",
        steps: &["End", "Enter"],
    },
    BuiltinMacro {
        id: "cutWordUnderCursor",
        name: "Cut word under cursor",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "copyWordUnderCursor",
        name: "Copy word under cursor",
        steps: &[
            "Ctrl+ArrowRight",
            "Ctrl+Shift+ArrowLeft",
            "Ctrl+KeyC",
            "ArrowLeft",
        ],
    },
    BuiltinMacro {
        id: "pasteOnNewLineAbove",
        name: "Paste on new line above",
        steps: &["Home", "Enter", "ArrowUp", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceWordWithClipboard",
        name: "Replace word with clipboard",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "nextLineStart",
        name: "Start of next line",
        steps: &["ArrowDown", "Home"],
    },
    BuiltinMacro {
        id: "prevLineStart",
        name: "Start of previous line",
        steps: &["ArrowUp", "Home"],
    },
    BuiltinMacro {
        id: "down5Lines",
        name: "Move down 5 lines",
        steps: &[
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
        ],
    },
    BuiltinMacro {
        id: "pasteOnNewLineBelow",
        name: "Paste on new line below",
        steps: &["End", "Enter", "Ctrl+KeyV"],
    },
    // select layer
    BuiltinMacro {
        id: "cutToLineStart",
        name: "Cut to line start",
        steps: &["Shift+Home", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutToLineEnd",
        name: "Cut to line end",
        steps: &["Shift+End", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutLineText",
        name: "Cut line text",
        steps: &["Home", "Shift+End", "Ctrl+KeyX"],
    },
    BuiltinMacro {
        id: "cutWholeLine",
        name: "Cut whole line",
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
        id: "copyToLineStart",
        name: "Copy to line start",
        steps: &["Shift+Home", "Ctrl+KeyC", "Home"],
    },
    BuiltinMacro {
        id: "copyToLineEnd",
        name: "Copy to line end",
        steps: &["Shift+End", "Ctrl+KeyC", "End"],
    },
    BuiltinMacro {
        id: "copyLineText",
        name: "Copy line text",
        steps: &["Home", "Shift+End", "Ctrl+KeyC", "Home"],
    },
    BuiltinMacro {
        id: "selectLineText",
        name: "Select line text",
        steps: &["Home", "Shift+End"],
    },
    BuiltinMacro {
        id: "selectWordUnderCursor",
        name: "Select word under cursor",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft"],
    },
    BuiltinMacro {
        id: "replaceToLineStartWithClipboard",
        name: "Replace to line start with clipboard",
        steps: &["Shift+Home", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceToLineEndWithClipboard",
        name: "Replace to line end with clipboard",
        steps: &["Shift+End", "Ctrl+KeyV"],
    },
    BuiltinMacro {
        id: "replaceLineTextWithClipboard",
        name: "Replace line text with clipboard",
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

/// IDs used before the built-in macros were renamed, mapped to the current
/// ones so existing configs and layouts keep working.
const LEGACY_IDS: &[(&str, &str)] = &[
    ("moveLineDown", "emptyLineAbove"),
    ("downEnd", "nextLineEnd"),
    ("upEnd", "prevLineEnd"),
    ("up5Times", "up5Lines"),
    ("rightSpace", "spaceAfterNextChar"),
    ("cutWordRightCenter", "cutWordUnderCursor"),
    ("copyWordAfterCenter", "copyWordUnderCursor"),
    ("pasteAtLineAbove", "pasteOnNewLineAbove"),
    ("replaceWordWidthBuffer", "replaceWordWithClipboard"),
    ("downHome", "nextLineStart"),
    ("upHome", "prevLineStart"),
    ("down5Times", "down5Lines"),
    ("pasteAtLineBottom", "pasteOnNewLineBelow"),
    ("cutToStart", "cutToLineStart"),
    ("cutToEnd", "cutToLineEnd"),
    ("cutLineContent", "cutLineText"),
    ("cutAndRemoveLine", "cutWholeLine"),
    ("copyToStart", "copyToLineStart"),
    ("copyToEnd", "copyToLineEnd"),
    ("copyLine", "copyLineText"),
    ("selectWholeLine", "selectLineText"),
    ("selectWordRightCenter", "selectWordUnderCursor"),
    (
        "replaceToStartWithBuffer",
        "replaceToLineStartWithClipboard",
    ),
    ("replaceToEndWithBuffer", "replaceToLineEndWithClipboard"),
    ("replaceLineWidthBuffer", "replaceLineTextWithClipboard"),
];

/// Current ID for `id`, translating a legacy built-in macro ID.
pub fn canonical_id(id: &str) -> &str {
    LEGACY_IDS
        .iter()
        .find(|(old, _)| *old == id)
        .map_or(id, |(_, new)| new)
}

#[cfg(all(test, target_os = "linux"))]
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
