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
    SysMacro { id: "moveLineDown", steps: &["Home", "Enter", "Up"] },
    SysMacro { id: "ctrlBackspace", steps: &["Ctrl+Backspace"] },
    SysMacro { id: "ctrlDelete", steps: &["Ctrl+Delete"] },
    SysMacro { id: "ctrlHome", steps: &["Ctrl+Home"] },
    SysMacro { id: "downEnd", steps: &["Down", "End"] },
    SysMacro { id: "upEnd", steps: &["Up", "End"] },
    SysMacro { id: "up5Times", steps: &["Up", "Up", "Up", "Up", "Up"] },
    SysMacro {
        id: "duplicateLine",
        steps: &["End", "Shift+Home", "Ctrl+C", "End", "Enter", "Ctrl+V"],
    },
    SysMacro { id: "ctrlD", steps: &["Ctrl+KeyD"] },
    SysMacro { id: "ctrlEnd", steps: &["Ctrl+End"] },
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
    SysMacro { id: "ctrlLeft", steps: &["Ctrl+Left"] },
    SysMacro { id: "downHome", steps: &["Down", "Home"] },
    SysMacro { id: "upHome", steps: &["Up", "Home"] },
    SysMacro { id: "ctrlRight", steps: &["Ctrl+Right"] },
    SysMacro {
        id: "down5Times",
        steps: &["Down", "Down", "Down", "Down", "Down"],
    },
    SysMacro { id: "toDesktop1", steps: &["sys:switchDesktop1"] },
];
