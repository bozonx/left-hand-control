// Shared types for the key-mapper config persisted at
// ~/.config/LeftHandControl/config.json
import { STANDARD_EMOJIS } from '~/utils/emojiCatalog'

export interface Layer {
  id: string
  name: string
  description?: string
}

// A rule that binds a physical key to either a layer (on hold) and/or a
// single-tap / double-tap action.
//
// Three-state semantics for tap / hold ("not specified" vs "explicit none"):
//   undefined / empty string => **native** passthrough (act like the
//     physical key itself on that event).
//   null                      => **swallow** — nothing happens.
//   non-empty string          => **action** — user-defined action (for
//     `tapAction`: any action string; for `holdAction`: a keystroke
//     string like "ControlLeft" or "ControlLeft+ShiftLeft" which is held
//     down while the physical key is held).
//
export interface LayerRule {
  id: string
  // Determines if the rule is active. If false, it acts as native passthrough.
  enabled?: boolean
  // Condition required for the rule to trigger based on Game Mode.
  conditionGameMode?: 'ignore' | 'on' | 'off'
  // Condition required for the rule to trigger based on current keyboard layouts.
  // Empty array means 'ignore' (matches any layout). If not empty, the current
  // layout must be in this list.
  conditionLayouts?: string[]
  // Whitelist of substrings matched (case-insensitive, OR) against the
  // currently focused window's title and app id. When non-empty, the rule
  // is active only while the focused window matches at least one entry.
  conditionAppsWhitelist?: string[]
  // Blacklist of substrings matched (case-insensitive, OR) against the
  // currently focused window's title and app id. When non-empty and any
  // entry matches, the rule is blocked (takes precedence over whitelist).
  conditionAppsBlacklist?: string[]
  // Physical key on which the rule triggers (e.g. "CapsLock", "Space").
  key: string
  // Layer activated while the key is held. Empty string = no layer.
  layerId: string
  // Action fired on single tap. See three-state semantics above.
  tapAction: string | null
  // Keystroke held down while the physical key is held. See three-state
  // semantics above. May be combined with `layerId`, so one key can both
  // activate a layer and hold a modifier.
  holdAction: string | null
  isolate?: string
  // Action fired on double tap (second key-down within the double-tap
  // window after a short press). Empty string = no double-tap action.
  // Note: when set, a single tap is delayed by `doubleTapTimeoutMs` to
  // disambiguate it from the first press of a double-tap.
  doubleTapAction: string | null
  // Milliseconds the key must be held down before switching from "tap"
  // interpretation to "hold / layer". If omitted, falls back to
  // settings.defaultHoldTimeoutMs.
  holdTimeoutMs?: number
  // Milliseconds between first release and second key-down within which
  // a double tap is recognised. If omitted, falls back to
  // settings.defaultDoubleTapTimeoutMs.
  doubleTapTimeoutMs?: number
}

export interface ExtraKey {
  id: string
  // Physical key code (mouse button, media key, etc.) that triggers this binding.
  key: string
  // Action mapped to this extra key. Null explicitly swallows the key.
  action: string | null
}

export interface LayerKeymap {
  // key code -> action string, or null for explicit swallow inside the layer.
  // Missing entry means transparent passthrough to the base layout.
  keys: Record<string, string | null>
  isolate?: string[]
  // Extra user-defined key bindings (e.g. mouse buttons, media keys, ...).
  extras: ExtraKey[]
}

// One step of a macro. The value is a full action string: key chord,
// `macro:<id>`, `sys:<id>`, `app:<id>`, `cmd:<id>`, `text:<text>`,
// or macro-only `pause:<ms>`.
export interface MacroStep {
  id: string
  // The action spec string (e.g. 'KeyA', 'cmd:bar', 'app:showQuickMenu1', 'text:hello', 'pause:100').
  action: string
}

