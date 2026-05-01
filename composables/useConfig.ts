import {
  type AppConfig,
  type LayoutPreset,
  type PersistedConfig,
  createDefaultConfig,
  createDefaultPersistedConfig,
} from '~/types/config'
import {
  applyPresetToConfig,
  extractPresetFromConfig,
  layoutSnapshotOf,
  loadBuiltinLayout,
  parseLayoutYaml,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'
import { userLayoutId } from '~/composables/useLayoutLibrary'

async function readConfigRaw(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  return await tauri.invoke<string>('load_config')
}

async function writeConfigRaw(contents: string): Promise<void> {
  const tauri = await useTauri()
  if (!tauri) return
  await tauri.invoke('save_config', { contents })
}

async function readCurrentLayoutRaw(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  return await tauri.invoke<string>('load_current_layout')
}

async function writeCurrentLayoutRaw(contents: string): Promise<void> {
  const tauri = await useTauri()
  if (!tauri) return
  await tauri.invoke('save_current_layout', { contents })
}

async function writeUserLayoutRaw(
  name: string,
  contents: string,
  overwrite = true,
): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return name
  return await tauri.invoke<string>('save_user_layout', {
    name,
    contents,
    overwrite,
  })
}

export async function getSettingsDir(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  try {
    return await tauri.invoke<string>('get_settings_dir')
  } catch {
    return ''
  }
}

// Merge an (optionally partial) AppSettings with defaults, sanitizing
// shape of the new layout-mode fields.
function normalizeSettings(
  base: AppConfig['settings'],
  raw: Partial<AppConfig['settings']> | undefined,
): AppConfig['settings'] {
  const merged = { ...base, ...(raw ?? {}) }
  merged.layoutMode = merged.layoutMode === 'auto' ? 'auto' : 'manual'
  if (typeof merged.manualActiveLayoutId !== 'string' || !merged.manualActiveLayoutId) {
    merged.manualActiveLayoutId = undefined
  }
  merged.layoutOrder = Array.isArray(merged.layoutOrder)
    ? merged.layoutOrder.filter((id): id is string => typeof id === 'string')
    : []
  if (!merged.layoutConditions || typeof merged.layoutConditions !== 'object') {
    merged.layoutConditions = {}
  } else {
    const cleaned: AppConfig['settings']['layoutConditions'] = {}
    for (const [id, value] of Object.entries(merged.layoutConditions)) {
      if (!value || typeof value !== 'object') continue
      const v = value as Partial<AppConfig['settings']['layoutConditions'][string]>
      cleaned[id] = {
        whitelist: normalizeConditionSet(v.whitelist),
        blacklist: normalizeConditionSet(v.blacklist),
        disabledInAuto: !!v.disabledInAuto,
      }
    }
    merged.layoutConditions = cleaned
  }
  if (typeof merged.autoDefaultLayoutId !== 'string' || !merged.autoDefaultLayoutId) {
    merged.autoDefaultLayoutId = undefined
  }
  if (
    merged.layoutMode === 'manual' &&
    !merged.manualActiveLayoutId &&
    typeof merged.currentLayoutId === 'string' &&
    merged.currentLayoutId
  ) {
    merged.manualActiveLayoutId = merged.currentLayoutId
  }
  return merged
}

function normalizeConditionSet(
  raw: unknown,
): { gameMode?: 'on' | 'off'; layouts: string[]; apps?: string[] } | undefined {
  if (!raw || typeof raw !== 'object') return undefined
  const v = raw as { gameMode?: unknown; layouts?: unknown; apps?: unknown }
  const gameMode =
    v.gameMode === 'on' || v.gameMode === 'off' ? v.gameMode : undefined
  const layouts = Array.isArray(v.layouts)
    ? v.layouts.filter((s): s is string => typeof s === 'string' && !!s)
    : []
  const apps = Array.isArray(v.apps)
    ? v.apps.filter((s): s is string => typeof s === 'string' && !!s.trim())
    : []
  if (!gameMode && layouts.length === 0 && apps.length === 0) return undefined
  return {
    gameMode,
    layouts,
    ...(apps.length > 0 ? { apps } : {}),
  }
}

