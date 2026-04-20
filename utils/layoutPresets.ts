import yaml from 'js-yaml'
import {
  type AppConfig,
  type ExtraKey,
  type Layer,
  type LayerKeymap,
  type LayerRule,
  type LayoutPreset,
  type Macro,
  type MacroStep,
  BASE_LAYER_ID,
  BUILTIN_LAYOUT_ID,
} from '~/types/config'

// Raw YAML shape of a layout preset file (both the bundled one in /public
// and user files under <configDir>/layouts/*.yaml). Settings live in
// config.json, NOT in layout files.
interface LayoutYaml {
  name?: string
  description?: string
  layers?: Array<{ id: string; name?: string; description?: string }>
  rules?: Array<{
    key?: string
    layer?: string | null
    tap?: string | null
    holdMs?: number | null
    id?: string
  }>
  keymaps?: Record<
    string,
    {
      keys?: Record<string, string | null>
      extras?: Array<{ id?: string; name?: string; action?: string }>
    }
  >
  macros?: Array<{
    id?: string
    name?: string
    steps?: Array<string | { id?: string; keystroke?: string }>
    stepPauseMs?: number | null
    modifierDelayMs?: number | null
  }>
}

function genId(prefix: string): string {
  return `${prefix}${Math.random().toString(36).slice(2, 10)}`
}

function parsePreset(doc: LayoutYaml, fallbackName: string): LayoutPreset {
  const layers: Layer[] = [{ id: BASE_LAYER_ID, name: 'Base' }]
  for (const l of doc.layers ?? []) {
    if (!l?.id || l.id === BASE_LAYER_ID) continue
    layers.push({
      id: l.id,
      name: l.name ?? l.id,
      description: l.description,
    })
  }

  const rules: LayerRule[] = []
  for (const r of doc.rules ?? []) {
    if (!r?.key) continue
    rules.push({
      id: r.id ?? genId('r_'),
      key: r.key,
      layerId: r.layer ?? '',
      tapAction: r.tap ?? '',
      holdTimeoutMs:
        typeof r.holdMs === 'number' && r.holdMs >= 0 ? r.holdMs : undefined,
    })
  }

  const layerKeymaps: Record<string, LayerKeymap> = {}
  for (const layer of layers) {
    layerKeymaps[layer.id] = { keys: {}, extras: [] }
  }
  for (const [layerId, km] of Object.entries(doc.keymaps ?? {})) {
    if (!layerKeymaps[layerId]) layerKeymaps[layerId] = { keys: {}, extras: [] }
    const keys: Record<string, string> = {}
    for (const [k, v] of Object.entries(km?.keys ?? {})) {
      if (v == null || v === '') continue
      keys[k] = String(v)
    }
    const extras: ExtraKey[] = []
    for (const e of km?.extras ?? []) {
      if (!e?.name || !e?.action) continue
      extras.push({
        id: e.id ?? genId('x_'),
        name: e.name,
        action: e.action,
      })
    }
    layerKeymaps[layerId] = { keys, extras }
  }

  const macros: Macro[] = []
  for (const m of doc.macros ?? []) {
    if (!m?.id) continue
    const steps: MacroStep[] = []
    for (const s of m.steps ?? []) {
      if (typeof s === 'string') {
        if (s.trim()) steps.push({ id: genId('s_'), keystroke: s })
      } else if (s?.keystroke) {
        steps.push({ id: s.id ?? genId('s_'), keystroke: s.keystroke })
      }
    }
    macros.push({
      id: m.id,
      name: m.name ?? m.id,
      steps,
      stepPauseMs:
        typeof m.stepPauseMs === 'number' && m.stepPauseMs >= 0
          ? m.stepPauseMs
          : undefined,
      modifierDelayMs:
        typeof m.modifierDelayMs === 'number' && m.modifierDelayMs >= 0
          ? m.modifierDelayMs
          : undefined,
    })
  }

  return {
    name: doc.name?.trim() || fallbackName,
    description: doc.description?.trim() || undefined,
    layers,
    rules,
    layerKeymaps,
    macros,
  }
}

export function parseLayoutYaml(
  text: string,
  fallbackName = 'Untitled',
): LayoutPreset | null {
  try {
    const doc = yaml.load(text) as LayoutYaml | null
    if (!doc || typeof doc !== 'object') return null
    return parsePreset(doc, fallbackName)
  } catch (e) {
    console.error('[LHC] parseLayoutYaml failed:', e)
    return null
  }
}

