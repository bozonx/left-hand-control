// Shared types for the key-mapper config persisted at
// ~/.config/LeftHandControl/config.json

export interface Layer {
  id: string
  name: string
  description?: string
}

// A rule that binds a physical key to either a layer (on hold) and/or a
// single-tap action.
export interface LayerRule {
  id: string
  // Physical key on which the rule triggers (e.g. "CapsLock", "Space").
  key: string
  // Layer activated while the key is held. Empty string = no layer.
  layerId: string
  // Action fired on single tap. Empty string = no tap action.
  tapAction: string
  // Milliseconds the key must be held down before switching from "tap"
  // interpretation to "hold / layer". If omitted, falls back to
  // settings.defaultHoldTimeoutMs.
  holdTimeoutMs?: number
}

export interface ExtraKey {
  id: string
  // User-provided display name for the extra key / binding slot.
  name: string
  // Action mapped to this extra key.
  action: string
}

export interface LayerKeymap {
  // key code -> action string
  keys: Record<string, string>
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

export interface AppSettings {
  launchOnStartup: boolean
  // Default hold timeout used when a rule does not specify one.
  defaultHoldTimeoutMs: number
  // Default pause between macro steps, ms.
  defaultMacroStepPauseMs: number
  // Default delay between pressing modifiers and the main key within one
  // macro chord, ms.
  defaultMacroModifierDelayMs: number
  // /dev/input/eventX path of the keyboard to intercept. Empty/undefined
  // means the mapper cannot start until the user picks one in Settings.
  inputDevicePath?: string
  // Id of the currently applied layout preset: BUILTIN_LAYOUT_ID for the
  // bundled "Ivan K's" preset, or "user:<name>" for a preset saved by the
  // user. Empty/undefined means a custom / unnamed layout.
  currentLayoutId?: string
}

// A layout preset: the subset of AppConfig that describes keyboard behaviour
// (no global settings). Used for the bundled default and for user-saved YAML
// layouts under <configDir>/layouts/.
export interface LayoutPreset {
  name: string
  description?: string
  layers: Layer[]
  rules: LayerRule[]
  layerKeymaps: Record<string, LayerKeymap>
  macros: Macro[]
}

export interface AppConfig {
  version: 1
  layers: Layer[]
  rules: LayerRule[]
  // Per-layer keymap keyed by Layer.id. A "base" layer with id "base" is
  // always present.
  layerKeymaps: Record<string, LayerKeymap>
  // User-defined macros, referenced from actions as "macro:<id>".
  macros: Macro[]
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

export const BASE_LAYER_ID = 'base'

// Id of the bundled "Ivan K's left hand control" layout preset.
export const BUILTIN_LAYOUT_ID = 'builtin:ivank'
// Prefix used to compose an id for a user-saved layout: `user:<name>`.
export const USER_LAYOUT_PREFIX = 'user:'

export function createDefaultConfig(): AppConfig {
  return {
    version: 1,
    layers: [{ id: BASE_LAYER_ID, name: 'Base' }],
    rules: [],
    layerKeymaps: {
      [BASE_LAYER_ID]: { keys: {}, extras: [] },
    },
    macros: [],
    settings: {
      launchOnStartup: false,
      defaultHoldTimeoutMs: 200,
      defaultMacroStepPauseMs: 20,
      defaultMacroModifierDelayMs: 5,
      inputDevicePath: '',
    },
  }
}
