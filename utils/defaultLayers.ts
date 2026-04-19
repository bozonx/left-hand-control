import yaml from 'js-yaml'
import {
  type AppConfig,
  type Layer,
  type LayerKeymap,
  type LayerRule,
  BASE_LAYER_ID,
  createDefaultConfig,
} from '~/types/config'
import { randomId } from '~/utils/keys'

// Shape of the YAML file (see public/default-layers.yaml).
interface DefaultsFile {
  settings?: {
    defaultHoldTimeoutMs?: number
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

  return cfg
}
