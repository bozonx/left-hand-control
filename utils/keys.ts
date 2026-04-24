// Physical key definitions used across the Rules and Keymap tabs.
//
// Layout uses a flat grid (NOT a realistic keyboard layout) split into
// "left hand" and "right hand" halves. Keys are identified by stable codes
// (roughly following KeyboardEvent.code naming for letters/digits, plus
// explicit names for modifiers / function keys).

export interface KeyDef {
  // Stable code used as map key in LayerKeymap.keys.
  code: string
  // Label shown on the cap (English).
  label: string
}

export type KeyLabelMode = 'label' | 'code' | 'numeric'

const KEY_NUMERIC_CODES: Record<string, number> = {
  Escape: 1,
  Digit1: 2,
  Digit2: 3,
  Digit3: 4,
  Digit4: 5,
  Digit5: 6,
  Digit6: 7,
  Digit7: 8,
  Digit8: 9,
  Digit9: 10,
  Digit0: 11,
  Minus: 12,
  Equal: 13,
  Backspace: 14,
  Tab: 15,
  KeyQ: 16,
  KeyW: 17,
  KeyE: 18,
  KeyR: 19,
  KeyT: 20,
  KeyY: 21,
  KeyU: 22,
  KeyI: 23,
  KeyO: 24,
  KeyP: 25,
  BracketLeft: 26,
  BracketRight: 27,
  Enter: 28,
  ControlLeft: 29,
  KeyA: 30,
  KeyS: 31,
  KeyD: 32,
  KeyF: 33,
  KeyG: 34,
  KeyH: 35,
  KeyJ: 36,
  KeyK: 37,
  KeyL: 38,
  Semicolon: 39,
  Quote: 40,
  Backquote: 41,
  ShiftLeft: 42,
  Backslash: 43,
  KeyZ: 44,
  KeyX: 45,
  KeyC: 46,
  KeyV: 47,
  KeyB: 48,
  KeyN: 49,
  KeyM: 50,
  Comma: 51,
  Period: 52,
  Slash: 53,
  ShiftRight: 54,
  AltLeft: 56,
  Space: 57,
  CapsLock: 58,
  F1: 59,
  F2: 60,
  F3: 61,
  F4: 62,
  F5: 63,
  F6: 64,
  F7: 65,
  F8: 66,
  F9: 67,
  F10: 68,
  F11: 87,
  F12: 88,
  ControlRight: 97,
  AltRight: 100,
  MetaLeft: 125,
  MetaRight: 126,
  ContextMenu: 127,
  PrintScreen: 99,
}

function row(codes: Array<[string, string]>): KeyDef[] {
  return codes.map(([code, label]) => ({ code, label }))
}

export const LEFT_HAND_ROWS: KeyDef[][] = [
  row([
    ['Escape', 'Esc'],
    ['F1', 'F1'],
    ['F2', 'F2'],
    ['F3', 'F3'],
    ['F4', 'F4'],
    ['F5', 'F5'],
  ]),
  row([
    ['Backquote', '`'],
    ['Digit1', '1'],
    ['Digit2', '2'],
    ['Digit3', '3'],
    ['Digit4', '4'],
    ['Digit5', '5'],
  ]),
  row([
    ['Tab', 'Tab'],
    ['KeyQ', 'Q'],
    ['KeyW', 'W'],
    ['KeyE', 'E'],
    ['KeyR', 'R'],
    ['KeyT', 'T'],
  ]),
  row([
    ['CapsLock', 'Caps'],
    ['KeyA', 'A'],
    ['KeyS', 'S'],
    ['KeyD', 'D'],
    ['KeyF', 'F'],
    ['KeyG', 'G'],
  ]),
  row([
    ['ShiftLeft', 'Shift'],
    ['KeyZ', 'Z'],
    ['KeyX', 'X'],
    ['KeyC', 'C'],
    ['KeyV', 'V'],
    ['KeyB', 'B'],
  ]),
  row([
    ['ControlLeft', 'Ctrl'],
    ['MetaLeft', 'Meta'],
    ['AltLeft', 'Alt'],
    ['Space', 'Space'],
  ]),
]

export const RIGHT_HAND_ROWS: KeyDef[][] = [
  row([
    ['F6', 'F6'],
    ['F7', 'F7'],
    ['F8', 'F8'],
    ['F9', 'F9'],
    ['F10', 'F10'],
    ['F11', 'F11'],
    ['F12', 'F12'],
    ['PrintScreen', 'PrtSc'],
  ]),
  row([
    ['Digit6', '6'],
    ['Digit7', '7'],
    ['Digit8', '8'],
    ['Digit9', '9'],
    ['Digit0', '0'],
    ['Minus', '-'],
    ['Equal', '='],
    ['Backspace', 'Bksp'],
  ]),
  row([
    ['KeyY', 'Y'],
    ['KeyU', 'U'],
    ['KeyI', 'I'],
    ['KeyO', 'O'],
    ['KeyP', 'P'],
    ['BracketLeft', '['],
    ['BracketRight', ']'],
    ['Backslash', '\\'],
  ]),
  row([
    ['KeyH', 'H'],
    ['KeyJ', 'J'],
    ['KeyK', 'K'],
    ['KeyL', 'L'],
    ['Semicolon', ';'],
    ['Quote', "'"],
    ['Enter', 'Enter'],
  ]),
  row([
    ['KeyN', 'N'],
    ['KeyM', 'M'],
    ['Comma', ','],
    ['Period', '.'],
    ['Slash', '/'],
    ['ShiftRight', 'Shift'],
  ]),
  row([
    ['AltRight', 'Alt'],
    ['MetaRight', 'Meta'],
    ['ContextMenu', 'Menu'],
    ['ControlRight', 'Ctrl'],
  ]),
]

export const ALL_KEYS: KeyDef[] = [
  ...LEFT_HAND_ROWS.flat(),
  ...RIGHT_HAND_ROWS.flat(),
]

export function keyLabel(code: string, mode: KeyLabelMode = 'label'): string {
  if (mode === 'code') return code
  if (mode === 'numeric') return String(KEY_NUMERIC_CODES[code] ?? code)
  return ALL_KEYS.find((k) => k.code === code)?.label ?? code
}

export function randomId(): string {
  return Math.random().toString(36).slice(2, 10)
}