export function serializeLayoutYaml(preset: LayoutPreset): string {
  const doc: LayoutYaml = {
    name: preset.name,
    description: preset.description,
    layers: preset.layers
      .filter((l) => l.id !== BASE_LAYER_ID)
      .map((l) => ({
        id: l.id,
        name: l.name,
        ...(l.description ? { description: l.description } : {}),
      })),
    rules: preset.rules.map((r) => ({
      key: r.key,
      ...(r.layerId ? { layer: r.layerId } : {}),
      ...(r.tapAction ? { tap: r.tapAction } : {}),
      ...(typeof r.holdTimeoutMs === 'number'
        ? { holdMs: r.holdTimeoutMs }
        : {}),
    })),
    keymaps: Object.fromEntries(
      Object.entries(preset.layerKeymaps).map(([id, km]) => [
        id,
        {
          keys: km.keys,
          extras: km.extras.map((e) => ({ name: e.name, action: e.action })),
        },
      ]),
    ),
    macros: preset.macros.map((m) => ({
      id: m.id,
      name: m.name,
      ...(typeof m.stepPauseMs === 'number'
        ? { stepPauseMs: m.stepPauseMs }
        : {}),
      ...(typeof m.modifierDelayMs === 'number'
        ? { modifierDelayMs: m.modifierDelayMs }
        : {}),
      steps: m.steps.map((s) => s.keystroke),
    })),
  }
  return yaml.dump(doc, { lineWidth: 100, noRefs: true })
}

export function emptyLayoutPreset(name = 'Пустая раскладка'): LayoutPreset {
  return {
    name,
    layers: [{ id: BASE_LAYER_ID, name: 'Base' }],
    rules: [],
    layerKeymaps: { [BASE_LAYER_ID]: { keys: {}, extras: [] } },
    macros: [],
  }
}

// Extract the preset-subset of the current config (used when saving the
// current layout as a user preset).
export function extractPresetFromConfig(
  config: AppConfig,
  name: string,
  description?: string,
): LayoutPreset {
  return {
    name,
    description,
    layers: JSON.parse(JSON.stringify(config.layers)),
    rules: JSON.parse(JSON.stringify(config.rules)),
    layerKeymaps: JSON.parse(JSON.stringify(config.layerKeymaps)),
    macros: JSON.parse(JSON.stringify(config.macros)),
  }
}

// Overwrite the layout-related fields of `config` with those from `preset`.
// Settings are intentionally preserved.
export function applyPresetToConfig(
  config: AppConfig,
  preset: LayoutPreset,
  layoutId: string | undefined,
): AppConfig {
  const next: AppConfig = {
    ...config,
    layers: JSON.parse(JSON.stringify(preset.layers)),
    rules: JSON.parse(JSON.stringify(preset.rules)),
    layerKeymaps: JSON.parse(JSON.stringify(preset.layerKeymaps)),
    macros: JSON.parse(JSON.stringify(preset.macros)),
    settings: { ...config.settings, currentLayoutId: layoutId },
  }
  // Ensure base layer always exists.
  if (!next.layers.some((l) => l.id === BASE_LAYER_ID)) {
    next.layers.unshift({ id: BASE_LAYER_ID, name: 'Base' })
  }
  if (!next.layerKeymaps[BASE_LAYER_ID]) {
    next.layerKeymaps[BASE_LAYER_ID] = { keys: {}, extras: [] }
  }
  return next
}

// A stable string snapshot of the layout-subset, used for dirty-tracking.
export function layoutSnapshotOf(config: AppConfig): string {
  return JSON.stringify({
    layers: config.layers,
    rules: config.rules,
    layerKeymaps: config.layerKeymaps,
    macros: config.macros,
  })
}

// Built-in preset metadata.
export const BUILTIN_LAYOUT_META = {
  id: BUILTIN_LAYOUT_ID,
  name: "Ivan K's left hand control",
} as const

export async function loadBuiltinLayout(): Promise<LayoutPreset | null> {
  try {
    const res = await fetch('/ivank-layout.yaml', { cache: 'no-cache' })
    if (!res.ok) return null
    const text = await res.text()
    return parseLayoutYaml(text, BUILTIN_LAYOUT_META.name)
  } catch (e) {
    console.error('[LHC] loadBuiltinLayout failed:', e)
    return null
  }
}
