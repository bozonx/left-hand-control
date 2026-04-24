import {
  parseCommandRef,
  parseMacroRef,
  parseSystemRef,
  parseTextAction,
} from '~/types/config'

const KEY_TOKENS = new Set([
  'Escape',
  'Tab',
  'CapsLock',
  'Enter',
  'Backspace',
  'Space',
  'Delete',
  'Insert',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'ArrowLeft',
  'ArrowRight',
  'ArrowUp',
  'ArrowDown',
  'PrintScreen',
  'Backquote',
  'Minus',
  'Equal',
  'BracketLeft',
  'BracketRight',
  'Backslash',
  'Semicolon',
  'Quote',
  'Comma',
  'Period',
  'Slash',
  'ShiftLeft',
  'ShiftRight',
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
  'ContextMenu',
  'BrowserBack',
  'BrowserForward',
  'BrowserRefresh',
  'BrowserHome',
  'VolumeUp',
  'VolumeDown',
  'VolumeMute',
  'MediaPlayPause',
  'MediaNext',
  'MediaPrev',
  ...Array.from({ length: 26 }, (_, i) => `Key${String.fromCharCode(65 + i)}`),
  ...Array.from({ length: 10 }, (_, i) => `Digit${i}`),
  ...Array.from({ length: 24 }, (_, i) => `F${i + 1}`),
])

const MOD_TOKENS = new Set([
  'Ctrl',
  'Shift',
  'Alt',
  'Meta',
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
  'MetaLeft',
  'MetaRight',
])

function isKeyToken(token: string) {
  return KEY_TOKENS.has(token)
}

function isModifierToken(token: string) {
  return MOD_TOKENS.has(token)
}

export function isCanonicalAction(action: string | null | undefined): boolean {
  const raw = action ?? ''
  if (!raw) return true

  if (parseMacroRef(raw) || parseCommandRef(raw) || parseSystemRef(raw)) return true

  const textAction = parseTextAction(raw)
  if (textAction !== null) return true

  const tokens = raw.split('+').map((token) => token.trim())
  if (tokens.some((token) => token.length === 0)) return false
  if (tokens.length === 1) return isKeyToken(tokens[0]!)

  const main = tokens.at(-1)!
  const modifiers = tokens.slice(0, -1)
  if (!isKeyToken(main)) return false
  if (!modifiers.every(isModifierToken)) return false
  return true
}
