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
  label: string
  icon: string
  items: ActionItem[]
}

function k(values: string[]): ActionItem[] {
  return values.map((v) => ({ label: v, value: v }))
}

export const STATIC_CATEGORIES: StaticCategory[] = [
  {
    id: 'special',
    label: 'Специальные',
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
    id: 'nav',
    label: 'Навигация',
    icon: 'i-lucide-navigation',
    items: k(['Left', 'Right', 'Up', 'Down', 'Home', 'End', 'PageUp', 'PageDown']),
  },
  {
    id: 'letters',
    label: 'Буквы',
    icon: 'i-lucide-case-sensitive',
    items: 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
      .split('')
      .map((c) => ({ label: c, value: `Key${c}` })),
  },
  {
    id: 'digits',
    label: 'Цифры',
    icon: 'i-lucide-hash',
    items: '0123456789'
      .split('')
      .map((c) => ({ label: c, value: `Digit${c}` })),
  },
  {
    id: 'fkeys',
    label: 'F-клавиши',
    icon: 'i-lucide-chevron-up-square',
    items: Array.from({ length: 24 }, (_, i) => {
      const n = `F${i + 1}`
      return { label: n, value: n }
    }),
  },
  {
    id: 'symbols',
    label: 'Символы',
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
    id: 'modifiers',
    label: 'Модификаторы',
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
    id: 'media',
    label: 'Медиа',
    icon: 'i-lucide-volume-2',
    items: k([
      'VolumeUp',
      'VolumeDown',
      'VolumeMute',
      'MediaPlayPause',
      'MediaNext',
      'MediaPrev',
      'MediaStop',
    ]),
  },
  {
    id: 'browser',
    label: 'Браузер',
    icon: 'i-lucide-globe',
    items: k([
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
    label: 'Мышь',
    icon: 'i-lucide-mouse-pointer',
    items: k([
      'MouseLeft',
      'MouseRight',
      'MouseMiddle',
      'Mouse4',
      'Mouse5',
      'WheelUp',
      'WheelDown',
    ]),
  },
]
