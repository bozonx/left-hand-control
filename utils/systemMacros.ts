// Catalog of built-in "system macros" shipped with the application.
//
// System macros are globally available regardless of the active layout:
// `macro:<id>` references resolve against user macros first and fall back
// to this catalog. The user cannot edit a system macro directly; the UI
// offers a "Create user copy" action that clones a system macro into
// `config.macros` with a fresh id (shadowing the system one if the id is
// kept). Layout presets may still declare their own layout-specific
// macros in their YAML `macros:` block.
//
// IMPORTANT: the Rust side mirrors this catalog in
// `src-tauri/src/mapper/system_macros.rs`. Keep the two files in sync —
// same ids, same step strings, or references will resolve in the UI but
// silently fail at runtime (or vice-versa).

export interface SystemMacroStep {
  keystroke: string;
}

export interface SystemMacro {
  // Stable identifier used in actions as `macro:<id>`. Same namespace as
  // user macros — user macros with the same id override the system one.
  id: string;
  // Human-readable name.
  name: string;
  // Short description shown as a secondary label in the UI.
  description?: string;
  // Ordered list of keystroke steps.
  steps: SystemMacroStep[];
}

function s(...keystrokes: string[]): SystemMacroStep[] {
  return keystrokes.map((k) => ({ keystroke: k }));
}

export const SYSTEM_MACROS: SystemMacro[] = [
  {
    id: "moveLineDown",
    name: "Move line down",
    steps: s("Home", "Enter", "ArrowUp"),
  },
  { id: "downEnd", name: "Down + End", steps: s("ArrowDown", "End") },
  { id: "upEnd", name: "Up + End", steps: s("ArrowUp", "End") },
  {
    id: "up5Times",
    name: "Up 5 times",
    steps: s("ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp", "ArrowUp"),
  },
  {
    id: "duplicateLine",
    name: "Duplicate line",
    steps: s("End", "Shift+Home", "Ctrl+KeyC", "End", "Enter", "Ctrl+KeyV"),
  },
  { id: "rightSpace", name: "Right + Space", steps: s("ArrowRight", "Space") },
  { id: "emptyLineBelow", name: "Empty line below", steps: s("End", "Enter") },
  {
    id: "cutWordRightCenter",
    name: "Cut word right from center",
    steps: s("Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyX"),
  },
  {
    id: "copyWordAfterCenter",
    name: "Copy word right from center",
    steps: s("Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyC", "ArrowLeft"),
  },
  {
    id: "pasteAtLineAbove",
    name: "Paste at line above",
    steps: s("Home", "Enter", "ArrowUp", "Ctrl+KeyV"),
  },
  {
    id: "replaceWordWidthBuffer",
    name: "Replace word with buffer",
    steps: s("Ctrl+ArrowRight", "Ctrl+Shift+ArrowLeft", "Ctrl+KeyC"),
  },
  { id: "downHome", name: "Down + Home", steps: s("ArrowDown", "Home") },
  { id: "upHome", name: "Up + Home", steps: s("ArrowUp", "Home") },
  {
    id: "down5Times",
    name: "Down 5 times",
    steps: s("ArrowDown", "ArrowDown", "ArrowDown", "ArrowDown", "ArrowDown"),
  },
  {
    id: "pasteAtLineBottom",
    name: "Paste at line bottom",
    steps: s("End", "Enter", "Ctrl+KeyV"),
  },
];

const BY_ID: Record<string, SystemMacro> = Object.fromEntries(
  SYSTEM_MACROS.map((m) => [m.id, m]),
);

export function systemMacroById(id: string): SystemMacro | undefined {
  return BY_ID[id];
}