export interface Macro {
  id: string
  name: string
  steps: MacroStep[]
  // Pause between steps, ms. Falls back to settings.defaultMacroStepPauseMs.
  stepPauseMs?: number
  // Delay after pressing modifiers (Shift/Ctrl/...) before pressing the
  // main key of a chord, ms. Falls back to settings.defaultMacroModifierDelayMs.
  modifierDelayMs?: number
}

export interface Command {
  id: string
  name: string
  linux: string
  windows?: string
  macos?: string
}

export type AppearancePreference = 'system' | 'light' | 'dark'

// UI locale preference. 'auto' picks a language close to the OS one,
// falling back to English.
export type LocalePreference = 'auto' | 'en-US' | 'ru-RU'

// How the currently active layout is chosen.
//   'manual' — user picks a layout and it stays active; its whitelist /
//     blacklist still gate rule firing (a non-matching layout acts as
//     native passthrough).
//   'auto'   — the app picks the first enabled layout in `layoutOrder`
//     whose whitelist / blacklist allows it. If none matches, native
//     passthrough is used.
export type LayoutMode = 'manual' | 'auto'

export const LAYOUT_MODE_MANUAL = 'manual' as const
export const LAYOUT_MODE_AUTO = 'auto' as const

// A single condition set used as a whitelist or blacklist for a layout.
// Missing `gameMode` means "do not check"; empty `layouts` means
// "do not check layouts". Empty `apps` means "do not check apps". A
// condition set is considered "empty" (no-op) when no fields constrain it.
export interface LayoutConditionSet {
  gameMode?: 'on' | 'off'
  layouts: string[]
  // Substrings (case-insensitive, OR) matched against the currently focused
  // window's title and app id. Empty array means "do not check apps".
  apps?: string[]
}

export interface LayoutConditionRule {
  // When true, the layout participates in auto-mode picking.
  enabledInAuto?: boolean
  // When set, the layout is only active while conditions match.
  whitelist?: LayoutConditionSet
  // When set and matches, the layout is blocked from activating (takes
  // precedence over whitelist).
  blacklist?: LayoutConditionSet
}

export interface CommandTrustEntry {
  fingerprint: string
  trustedAt: string
}

export interface GameModeProcessMatcher {
  id: string
  name: string
  onlyActiveWindow: boolean
  isBlacklist?: boolean
}

// How literal text is injected on Linux/Wayland.
//   'libei'     — preferred Wayland-native emulated input path.
//   'keycode'   — XKB-aware RemoteDesktop keycode injection; falls back to
//                 clipboard for characters missing from the current layout.
//   'clipboard' — always use wl-copy + Ctrl+V for every text action.
//                 Use this if keycode injection produces wrong characters.
//   'ydotool'   — run a ydotool-compatible executable (`ydotool type ...`).
export type LinuxWaylandTextMode = 'libei' | 'keycode' | 'clipboard' | 'ydotool'

export interface AppSettings {
  launchOnStartup: boolean
  // Visual theme preference. 'system' follows prefers-color-scheme.
  appearance: AppearancePreference
  // UI language preference. 'auto' follows the OS language.
  locale: LocalePreference
  // Default hold timeout used when a rule does not specify one.
  defaultHoldTimeoutMs: number
  // Default double-tap window (ms between first release and second
  // key-down) used when a rule does not specify one.
  defaultDoubleTapTimeoutMs: number
  // Default pause between macro steps, ms.
  defaultMacroStepPauseMs: number
  // Default delay between pressing modifiers and the main key within one
  // macro chord, ms.
  defaultMacroModifierDelayMs: number
  // /dev/input/eventX path of the keyboard to intercept. Empty/undefined
  // means the mapper cannot start until the user picks one in Settings.
  inputDevicePath?: string
  // /dev/input/eventX path of the mouse to intercept (optional).
  // When set, mouse button events are also watched so they correctly
  // interact with modifier tap-hold rules.
  inputMouseDevicePath?: string
  // Id of the currently applied user layout file: `user:<name>`.
  // Empty/undefined means a custom / unnamed layout.
  currentLayoutId?: string
  // How the current layout is picked. Default: 'manual'.
  layoutMode: LayoutMode
  // Id of the layout the user explicitly chose in manual mode.
  manualActiveLayoutId?: string
  // Priority order for auto mode — array of layout ids. Layouts absent
  // from this array are placed at the end (preserving library order).
  layoutOrder: string[]
  // Per-layout auto-enable / whitelist / blacklist settings, keyed by
  // layout id (`user:<name>`). Stored here (not in the YAML) so layouts
  // stay portable across machines.
  layoutConditions: Record<string, LayoutConditionRule>
  commandTrust: Record<string, CommandTrustEntry>
  gameMode: {
    useGamemoded: boolean
    useFullscreen: boolean
    processMatchers: GameModeProcessMatcher[]
  }
  // Linux/Wayland only. Controls how text: actions are injected.
  linuxWaylandTextMode?: LinuxWaylandTextMode
  // Linux/Wayland only. Optional path/name for the ydotool-compatible
  // executable used when linuxWaylandTextMode is 'ydotool'.
  linuxYdotoolPath?: string
}

