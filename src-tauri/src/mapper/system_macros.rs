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
    SysMacro { id: "moveLineDown", steps: &["Home", "Enter", "Up"] },
    SysMacro { id: "downEnd", steps: &["Down", "End"] },
    SysMacro { id: "upEnd", steps: &["Up", "End"] },
    SysMacro { id: "up5Times", steps: &["Up", "Up", "Up", "Up", "Up"] },
    SysMacro {
        id: "duplicateLine",
        steps: &["End", "Shift+Home", "Ctrl+C", "End", "Enter", "Ctrl+V"],
    },
    SysMacro { id: "rightSpace", steps: &["Right", "Space"] },
    SysMacro { id: "emptyLineBelow", steps: &["End", "Enter"] },
    SysMacro {
        id: "cutWordRightCenter",
        steps: &["Ctrl+Right", "Ctrl+Shift+Left", "Ctrl+X"],
    },
    SysMacro {
        id: "copyWordAfterCenter",
        steps: &["Ctrl+Right", "Ctrl+Shift+Left", "Ctrl+C", "Left"],
    },
    SysMacro {
        id: "pasteAtLineAbove",
        steps: &["Home", "Enter", "Up", "Ctrl+V"],
    },
    SysMacro {
        id: "replaceWordWidthBuffer",
        steps: &["Ctrl+Right", "Ctrl+Shift+Left", "Ctrl+C"],
    },
    SysMacro { id: "downHome", steps: &["Down", "Home"] },
    SysMacro { id: "upHome", steps: &["Up", "Home"] },
    SysMacro {
        id: "down5Times",
        steps: &["Down", "Down", "Down", "Down", "Down"],
    },
    SysMacro {
        id: "pasteAtLineBottom",
        steps: &["End", "Enter", "Ctrl+V"],
    },

    // select layer
    SysMacro { id: "cutToStart", steps: &["Shift+Home", "Ctrl+X"] },
    SysMacro { id: "cutToEnd", steps: &["Shift+End", "Ctrl+X"] },
    SysMacro { id: "cutLineContent", steps: &["Home", "Shift+End", "Ctrl+X"] },
    SysMacro { id: "cutAndRemoveLine", steps: &["Home", "Shift+End", "Ctrl+X", "Delete"] },
    SysMacro { id: "select5LinesUp", steps: &["Shift+Up", "Shift+Up", "Shift+Up", "Shift+Up", "Shift+Up"] },

    SysMacro { id: "copyToStart", steps: &["Shift+Home", "Ctrl+C", "Home"] },
    SysMacro { id: "copyToEnd", steps: &["Shift+End", "Ctrl+C", "End"] },
    SysMacro { id: "copyLine", steps: &["Home", "Shift+End", "Ctrl+C", "Home"] },
    SysMacro { id: "selectWholeLine", steps: &["Home", "Shift+End"] },

    SysMacro { id: "replaceToStartWithBuffer", steps: &["Shift+Home", "Ctrl+V"] },
    SysMacro { id: "replaceToEndWithBuffer", steps: &["Shift+End", "Ctrl+V"] },
    SysMacro { id: "replaceLineWidthBuffer", steps: &["Home", "Shift+End", "Ctrl+V"] },
    SysMacro { id: "select5LinesDown", steps: &["Shift+Down", "Shift+Down", "Shift+Down", "Shift+Down", "Shift+Down"] },

];
