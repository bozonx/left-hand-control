// Categories used by ActionPickerModal. Each category surfaces a subset of
// the actions the mapper understands. Users can also type an arbitrary
// string (e.g. chord "Ctrl+Shift+T") via the free-text input.

export interface ActionItem {
  label: string
  value: string
  hint?: string
}

export interface StaticCategory {
  id: string
  // i18n key under `categories.<...>` — resolved by the UI at render time.
  labelKey: string
  icon: string
  items: ActionItem[]
}

function k(values: string[]): ActionItem[] {
  return values.map((v) => ({ label: v, value: v }))
}

export const STATIC_CATEGORIES: StaticCategory[] = [
  {
    id: 'special',
    labelKey: 'categories.special',
    icon: 'i-lucide-square-asterisk',
    items: k([
      'Escape',
      'Enter',
      'Tab',
      'Backspace',
      'Delete',
      'Space',
      'CapsLock',
      'ContextMenu',
      'Insert',
      'PrintScreen',
      'ScrollLock',
      'Pause',
    ]),
  },
  {
    id: 'modifiers',
    labelKey: 'categories.modifiers',
    icon: 'i-lucide-keyboard',
    items: k([
      'ShiftLeft',
      'ShiftRight',
      'ControlLeft',
      'ControlRight',
      'AltLeft',
      'AltRight',
      'MetaLeft',
      'MetaRight',
    ]),
  },
  {
    id: 'letters',
    labelKey: 'categories.letters',
    icon: 'i-lucide-case-sensitive',
    items: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
      .split('')
      .map((c) => ({ label: c, value: `Key${c}` })),
  },
  {
    id: 'digits',
    labelKey: 'categories.digits',
    icon: 'i-lucide-hash',
    items: '0123456789'
      .split('')
      .map((c) => ({ label: c, value: `Digit${c}` })),
  },
  {
    id: 'numpad',
    labelKey: 'categories.numpad',
    icon: 'i-lucide-calculator',
    items: k([
      'NumLock',
      'Numpad0',
      'Numpad1',
      'Numpad2',
      'Numpad3',
      'Numpad4',
      'Numpad5',
      'Numpad6',
      'Numpad7',
      'Numpad8',
      'Numpad9',
      'NumpadAdd',
      'NumpadSubtract',
      'NumpadMultiply',
      'NumpadDivide',
      'NumpadDecimal',
      'NumpadEnter',
    ]),
  },
  {
    id: 'fkeys',
    labelKey: 'categories.fkeys',
    icon: 'i-lucide-square-chevron-up',
    items: Array.from({ length: 12 }, (_, i) => {
      const n = `F${i + 1}`
      return { label: n, value: n }
    }),
  },
  {
    id: 'nav',
    labelKey: 'categories.nav',
    icon: 'i-lucide-navigation',
    items: k(['Left', 'Right', 'Up', 'Down', 'Home', 'End', 'PageUp', 'PageDown']),
  },
  {
    id: 'symbols',
    labelKey: 'categories.symbols',
    icon: 'i-lucide-at-sign',
    items: [
      { label: '`', value: 'Backquote' },
      { label: '-', value: 'Minus' },
      { label: '=', value: 'Equal' },
      { label: '[', value: 'BracketLeft' },
      { label: ']', value: 'BracketRight' },
      { label: '\\', value: 'Backslash' },
      { label: ';', value: 'Semicolon' },
      { label: "'", value: 'Quote' },
      { label: ',', value: 'Comma' },
      { label: '.', value: 'Period' },
      { label: '/', value: 'Slash' },
    ],
  },
  {
    id: 'media',
    labelKey: 'categories.media',
    icon: 'i-lucide-volume-2',
    items: k([
      'VolumeUp',
      'VolumeDown',
      'VolumeMute',
      'MediaPlayPause',
      'MediaNext',
      'MediaPrev',
      'MediaStop',
      'Calculator',
      'BrowserBack',
      'BrowserForward',
      'BrowserRefresh',
      'BrowserHome',
      'BrowserStop',
      'BrowserSearch',
      'BrowserFavorites',
    ]),
  },
  {
    id: 'mouse',
    labelKey: 'categories.mouse',
    icon: 'i-lucide-mouse-pointer',
    items: k([
      'MouseLeft',
      'MouseRight',
      'MouseMiddle',
      'Mouse4',
      'Mouse5',
      'Mouse6',
      'WheelUp',
      'WheelDown',
    ]),
  },
  {
    id: 'other',
    labelKey: 'categories.other',
    icon: 'i-lucide-ellipsis',
    items: k([
      'F13',
      'F14',
      'F15',
      'F16',
      'F17',
      'F18',
      'F19',
      'F20',
      'F21',
      'F22',
      'F23',
      'F24',
      'Fn',
      'FnLock',
      'Help',
      'Undo',
      'Again',
      'Find',
      'Open',
      'Props',
      'Select',
      'Copy',
      'Cut',
      'Paste',
      'Power',
      'Sleep',
      'WakeUp',
      'Eject',
      'LaunchMail',
      'LaunchApp1',
      'LaunchApp2',
      'MediaSelect',
      'IntlBackslash',
      'IntlYen',
      'IntlRo',
      'KanaMode',
      'Convert',
      'NonConvert',
      'Lang1',
      'Lang2',
      'Lang3',
      'Lang4',
      'Lang5',
    ]),
  },
]
