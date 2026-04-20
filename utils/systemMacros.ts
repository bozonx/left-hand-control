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
  keystroke: string
}

export interface SystemMacro {
  // Stable identifier used in actions as `macro:<id>`. Same namespace as
  // user macros — user macros with the same id override the system one.
  id: string
  // Human-readable name.
  name: string
  // Short description shown as a secondary label in the UI.
  description?: string
  // Ordered list of keystroke steps.
  steps: SystemMacroStep[]
}

function s(...keystrokes: string[]): SystemMacroStep[] {
  return keystrokes.map((k) => ({ keystroke: k }))
}

export const SYSTEM_MACROS: SystemMacro[] = [
  { id: 'moveLineDown', name: 'Move line down', steps: s('Home', 'Enter', 'Up') },
  { id: 'ctrlBackspace', name: 'Ctrl + Backspace', steps: s('Ctrl+Backspace') },
  { id: 'ctrlDelete', name: 'Ctrl + Delete', steps: s('Ctrl+Delete') },
  { id: 'ctrlHome', name: 'Ctrl + Home', steps: s('Ctrl+Home') },
  { id: 'downEnd', name: 'Down + End', steps: s('Down', 'End') },
  { id: 'upEnd', name: 'Up + End', steps: s('Up', 'End') },
  { id: 'up5Times', name: 'Up 5 times', steps: s('Up', 'Up', 'Up', 'Up', 'Up') },
  {
    id: 'duplicateLine',
    name: 'Duplicate line',
    steps: s('End', 'Shift+Home', 'Ctrl+C', 'End', 'Enter', 'Ctrl+V'),
  },
  { id: 'ctrlD', name: 'Ctrl + d', steps: s('Ctrl+KeyD') },
  { id: 'ctrlEnd', name: 'Ctrl + End', steps: s('Ctrl+End') },
  { id: 'rightSpace', name: 'Right + Space', steps: s('Right', 'Space') },
  { id: 'emptyLineBelow', name: 'Empty line below', steps: s('End', 'Enter') },
  {
    id: 'cutWordRightCenter',
    name: 'Cut word right from center',
    steps: s('Ctrl+Right', 'Ctrl+Shift+Left', 'Ctrl+X'),
  },
  {
    id: 'copyWordAfterCenter',
    name: 'Copy word right from center',
    steps: s('Ctrl+Right', 'Ctrl+Shift+Left', 'Ctrl+C', 'Left'),
  },
  {
    id: 'pasteAtLineAbove',
    name: 'Paste at line above',
    steps: s('Home', 'Enter', 'Up', 'Ctrl+V'),
  },
  {
    id: 'replaceWordWidthBuffer',
    name: 'Replace word with buffer',
    steps: s('Ctrl+Right', 'Ctrl+Shift+Left', 'Ctrl+C'),
  },
  { id: 'ctrlLeft', name: 'Ctrl + Left', steps: s('Ctrl+Left') },
  { id: 'downHome', name: 'Down + Home', steps: s('Down', 'Home') },
  { id: 'upHome', name: 'Up + Home', steps: s('Up', 'Home') },
  { id: 'ctrlRight', name: 'Ctrl + Right', steps: s('Ctrl+Right') },
  {
    id: 'down5Times',
    name: 'Down 5 times',
    steps: s('Down', 'Down', 'Down', 'Down', 'Down'),
  },
  { id: 'toDesktop1', name: 'To Desktop 1', steps: s('sys:switchDesktop1') },
]

const BY_ID: Record<string, SystemMacro> = Object.fromEntries(
  SYSTEM_MACROS.map((m) => [m.id, m]),
)

export function systemMacroById(id: string): SystemMacro | undefined {
  return BY_ID[id]
}