// Merge a (possibly partial / old-version) persisted config with defaults so
// the UI always sees a complete shape.
export function normalizeConfig(raw: unknown): AppConfig {
  const base = createDefaultConfig()
  if (!raw || typeof raw !== 'object') return base
  const r = raw as Partial<AppConfig>
  const cfg: AppConfig = {
    version: 1,
    layers:
      Array.isArray(r.layers) && r.layers.length
        ? r.layers
        : base.layers,
    rules: Array.isArray(r.rules)
      ? r.rules.map((rule) => ({
          ...rule,
          doubleTapAction: rule.doubleTapAction ?? '',
          tapAction: rule.tapAction ?? '',
          holdAction: rule.holdAction ?? '',
        }))
      : [],
    layerKeymaps:
      r.layerKeymaps && typeof r.layerKeymaps === 'object'
        ? r.layerKeymaps
        : base.layerKeymaps,
    macros: Array.isArray(r.macros) ? r.macros : [],
    commands: Array.isArray(r.commands) ? r.commands : [],
    settings: normalizeSettings(base.settings, r.settings),
  }
  for (const layer of cfg.layers) {
    if (!cfg.layerKeymaps[layer.id]) {
      cfg.layerKeymaps[layer.id] = { keys: {}, extras: [] }
    } else {
      const km = cfg.layerKeymaps[layer.id]!
      if (!km.keys || typeof km.keys !== 'object') {
        km.keys = {}
      } else {
        km.keys = Object.fromEntries(
          Object.entries(km.keys)
            .filter(([code, value]) => !!code && (typeof value === 'string' || value === null))
            .map(([code, value]) => [code, value === '' ? null : value]),
        )
      }
      if (!Array.isArray(km.extras)) km.extras = []
    }
  }
  return cfg
}

export function parsePersistedConfig(raw: string): AppConfig {
  try {
    return normalizeConfig(JSON.parse(raw))
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`config.json is invalid: ${message}`)
  }
}

function normalizePersistedConfig(raw: unknown): PersistedConfig {
  const base = createDefaultPersistedConfig()
  if (!raw || typeof raw !== 'object') return base
  const value = raw as Partial<PersistedConfig>
  return {
    version: 1,
    settings: normalizeSettings(base.settings, value.settings),
  }
}

function parsePersistedSettings(raw: string): PersistedConfig {
  try {
    return normalizePersistedConfig(JSON.parse(raw))
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`config.json is invalid: ${message}`)
  }
}

function serializePersistedSettings(config: AppConfig): string {
  const persisted: PersistedConfig = {
    version: 1,
    settings: config.settings,
  }
  return JSON.stringify(persisted, null, 2)
}

function parseCurrentLayout(raw: string): LayoutPreset | null {
  return parseLayoutYaml(raw)
}

function serializeCurrentLayout(config: AppConfig): string {
  return serializeLayoutYaml(extractPresetFromConfig(config))
}

function legacyPresetFromPersistedConfig(raw: string): LayoutPreset | null {
  try {
    const parsed = parsePersistedConfig(raw)
    return extractPresetFromConfig(parsed)
  } catch {
    return null
  }
}

function clonePreset(preset: LayoutPreset): LayoutPreset {
  return JSON.parse(JSON.stringify(preset))
}

interface ConfigState {
  config: Ref<AppConfig>
  loaded: Ref<boolean>
  saving: Ref<boolean>
  lastError: Ref<string | null>
  loadError: Ref<string | null>
  settingsDir: Ref<string>
  needsWelcome: Ref<boolean>
  currentLayoutId: ComputedRef<string | undefined>
  isLayoutDirty: ComputedRef<boolean>
  load: () => Promise<void>
  flush: () => Promise<void>
  applyPreset: (preset: LayoutPreset, layoutId: string | undefined) => Promise<void>
  markLayoutSavedAs: (layoutId: string) => Promise<void>
  replaceCurrentLayoutSnapshot: (preset: LayoutPreset, layoutId: string) => Promise<void>
  resetCurrentLayout: () => Promise<void>
}

let singleton: ConfigState | null = null
let loadGeneration = 0

export function resetConfigStateForTests() {
  singleton = null
}

