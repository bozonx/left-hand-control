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
    ['F6', 'F6'],
  ]),
  row([
    ['Backquote', '`'],
    ['Digit1', '1'],
    ['Digit2', '2'],
    ['Digit3', '3'],
    ['Digit4', '4'],
    ['Digit5', '5'],
    ['Digit6', '6'],
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
    ['F7', 'F7'],
    ['F8', 'F8'],
    ['F9', 'F9'],
    ['F10', 'F10'],
    ['F11', 'F11'],
    ['F12', 'F12'],
  ]),
  row([
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

export function keyLabel(code: string): string {
  return ALL_KEYS.find((k) => k.code === code)?.label ?? code
}

export function randomId(): string {
  return Math.random().toString(36).slice(2, 10)
}