// A layout preset: the subset of AppConfig that describes keyboard behaviour
// (no global settings). Used for the bundled default and for user-saved YAML
// layouts under <configDir>/layouts/.
export interface QuickAction {
  id: string
  name: string
  action: string
  icon?: string
}

export interface QuickActionPage {
  id: string
  name: string
}

export const LEFT_HAND_HOTKEYS = [
  'KeyQ',
  'KeyW',
  'KeyE',
  'KeyR',
  'KeyT',
  'KeyA',
  'KeyS',
  'KeyD',
  'KeyF',
  'KeyG',
  'KeyZ',
  'KeyX',
  'KeyC',
  'KeyV',
  'KeyB',
] as const

export type LeftHandHotkey = (typeof LEFT_HAND_HOTKEYS)[number]

export const LEFT_HAND_HOTKEY_LABELS: Record<LeftHandHotkey, string> = {
  KeyQ: 'Q',
  KeyW: 'W',
  KeyE: 'E',
  KeyR: 'R',
  KeyT: 'T',
  KeyA: 'A',
  KeyS: 'S',
  KeyD: 'D',
  KeyF: 'F',
  KeyG: 'G',
  KeyZ: 'Z',
  KeyX: 'X',
  KeyC: 'C',
  KeyV: 'V',
  KeyB: 'B',
} as const

export const EMOJI_HOTKEYS = LEFT_HAND_HOTKEYS
export type EmojiHotkey = LeftHandHotkey
export const EMOJI_HOTKEY_LABELS = LEFT_HAND_HOTKEY_LABELS

export interface EmojiPage {
  id: string
  name: string
  cells: Partial<Record<EmojiHotkey, string>>
}

export function createDefaultEmojiPage(): EmojiPage {
  return {
    id: 'emoji_default',
    name: 'Emoji 1',
    cells: Object.fromEntries(
      EMOJI_HOTKEYS.map((key, index) => [key, STANDARD_EMOJIS[index] ?? '']),
    ) as Partial<Record<EmojiHotkey, string>>,
  }
}

export const MAX_EMOJI_CELL_LENGTH = 100

export interface LayoutPreset {
  description?: string
  layers: Layer[]
  rules: LayerRule[]
  layerKeymaps: Record<string, LayerKeymap>
  macros: Macro[]
  commands: Command[]
  quickActions: QuickAction[]
  quickActionPages?: QuickActionPage[]
  emojiPages?: EmojiPage[]
}

export interface AppConfig {
  version: 1
  layoutDescription?: string
  layers: Layer[]
  rules: LayerRule[]
  // Per-layer keymap keyed by Layer.id.
  layerKeymaps: Record<string, LayerKeymap>
  // User-defined macros, referenced from actions as "macro:<id>".
  macros: Macro[]
  // User-defined shell commands, referenced from actions as "cmd:<id>".
  commands: Command[]
  // User-defined quick actions available in the Quick Menu.
  quickActions: QuickAction[]
  // User-defined quick action page names.
  quickActionPages: QuickActionPage[]
  // User-defined emoji pages available in the Emoji Menu.
  emojiPages: EmojiPage[]
  settings: AppSettings
}

