import {
  type AppConfig,
  type LayoutPreset,
  type PersistedConfig,
  BASE_LAYER_ID,
  BUILTIN_LAYOUT_ID,
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

export async function getSettingsDir(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  try {
    return await tauri.invoke<string>('get_settings_dir')
  } catch {
    return ''
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
    settings: { ...base.settings, ...(r.settings ?? {}) },
  }
  if (!cfg.layers.some((l) => l.id === BASE_LAYER_ID)) {
    cfg.layers.unshift({ id: BASE_LAYER_ID, name: 'Base' })
  }
  for (const layer of cfg.layers) {
    if (!cfg.layerKeymaps[layer.id]) {
      cfg.layerKeymaps[layer.id] = { keys: {}, extras: [] }
    } else {
      const km = cfg.layerKeymaps[layer.id]!
      if (!km.keys || typeof km.keys !== 'object') km.keys = {}
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
    settings: { ...base.settings, ...(value.settings ?? {}) },
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
  return parseLayoutYaml(raw, 'Current layout')
}

function serializeCurrentLayout(config: AppConfig): string {
  return serializeLayoutYaml(extractPresetFromConfig(config, 'Current layout'))
}

function legacyPresetFromPersistedConfig(raw: string): LayoutPreset | null {
  try {
    const parsed = parsePersistedConfig(raw)
    return extractPresetFromConfig(parsed, 'Current layout')
  } catch {
    return null
  }
}

interface ConfigState {
  config: Ref<AppConfig>
  loaded: Ref<boolean>
  saving: Ref<boolean>
  lastError: Ref<string | null>
  loadError: Ref<string | null>
  settingsDir: Ref<string>
  needsWelcome: Ref<boolean>
  currentLayoutId: Ref<string | undefined>
  isLayoutDirty: Ref<boolean>
  load: () => Promise<void>
  flush: () => Promise<void>
  applyPreset: (preset: LayoutPreset, layoutId: string | undefined) => Promise<void>
  markLayoutSavedAs: (layoutId: string) => Promise<void>
}

let singleton: ConfigState | null = null

export function resetConfigStateForTests() {
  singleton = null
}

export function useConfig(): ConfigState {
  if (singleton) return singleton

  const toast = useToast()
  const { t } = useI18n()
  const config = ref<AppConfig>(createDefaultConfig())
  const loaded = ref(false)
  const saving = ref(false)
  const lastError = ref<string | null>(null)
  const loadError = ref<string | null>(null)
  const settingsDir = ref('')
  const needsWelcome = ref(false)
  const layoutSnapshot = ref<string>(layoutSnapshotOf(config.value))

  const currentLayoutId = computed<string | undefined>(
    () => config.value.settings.currentLayoutId || undefined,
  )

  const isLayoutDirty = computed<boolean>(
    () => layoutSnapshotOf(config.value) !== layoutSnapshot.value,
  )

  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let pendingFlushWaiters: Array<{
    resolve: () => void
    reject: (error: unknown) => void
  }> = []
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

  async function persistNow() {
    saving.value = true
    try {
      await Promise.all([
        writeConfigRaw(serializePersistedSettings(config.value)),
        writeCurrentLayoutRaw(serializeCurrentLayout(config.value)),
      ])
      lastError.value = null
      lastNotifiedSaveError = null
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.resolve()
    } catch (e: unknown) {
      const message = saveErrorMessage(e)
      lastError.value = message
      notifySaveError(message)
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.reject(e)
      throw e
    } finally {
      saving.value = false
    }
  }

  function scheduleSave() {
    if (!loaded.value) return
    if (needsWelcome.value) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      void persistNow().catch(() => {})
    }, 300)
  }

  async function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
      await persistNow()
      return
    }
    if (saving.value) {
      await new Promise<void>((resolve, reject) => {
        pendingFlushWaiters.push({ resolve, reject })
      })
    }
  }

  async function applyPreset(
    preset: LayoutPreset,
    layoutId: string | undefined,
  ) {
    config.value = applyPresetToConfig(config.value, preset, layoutId)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    needsWelcome.value = false
    await flush()
    if (!saveTimer && !saving.value) {
      await persistNow()
    }
  }

  async function markLayoutSavedAs(layoutId: string) {
    config.value.settings.currentLayoutId = layoutId
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    await flush()
  }

  async function load() {
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
        const preset = await loadBuiltinLayout()
        if (preset) {
          config.value = applyPresetToConfig(config.value, preset, BUILTIN_LAYOUT_ID)
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
        } else if (config.value.settings.currentLayoutId === BUILTIN_LAYOUT_ID) {
          const preset = await loadBuiltinLayout()
          if (preset) {
            config.value = applyPresetToConfig(
              config.value,
              preset,
              BUILTIN_LAYOUT_ID,
            )
          }
        }
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      } else {
        needsWelcome.value = true
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      }

      settingsDir.value = await getSettingsDir()
      loadError.value = null
    } catch (e: unknown) {
      loadError.value = e instanceof Error ? e.message : String(e)
      config.value = createDefaultConfig()
      needsWelcome.value = false
      layoutSnapshot.value = layoutSnapshotOf(config.value)
    } finally {
      loaded.value = true
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
  }
  return singleton
}
