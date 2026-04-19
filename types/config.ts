// Shared types for the key-mapper config persisted at
// ~/.config/LeftHandControl/config.json

export interface Layer {
  id: string
  name: string
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

export interface AppSettings {
  launchOnStartup: boolean
  // Default hold timeout used when a rule does not specify one.
  defaultHoldTimeoutMs: number
  // /dev/input/eventX path of the keyboard to intercept. Empty/undefined
  // means the mapper cannot start until the user picks one in Settings.
  inputDevicePath?: string
}

export interface AppConfig {
  version: 1
  layers: Layer[]
  rules: LayerRule[]
  // Per-layer keymap keyed by Layer.id. A "base" layer with id "base" is
  // always present.
  layerKeymaps: Record<string, LayerKeymap>
  settings: AppSettings
}

export const BASE_LAYER_ID = 'base'

export function createDefaultConfig(): AppConfig {
  return {
    version: 1,
    layers: [{ id: BASE_LAYER_ID, name: 'Base' }],
    rules: [],
    layerKeymaps: {
      [BASE_LAYER_ID]: { keys: {}, extras: [] },
    },
    settings: {
      launchOnStartup: false,
      defaultHoldTimeoutMs: 200,
      inputDevicePath: '',
    },
  }
}