export interface PersistedConfig {
  version: 1
  settings: AppSettings
}

// Prefix used to mark an action string as a macro reference.
export const MACRO_ACTION_PREFIX = 'macro:'

export function macroActionRef(id: string): string {
  return `${MACRO_ACTION_PREFIX}${id}`
}

export function parseMacroRef(
  action: string | null | undefined,
): string | null {
  if (!action) return null
  return action.startsWith(MACRO_ACTION_PREFIX)
    ? action.slice(MACRO_ACTION_PREFIX.length)
    : null
}

// Prefix used to mark an action string as a system-function reference.
// System functions have a cross-platform id (e.g. "switchDesktop1") that
// is mapped to a concrete OS/DE-specific command by the Rust side.
export const SYSTEM_ACTION_PREFIX = 'sys:'

export function systemActionRef(id: string): string {
  return `${SYSTEM_ACTION_PREFIX}${id}`
}

export function parseSystemRef(
  action: string | null | undefined,
): string | null {
  if (!action) return null
  return action.startsWith(SYSTEM_ACTION_PREFIX)
    ? action.slice(SYSTEM_ACTION_PREFIX.length)
    : null
}

export const APP_ACTION_PREFIX = 'app:'

export function appActionRef(id: string): string {
  return `${APP_ACTION_PREFIX}${id}`
}

export function parseAppRef(
  action: string | null | undefined,
): string | null {
  if (!action) return null
  return action.startsWith(APP_ACTION_PREFIX)
    ? action.slice(APP_ACTION_PREFIX.length)
    : null
}

export const COMMAND_ACTION_PREFIX = 'cmd:'

export function commandActionRef(id: string): string {
  return `${COMMAND_ACTION_PREFIX}${id}`
}

export function parseCommandRef(
  action: string | null | undefined,
): string | null {
  if (!action) return null
  return action.startsWith(COMMAND_ACTION_PREFIX)
    ? action.slice(COMMAND_ACTION_PREFIX.length)
    : null
}

export const TEXT_ACTION_PREFIX = 'text:'

export function textActionRef(text: string): string {
  return `${TEXT_ACTION_PREFIX}${text}`
}

export function parseTextAction(
  action: string | null | undefined,
): string | null {
  if (!action) return null
  return action.startsWith(TEXT_ACTION_PREFIX)
    ? action.slice(TEXT_ACTION_PREFIX.length)
    : null
}

// Prefix used to compose an id for a user-saved layout: `user:<name>`.
export const USER_LAYOUT_PREFIX = 'user:'

export function createDefaultConfig(): AppConfig {
  return {
    version: 1,
    layers: [],
    rules: [],
    layerKeymaps: {},
    macros: [],
    commands: [],
    quickActions: [],
    quickActionPages: [{ id: 'quick_default', name: 'Page 1' }],
    emojiPages: [createDefaultEmojiPage()],
    settings: {
      launchOnStartup: false,
      appearance: 'system',
      locale: 'auto',
      defaultHoldTimeoutMs: 200,
      defaultDoubleTapTimeoutMs: 200,
      defaultMacroStepPauseMs: 20,
      defaultMacroModifierDelayMs: 5,
      inputDevicePath: '',
      layoutMode: LAYOUT_MODE_MANUAL,
      manualActiveLayoutId: undefined,
      layoutOrder: [],
      layoutConditions: {},
      commandTrust: {},
      gameMode: {
        useGamemoded: true,
        useFullscreen: false,
        processMatchers: [],
      },
      linuxWaylandTextMode: 'libei',
      linuxYdotoolPath: '',
    },
  }
}

export function createDefaultPersistedConfig(): PersistedConfig {
  const { settings, version } = createDefaultConfig()
  return {
    version,
    settings,
  }
}