export function useConfig(): ConfigState {
  if (singleton) return singleton

  const toast = useToast()
  const { t } = useI18n()
  const config = ref<AppConfig>(createDefaultConfig())
  const loaded = ref(false)
  const lastError = ref<string | null>(null)
  const loadError = ref<string | null>(null)
  const settingsDir = ref('')
  const needsWelcome = ref(false)
  const layoutSnapshot = ref<string>(layoutSnapshotOf(config.value))
  const savedLayoutPreset = ref<LayoutPreset>(extractPresetFromConfig(config.value))

  const currentLayoutId = computed<string | undefined>(
    () => config.value.settings.currentLayoutId || undefined,
  )

  const isLayoutDirty = computed<boolean>(
    () => layoutSnapshotOf(config.value) !== layoutSnapshot.value,
  )

  let lastNotifiedSaveError: string | null = null

  function saveErrorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
  }

  function notifySaveError(message: string) {
    if (!message || message === lastNotifiedSaveError) return
    lastNotifiedSaveError = message
    toast.add({
      title: t('app.saveFailedTitle'),
      description: message,
      color: 'error',
      icon: 'i-lucide-circle-alert',
      close: true,
      duration: 0,
    })
  }

  const persistence = usePersistedState({
    delayMs: 300,
    async onSave() {
      await Promise.all([
        writeConfigRaw(serializePersistedSettings(config.value)),
        writeCurrentLayoutRaw(serializeCurrentLayout(config.value)),
      ])
      lastError.value = null
      lastNotifiedSaveError = null
    },
    onError(e) {
      const message = saveErrorMessage(e)
      lastError.value = message
      notifySaveError(message)
    },
    canSave() {
      return loaded.value && !needsWelcome.value
    },
  })

  const { saving, scheduleSave, flush, persistNow } = persistence

  async function applyPreset(
    preset: LayoutPreset,
    layoutId: string | undefined,
  ) {
    config.value = applyPresetToConfig(config.value, preset, layoutId)
    if (config.value.settings.layoutMode === 'manual') {
      config.value.settings.manualActiveLayoutId = layoutId
    }
    savedLayoutPreset.value = clonePreset(preset)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    needsWelcome.value = false
    await flush()
    await persistNow()
  }

  async function markLayoutSavedAs(layoutId: string) {
    config.value.settings.currentLayoutId = layoutId
    if (config.value.settings.layoutMode === 'manual') {
      config.value.settings.manualActiveLayoutId = layoutId
    }
    savedLayoutPreset.value = extractPresetFromConfig(config.value)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    await flush()
  }

  async function replaceCurrentLayoutSnapshot(
    preset: LayoutPreset,
    layoutId: string,
  ) {
    config.value = applyPresetToConfig(config.value, preset, layoutId)
    config.value.settings.layoutMode = 'manual'
    config.value.settings.manualActiveLayoutId = layoutId
    savedLayoutPreset.value = clonePreset(preset)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    needsWelcome.value = false
    await flush()
    await persistNow()
  }

  async function resetCurrentLayout() {
    config.value = applyPresetToConfig(
      config.value,
      clonePreset(savedLayoutPreset.value),
      currentLayoutId.value,
    )
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    await flush()
    await persistNow()
  }

  async function load() {
    const gen = ++loadGeneration
    const forceIvank =
      import.meta.env.VITE_LHC_FORCE_IVANK === 'true' ||
      import.meta.env.VITE_LHC_FORCE_IVANK === '1'

    loaded.value = false
    loadError.value = null
    needsWelcome.value = false

    try {
      config.value = createDefaultConfig()

      const [rawConfig, rawCurrentLayout] = await Promise.all([
        readConfigRaw(),
        readCurrentLayoutRaw(),
      ])

      if (rawConfig) {
        config.value.settings = parsePersistedSettings(rawConfig).settings
      }

      if (forceIvank) {
        const preset = await loadBuiltinLayout(t)
        if (preset) {
          const savedName = await writeUserLayoutRaw(
            t('welcome.defaultIvanKFileName'),
            serializeLayoutYaml(preset),
            true,
          )
          config.value = applyPresetToConfig(
            config.value,
            preset,
            userLayoutId(savedName),
          )
          savedLayoutPreset.value = clonePreset(preset)
        }
        layoutSnapshot.value = layoutSnapshotOf(config.value)
        settingsDir.value = await getSettingsDir()
        loadError.value = null
        console.info(
          '[LHC] VITE_LHC_FORCE_IVANK is set — loaded bundled preset, ignoring persisted layout.',
        )
        return
      }

      if (rawConfig) {
        needsWelcome.value = false
        const persistedLayout =
          parseCurrentLayout(rawCurrentLayout) ?? legacyPresetFromPersistedConfig(rawConfig)

        if (persistedLayout) {
          config.value = applyPresetToConfig(
            config.value,
            persistedLayout,
            config.value.settings.currentLayoutId,
          )
        }
        savedLayoutPreset.value = extractPresetFromConfig(config.value)
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      } else {
        needsWelcome.value = true
        savedLayoutPreset.value = extractPresetFromConfig(config.value)
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      }

      settingsDir.value = await getSettingsDir()
      loadError.value = null
    } catch (e: unknown) {
      loadError.value = e instanceof Error ? e.message : String(e)
      config.value = createDefaultConfig()
      needsWelcome.value = false
      savedLayoutPreset.value = extractPresetFromConfig(config.value)
      layoutSnapshot.value = layoutSnapshotOf(config.value)
    } finally {
      if (gen === loadGeneration) {
        loaded.value = true
      }
    }
  }

  watch(
    config,
    () => {
      scheduleSave()
    },
    { deep: true },
  )

  singleton = {
    config,
    loaded,
    saving,
    lastError,
    loadError,
    settingsDir,
    needsWelcome,
    currentLayoutId,
    isLayoutDirty,
    load,
    flush,
    applyPreset,
    markLayoutSavedAs,
    replaceCurrentLayoutSnapshot,
    resetCurrentLayout,
  }
  return singleton
}
