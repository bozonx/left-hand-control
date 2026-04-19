import yaml from 'js-yaml'
import {
  type AppConfig,
  type Layer,
  type LayerKeymap,
  type LayerRule,
  type Macro,
  type MacroStep,
  BASE_LAYER_ID,
  createDefaultConfig,
} from '~/types/config'
import { randomId } from '~/utils/keys'

// Shape of the YAML file (see public/default-layers.yaml).
interface DefaultsFile {
  settings?: {
    defaultHoldTimeoutMs?: number
    defaultMacroStepPauseMs?: number
    defaultMacroModifierDelayMs?: number
    launchOnStartup?: boolean
  }
  layers?: Array<{ id: string; name: string }>
  rules?: Array<{
    key: string
    layer?: string | null
    tap?: string | null
    holdMs?: number | null
  }>
  keymaps?: Record<
    string,
    {
      keys?: Record<string, string>
      extras?: Array<{ name: string; action: string }>
    }
  >
  macros?: Array<{
    id?: string
    name?: string
    stepPauseMs?: number | null
    modifierDelayMs?: number | null
    // Steps may be shorthand strings ("Ctrl+C") or objects.
    steps?: Array<string | { keystroke?: string } | null | undefined>
  }>
}

export async function loadDefaultsYaml(): Promise<AppConfig | null> {
  let raw: string
  try {
    const res = await fetch('/default-layers.yaml', { cache: 'no-cache' })
    if (!res.ok) return null
    raw = await res.text()
  } catch {
    return null
  }

  let parsed: DefaultsFile
  try {
    parsed = (yaml.load(raw) as DefaultsFile) ?? {}
  } catch (e) {
    console.warn('[LHC] failed to parse default-layers.yaml', e)
    return null
  }

  return fromDefaultsFile(parsed)
}

function fromDefaultsFile(input: DefaultsFile): AppConfig {
  const cfg = createDefaultConfig()

  if (input.settings) {
    if (typeof input.settings.defaultHoldTimeoutMs === 'number') {
      cfg.settings.defaultHoldTimeoutMs = input.settings.defaultHoldTimeoutMs
    }
    if (typeof input.settings.defaultMacroStepPauseMs === 'number') {
      cfg.settings.defaultMacroStepPauseMs =
        input.settings.defaultMacroStepPauseMs
    }
    if (typeof input.settings.defaultMacroModifierDelayMs === 'number') {
      cfg.settings.defaultMacroModifierDelayMs =
        input.settings.defaultMacroModifierDelayMs
    }
    if (typeof input.settings.launchOnStartup === 'boolean') {
      cfg.settings.launchOnStartup = input.settings.launchOnStartup
    }
  }

  const extraLayers: Layer[] = (input.layers ?? []).filter(
    (l) => l && l.id && l.id !== BASE_LAYER_ID,
  )
  cfg.layers = [{ id: BASE_LAYER_ID, name: 'Base' }, ...extraLayers]

  for (const layer of cfg.layers) {
    const km = input.keymaps?.[layer.id]
    const mapped: LayerKeymap = {
      keys: km?.keys && typeof km.keys === 'object' ? { ...km.keys } : {},
      extras: Array.isArray(km?.extras)
        ? km!.extras.map((e) => ({
            id: randomId(),
            name: e?.name ?? '',
            action: e?.action ?? '',
          }))
        : [],
    }
    cfg.layerKeymaps[layer.id] = mapped
  }

  const rules: LayerRule[] = (input.rules ?? [])
    .filter((r) => r && r.key)
    .map((r) => ({
      id: randomId(),
      key: r.key,
      layerId: r.layer ?? '',
      tapAction: r.tap ?? '',
      holdTimeoutMs:
        typeof r.holdMs === 'number' ? r.holdMs : undefined,
    }))
  cfg.rules = rules

  const macros: Macro[] = (input.macros ?? [])
    .filter((m) => m && Array.isArray(m.steps) && m.steps.length)
    .map((m) => {
      const steps: MacroStep[] = (m.steps ?? [])
        .map((s) => {
          if (typeof s === 'string') return { id: randomId(), keystroke: s }
          if (s && typeof s === 'object' && typeof s.keystroke === 'string') {
            return { id: randomId(), keystroke: s.keystroke }
          }
          return null
        })
        .filter((s): s is MacroStep => !!s && !!s.keystroke)
      return {
        id: m.id && m.id.trim() ? m.id : randomId(),
        name: m.name ?? m.id ?? 'macro',
        steps,
        stepPauseMs:
          typeof m.stepPauseMs === 'number' ? m.stepPauseMs : undefined,
        modifierDelayMs:
          typeof m.modifierDelayMs === 'number'
            ? m.modifierDelayMs
            : undefined,
      }
    })
    .filter((m) => m.steps.length > 0)
  cfg.macros = macros

  return cfg
}
