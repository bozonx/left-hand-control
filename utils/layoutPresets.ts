import yaml from 'js-yaml'
import { logger } from '~/utils/logger'
import {
  type AppConfig,
  type Command,
  type EmojiHotkey,
  type EmojiPage,
  type ExtraKey,
  type Layer,
  type LayerKeymap,
  type LayerRule,
  type LayoutPreset,
  type Macro,
  type MacroStep,
  type QuickAction,
  type QuickActionPage,
  EMOJI_HOTKEYS,
  LEFT_HAND_HOTKEYS,
  createDefaultEmojiPage,
} from '~/types/config'

type TranslateFn = (key: string) => string

// Raw YAML shape of a layout preset file (both the bundled one in /public
// and user files under <configDir>/layouts/*.yaml). Settings live in
// config.json, NOT in layout files.
interface LayoutYaml {
  description?: string
  layers?: Array<{
    id: string
    name?: string
    description?: string
    keys?: Record<string, string | null>
    isolate?: string[]
    extras?: Array<{
      id?: string
      key?: string
      name?: string
      action?: string
    }>
  }>
  rules?: Array<{
    key?: string
    layer?: string | null
    // tap / hold follow a three-state convention:
    //   field absent                          -> native passthrough
    //   field present but null / ~            -> explicit swallow
    //   field present with a non-empty string -> action / action
    tap?: string | null
    hold?: string | null
    dtap?: string | null
    holdMs?: number | null
    dtapMs?: number | null
    isolate?: string | string[] | null
    id?: string
  }>
  macros?: Array<{
    id?: string
    name?: string
    steps?: Array<string | { id?: string; action?: string }>
    stepPauseMs?: number | null
    modifierDelayMs?: number | null
  }>
  commands?: Array<{
    id?: string
    name?: string
    linux?: string
  }>
  quickActions?: Array<{
    id?: string
    name?: string
    action?: string
    icon?: string
  }>
  quickActionPages?: Array<{
    id?: string
    name?: string
  }>
  emojiPages?: Array<{
    id?: string
    name?: string
    cells?: Record<string, string | null>
  }>
}

function genId(prefix: string): string {
  return `${prefix}${Math.random().toString(36).slice(2, 10)}`
}

