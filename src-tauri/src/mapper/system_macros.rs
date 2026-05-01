// Built-in "system macros" — globally available macros that resolve
// `macro:<id>` at runtime regardless of the active layout preset.
//
// The engine builds these first, then overlays user macros from the
// config (same id → user wins). Keep this catalog in sync with
// `utils/systemMacros.ts` on the frontend.

#![cfg(target_os = "linux")]

pub struct SysMacro {
    pub id: &'static str,
    pub steps: &'static [&'static str],
}

pub const SYSTEM_MACROS: &[SysMacro] = &[
    // nav layer
    SysMacro {
        id: "moveLineDown",
        steps: &["Home", "Enter", "ArrowUp"],
    },
    SysMacro {
        id: "downEnd",
        steps: &["ArrowDown", "End"],
    },
    SysMacro {
        id: "upEnd",
        steps: &["ArrowUp", "End"],
    },
    SysMacro {
        id: "up5Times",
        steps: &["ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp"],
    },
    SysMacro {
        id: "duplicateLine",
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
        steps: &["ArrowRight", "Space"],
    },
    SysMacro {
        id: "emptyLineBelow",
        steps: &["End", "Enter"],
    },
    SysMacro {
        id: "cutWordRightCenter",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "copyWordAfterCenter",
        steps: &[
            "Ctrl+ArrowRight",
            "Ctrl+Shift+ArrowLeft",
            "Ctrl+KeyC",
            "ArrowLeft",
        ],
    },
    SysMacro {
        id: "pasteAtLineAbove",
        steps: &["Home", "Enter", "ArrowUp", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceWordWidthBuffer",
        steps: &["Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyC"],
    },
    SysMacro {
        id: "downHome",
        steps: &["ArrowDown", "Home"],
    },
    SysMacro {
        id: "upHome",
        steps: &["ArrowUp", "Home"],
    },
    SysMacro {
        id: "down5Times",
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
        steps: &["End", "Enter", "Ctrl+KeyV"],
    },
    // select layer
    SysMacro {
        id: "cutToStart",
        steps: &["Shift+Home", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutToEnd",
        steps: &["Shift+End", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutLineContent",
        steps: &["Home", "Shift+End", "Ctrl+KeyX"],
    },
    SysMacro {
        id: "cutAndRemoveLine",
        steps: &["Home", "Shift+End", "Ctrl+KeyX", "Delete"],
    },
    SysMacro {
        id: "select5LinesUp",
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
        steps: &["Shift+Home", "Ctrl+KeyC", "Home"],
    },
    SysMacro {
        id: "copyToEnd",
        steps: &["Shift+End", "Ctrl+KeyC", "End"],
    },
    SysMacro {
        id: "copyLine",
        steps: &["Home", "Shift+End", "Ctrl+KeyC", "Home"],
    },
    SysMacro {
        id: "selectWholeLine",
        steps: &["Home", "Shift+End"],
    },
    SysMacro {
        id: "replaceToStartWithBuffer",
        steps: &["Shift+Home", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceToEndWithBuffer",
        steps: &["Shift+End", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "replaceLineWidthBuffer",
        steps: &["Home", "Shift+End", "Ctrl+KeyV"],
    },
    SysMacro {
        id: "select5LinesDown",
        steps: &[
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
            "Shift+ArrowDown",
        ],
    },
];
