import {
  type AppConfig,
  type LayoutPreset,
  BASE_LAYER_ID,
  BUILTIN_LAYOUT_ID,
  createDefaultConfig,
} from '~/types/config'
import {
  applyPresetToConfig,
  layoutSnapshotOf,
  loadBuiltinLayout,
} from '~/utils/layoutPresets'

async function readRaw(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  return await tauri.invoke<string>('load_config')
}

async function writeRaw(contents: string): Promise<void> {
  const tauri = await useTauri()
  if (!tauri) return
  await tauri.invoke('save_config', { contents })
}

export async function getConfigPath(): Promise<string> {
  const tauri = await useTauri()
  if (!tauri) return ''
  try {
    return await tauri.invoke<string>('get_config_path')
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
          // Default tap/hold to '' (native) for configs that predate the
          // three-state tap/hold fields. `null` (swallow) and non-empty
          // string action are preserved as-is.
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

interface ConfigState {
  config: Ref<AppConfig>
  loaded: Ref<boolean>
  saving: Ref<boolean>
  lastError: Ref<string | null>
  loadError: Ref<string | null>
  configPath: Ref<string>
  // True when we launched and found no persisted config — show the welcome
  // screen so the user can pick a starting layout.
  needsWelcome: Ref<boolean>
  // Id of the currently applied layout preset (mirrors
  // config.settings.currentLayoutId but always reactive).
  currentLayoutId: Ref<string | undefined>
  // True when the in-memory layout-subset differs from the snapshot taken
  // the last time the user applied or saved a layout. If `currentLayoutId`
  // is empty, any non-empty layout is considered "dirty" so that switching
  // away prompts to save first.
  isLayoutDirty: Ref<boolean>
  load: () => Promise<void>
  flush: () => Promise<void>
  // Apply a layout preset to the current config (settings are preserved),
  // persists, and marks the layout as clean under `layoutId` (undefined for
  // an ad-hoc / empty layout).
  applyPreset: (preset: LayoutPreset, layoutId: string | undefined) => Promise<void>
  // Reset dirty-tracking to the current state (e.g. after saving the current
  // config as a user layout).
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
  const configPath = ref('')
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
      await writeRaw(JSON.stringify(config.value, null, 2))
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
    if (needsWelcome.value) return // don't write config.json until user picks a layout
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
    await flush() // ensure the file exists even before the debounce fires
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
    // When VITE_LHC_FORCE_IVANK=true, ignore any persisted user layout on
    // startup and seed state from the bundled Ivan K's preset. Saving still
    // works normally: any edit in the UI will be written back to config.json,
    // overwriting the user's layout on the next auto-save. Useful for
    // iterating on the bundled preset; should NOT be enabled in production.
    const forceIvank =
      import.meta.env.VITE_LHC_FORCE_IVANK === 'true' ||
      import.meta.env.VITE_LHC_FORCE_IVANK === '1'

    loaded.value = false
    loadError.value = null
    needsWelcome.value = false

    try {
      if (forceIvank) {
        const preset = await loadBuiltinLayout()
        // Preserve settings from the existing config.json if any, so global
        // options like inputDevicePath aren't clobbered.
        const raw = await readRaw()
        if (raw) {
          config.value = parsePersistedConfig(raw)
        }
        if (preset) {
          config.value = applyPresetToConfig(
            config.value,
            preset,
            BUILTIN_LAYOUT_ID,
          )
        }
        layoutSnapshot.value = layoutSnapshotOf(config.value)
        configPath.value = await getConfigPath()
        loadError.value = null
        console.info(
          '[LHC] VITE_LHC_FORCE_IVANK is set — loaded bundled preset, ignoring persisted layout.',
        )
        return
      }

      const raw = await readRaw()
      if (raw) {
        config.value = parsePersistedConfig(raw)
        needsWelcome.value = false
        if (config.value.settings.currentLayoutId === BUILTIN_LAYOUT_ID) {
          const preset = await loadBuiltinLayout()
          if (preset) {
            config.value = applyPresetToConfig(
              config.value,
              preset,
              BUILTIN_LAYOUT_ID,
            )
            await writeRaw(JSON.stringify(config.value, null, 2))
          }
        }
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      } else {
        // First launch: no config yet. Show the welcome screen.
        needsWelcome.value = true
        layoutSnapshot.value = layoutSnapshotOf(config.value)
      }
      configPath.value = await getConfigPath()
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

  // Auto-save: any deep change to `config` while loaded schedules a save.
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
    configPath,
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