function parsePreset(doc: LayoutYaml): LayoutPreset {
  const layers: Layer[] = []
  const layerKeymaps: Record<string, LayerKeymap> = {}

  for (const l of doc.layers ?? []) {
    if (!l?.id) continue
    layers.push({
      id: l.id,
      name: l.name ?? l.id,
      description: l.description,
    })

    const keys: Record<string, string | null> = {}
    for (const [k, v] of Object.entries(l.keys ?? {})) {
      if (v == null || v === '') {
        keys[k] = null
        continue
      }
      keys[k] = String(v)
    }
    const extras: ExtraKey[] = []
    for (const e of l.extras ?? []) {
      const key = e?.key ?? e?.name
      if (!key || !e?.action) continue
      extras.push({
        id: e.id ?? genId('x_'),
        key,
        action: e.action,
      })
    }
    layerKeymaps[l.id] = { keys, extras }
  }

  const rules: LayerRule[] = []
  for (const r of doc.rules ?? []) {
    if (!r?.key) continue
    // Three-state tap/hold: absent => '' (native), explicit null => null
    // (swallow), string => action.
    const tapAction: string | null = !('tap' in r)
      ? ''
      : r.tap === null
        ? null
        : String(r.tap)
    const holdAction: string | null = !('hold' in r)
      ? ''
      : r.hold === null
        ? null
        : String(r.hold)
    const ownIsolate = Array.isArray(r.isolate)
      ? r.isolate.filter((s): s is string => typeof s === 'string').join(', ')
      : typeof r.isolate === 'string'
        ? r.isolate
        : ''
    rules.push({
      id: r.id ?? genId('r_'),
      key: r.key,
      layerId: r.layer ?? '',
      tapAction,
      holdAction,
      isolate: ownIsolate || undefined,
      doubleTapAction: r.dtap ?? '',
      holdTimeoutMs:
        typeof r.holdMs === 'number' && r.holdMs >= 0 ? r.holdMs : undefined,
      doubleTapTimeoutMs:
        typeof r.dtapMs === 'number' && r.dtapMs >= 0 ? r.dtapMs : undefined,
    })
  }

  const macros: Macro[] = []
  for (const m of doc.macros ?? []) {
    if (!m?.id) continue
    const steps: MacroStep[] = []
    for (const s of m.steps ?? []) {
      if (typeof s === 'string') {
        if (s.trim()) steps.push({ id: genId('s_'), action: s })
      } else if (s?.action) {
        steps.push({ id: s.id ?? genId('s_'), action: s.action })
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

  const commands: Command[] = []
  for (const c of doc.commands ?? []) {
    if (!c?.id) continue
    commands.push({
      id: c.id,
      name: c.name ?? c.id,
      linux: c.linux?.trim() ?? '',
    })
  }

  const quickActions: QuickAction[] = []
  for (const action of doc.quickActions ?? []) {
    if (!action) continue
    const id = action.id?.trim() || genId('qa_')
    quickActions.push({
      id,
      name: action.name?.trim() || id,
      action: typeof action.action === 'string' ? action.action : '',
      icon: action.icon?.trim() || undefined,
    })
  }

  const quickActionPageCount = Math.max(
    1,
    Math.ceil(quickActions.length / LEFT_HAND_HOTKEYS.length),
    doc.quickActionPages?.length ?? 0,
  )
  const quickActionPages: QuickActionPage[] = []
  for (let index = 0; index < quickActionPageCount; index += 1) {
    const page = doc.quickActionPages?.[index]
    quickActionPages.push({
      id: page?.id?.trim() || genId('qap_'),
      name: page?.name?.trim() || `Page ${index + 1}`,
    })
  }

  const emojiKeySet = new Set<string>(EMOJI_HOTKEYS)
  const emojiPages: EmojiPage[] = []
  for (const page of doc.emojiPages ?? []) {
    if (!page) continue
    const id = page.id?.trim() || genId('emoji_')
    const cells: Partial<Record<EmojiHotkey, string>> = {}
    for (const [key, value] of Object.entries(page.cells ?? {})) {
      if (!emojiKeySet.has(key as EmojiHotkey) || typeof value !== 'string')
        continue
      cells[key as EmojiHotkey] = value
    }
    emojiPages.push({
      id,
      name: page.name?.trim() || id,
      cells,
    })
  }

  return {
    description: doc.description?.trim() || undefined,
    layers,
    rules,
    layerKeymaps,
    macros,
    commands,
    quickActions,
    quickActionPages,
    emojiPages: emojiPages.length > 0 ? emojiPages : [createDefaultEmojiPage()],
  }
}

export function parseLayoutYaml(text: string): LayoutPreset | null {
  try {
    const doc = yaml.load(text) as LayoutYaml | null
    if (!doc || typeof doc !== 'object') return null
    return parsePreset(doc)
  } catch (e) {
    logger.error('[LHC] parseLayoutYaml failed', e)
    return null
  }
}

export function serializeLayoutYaml(preset: LayoutPreset): string {
  const doc: LayoutYaml = {
    description: preset.description,
    layers: preset.layers.map((l) => {
      const km = preset.layerKeymaps[l.id]
      return {
        id: l.id,
        name: l.name,
        ...(l.description ? { description: l.description } : {}),
        ...(km && Object.keys(km.keys).length > 0 ? { keys: km.keys } : {}),
        ...(km && km.extras.length > 0
          ? {
              extras: km.extras.map((e) => ({
                key: e.key,
                action: e.action,
              })),
            }
          : {}),
      }
    }),
    rules: preset.rules.map((r) => ({
      key: r.key,
      ...(r.layerId ? { layer: r.layerId } : {}),
      // tap/hold: '' => native (omit field); null => explicit swallow
      // (emit `null`); non-empty string => action / action.
      ...(r.tapAction === '' ? {} : { tap: r.tapAction }),
      ...(r.holdAction === '' ? {} : { hold: r.holdAction }),
      ...(r.isolate && r.isolate.trim() ? { isolate: r.isolate.trim() } : {}),
      ...(r.doubleTapAction ? { dtap: r.doubleTapAction } : {}),
      ...(typeof r.holdTimeoutMs === 'number'
        ? { holdMs: r.holdTimeoutMs }
        : {}),
      ...(typeof r.doubleTapTimeoutMs === 'number'
        ? { dtapMs: r.doubleTapTimeoutMs }
        : {}),
    })),
    macros: preset.macros.map((m) => ({
      id: m.id,
      name: m.name,
      ...(typeof m.stepPauseMs === 'number'
        ? { stepPauseMs: m.stepPauseMs }
        : {}),
      ...(typeof m.modifierDelayMs === 'number'
        ? { modifierDelayMs: m.modifierDelayMs }
        : {}),
      steps: m.steps.map((s) => s.action),
    })),
    commands: preset.commands.map((c) => ({
      id: c.id,
      name: c.name,
      linux: c.linux,
    })),
    quickActions: preset.quickActions.map((action) => ({
      id: action.id,
      name: action.name,
      action: action.action,
      ...(action.icon ? { icon: action.icon } : {}),
    })),
    quickActionPages: (preset.quickActionPages ?? []).map((page) => ({
      id: page.id,
      name: page.name,
    })),
    emojiPages: (preset.emojiPages ?? []).map((page) => ({
      id: page.id,
      name: page.name,
      cells: Object.fromEntries(
        EMOJI_HOTKEYS.map((key) => [key, page.cells[key]]).filter(
          (entry): entry is [string, string] =>
            typeof entry[1] === 'string' && entry[1] !== '',
        ),
      ),
    })),
  }
  return yaml.dump(doc, { lineWidth: 100, noRefs: true })
}

export function emptyLayoutPreset(): LayoutPreset {
  return {
    layers: [],
    rules: [],
    layerKeymaps: {},
    macros: [],
    commands: [],
    quickActions: [],
    quickActionPages: [],
    emojiPages: [],
  }
}

// Extract the preset-subset of the current config (used when saving the
// current layout as a user preset).
export function extractPresetFromConfig(config: AppConfig): LayoutPreset {
  return {
    description: config.layoutDescription,
    layers: JSON.parse(JSON.stringify(config.layers)),
    rules: JSON.parse(JSON.stringify(config.rules)),
    layerKeymaps: JSON.parse(JSON.stringify(config.layerKeymaps)),
    macros: JSON.parse(JSON.stringify(config.macros)),
    commands: JSON.parse(JSON.stringify(config.commands)),
    quickActions: config.quickActions
      ? JSON.parse(JSON.stringify(config.quickActions))
      : [],
    quickActionPages: config.quickActionPages
      ? JSON.parse(JSON.stringify(config.quickActionPages))
      : [],
    emojiPages: config.emojiPages
      ? JSON.parse(JSON.stringify(config.emojiPages))
      : [createDefaultEmojiPage()],
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
    layoutDescription: preset.description,
    layers: JSON.parse(JSON.stringify(preset.layers)),
    rules: JSON.parse(JSON.stringify(preset.rules)),
    layerKeymaps: JSON.parse(JSON.stringify(preset.layerKeymaps)),
    macros: JSON.parse(JSON.stringify(preset.macros)),
    commands: JSON.parse(JSON.stringify(preset.commands)),
    quickActions: JSON.parse(JSON.stringify(preset.quickActions)),
    quickActionPages: JSON.parse(JSON.stringify(preset.quickActionPages ?? [])),
    emojiPages: JSON.parse(
      JSON.stringify(preset.emojiPages ?? [createDefaultEmojiPage()]),
    ),
    settings: { ...config.settings, currentLayoutId: layoutId },
  }
  for (const layer of next.layers) {
    if (!next.layerKeymaps[layer.id]) {
      next.layerKeymaps[layer.id] = { keys: {}, extras: [] }
    }
  }
  return next
}

// A stable string snapshot of the layout-subset, used for dirty-tracking.
export function layoutSnapshotOf(config: AppConfig): string {
  return JSON.stringify({
    layoutDescription: config.layoutDescription,
    layers: config.layers,
    rules: config.rules,
    layerKeymaps: config.layerKeymaps,
    macros: config.macros,
    commands: config.commands,
    quickActions: config.quickActions,
    quickActionPages: config.quickActionPages,
    emojiPages: config.emojiPages,
  })
}

// Built-in preset metadata.
export const BUILTIN_LAYOUT_META = {
  asset: '/ivank-layout.yaml',
  i18nBase: 'builtinLayouts.ivank',
  name: "Ivan K's left hand control",
} as const

function i18nOrFallback(
  t: TranslateFn,
  key: string,
  fallback?: string,
): string | undefined {
  const value = t(key)
  if (value !== key) return value
  return fallback
}

export function builtinLayoutName(t: TranslateFn): string {
  return (
    i18nOrFallback(
      t,
      `${BUILTIN_LAYOUT_META.i18nBase}.name`,
      BUILTIN_LAYOUT_META.name,
    ) ?? BUILTIN_LAYOUT_META.name
  )
}

export function localizeBuiltinLayoutPreset(
  preset: LayoutPreset,
  t: TranslateFn,
): LayoutPreset {
  return {
    ...preset,
    description: i18nOrFallback(
      t,
      `${BUILTIN_LAYOUT_META.i18nBase}.description`,
      preset.description,
    ),
    layers: preset.layers.map((layer) => ({
      ...layer,
      name:
        i18nOrFallback(
          t,
          `${BUILTIN_LAYOUT_META.i18nBase}.layers.${layer.id}.name`,
          layer.name,
        ) ?? layer.name,
      description: i18nOrFallback(
        t,
        `${BUILTIN_LAYOUT_META.i18nBase}.layers.${layer.id}.description`,
        layer.description,
      ),
    })),
  }
}

export async function loadBuiltinLayout(
  t: TranslateFn,
): Promise<LayoutPreset | null> {
  try {
    const res = await fetch(BUILTIN_LAYOUT_META.asset, { cache: 'no-cache' })
    if (!res.ok) return null
    const text = await res.text()
    const preset = parseLayoutYaml(text)
    return preset ? localizeBuiltinLayoutPreset(preset, t) : null
  } catch (e) {
    logger.error('[LHC] loadBuiltinLayout failed', e)
    return null
  }
}
