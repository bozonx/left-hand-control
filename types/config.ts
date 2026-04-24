// Shared types for the key-mapper config persisted at
// ~/.config/LeftHandControl/config.json

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
//     string like "ControlLeft" or "Ctrl+Shift" which is held down
//     while the physical key is held).
//
export interface LayerRule {
  id: string
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
  // Action fired on double tap (second key-down within the double-tap
  // window after a short press). Empty string = no double-tap action.
  // Note: when set, a single tap is delayed by `doubleTapTimeoutMs` to
  // disambiguate it from the first press of a double-tap.
  doubleTapAction: string
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
  // User-provided display name for the extra key / binding slot.
  name: string
  // Action mapped to this extra key.
  action: string
}

export interface LayerKeymap {
  // key code -> action string, or null for explicit swallow inside the layer.
  // Missing entry means transparent passthrough to the base layout.
  keys: Record<string, string | null>
  // Extra user-defined key bindings (e.g. mouse buttons, media keys, ...).
  extras: ExtraKey[]
}

// One step of a macro. For now only simple keystrokes are supported
// (single key or key with modifiers, e.g. "Ctrl+Shift+T", "Enter").
// Reserved for future: { type: 'shell', command: '...' },
//                      { type: 'system', name: '...' }.
export interface MacroStep {
  id: string
  // Keystroke string, same syntax as tap actions / keymap values.
  keystroke: string
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
}

export type AppearancePreference = 'system' | 'light' | 'dark'

// UI locale preference. 'auto' picks a language close to the OS one,
// falling back to English.
export type LocalePreference = 'auto' | 'en-US' | 'ru-RU'

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
  // Id of the currently applied user layout file: `user:<name>`.
  // Empty/undefined means a custom / unnamed layout.
  currentLayoutId?: string
}

// A layout preset: the subset of AppConfig that describes keyboard behaviour
// (no global settings). Used for the bundled default and for user-saved YAML
// layouts under <configDir>/layouts/.
export interface LayoutPreset {
  description?: string
  layers: Layer[]
  rules: LayerRule[]
  layerKeymaps: Record<string, LayerKeymap>
  macros: Macro[]
  commands: Command[]
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

export function parseMacroRef(action: string): string | null {
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

export function parseSystemRef(action: string): string | null {
  if (!action) return null
  return action.startsWith(SYSTEM_ACTION_PREFIX)
    ? action.slice(SYSTEM_ACTION_PREFIX.length)
    : null
}

export const COMMAND_ACTION_PREFIX = 'cmd:'

export function commandActionRef(id: string): string {
  return `${COMMAND_ACTION_PREFIX}${id}`
}

export function parseCommandRef(action: string): string | null {
  if (!action) return null
  return action.startsWith(COMMAND_ACTION_PREFIX)
    ? action.slice(COMMAND_ACTION_PREFIX.length)
    : null
}

export const TEXT_ACTION_PREFIX = 'text:'

export function textActionRef(text: string): string {
  return `${TEXT_ACTION_PREFIX}${text}`
}

export function parseTextAction(action: string): string | null {
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
    settings: {
      launchOnStartup: false,
      appearance: 'system',
      locale: 'auto',
      defaultHoldTimeoutMs: 200,
      defaultDoubleTapTimeoutMs: 200,
      defaultMacroStepPauseMs: 20,
      defaultMacroModifierDelayMs: 5,
      inputDevicePath: '',
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
