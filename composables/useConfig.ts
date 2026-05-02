import {
  type AppConfig,
  type LayoutPreset,
  createDefaultConfig,
} from '~/types/config'
import {
  applyPresetToConfig,
  extractPresetFromConfig,
  layoutSnapshotOf,
  loadBuiltinLayout,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'
import { userLayoutId } from '~/composables/useLayoutLibrary'
import {
  clonePreset,
  legacyPresetFromPersistedConfig,
  parseCurrentLayout,
  serializeCurrentLayout,
} from '~/composables/config/layoutSerialization'
import {
  parsePersistedSettings,
  serializePersistedSettings,
} from '~/composables/config/normalization'
import {
  getSettingsDir,
  readConfigRaw,
  readCurrentLayoutRaw,
  writeConfigRaw,
  writeCurrentLayoutRaw,
  writeUserLayoutRaw,
} from '~/composables/config/storage'

export { getSettingsDir } from '~/composables/config/storage'
export { normalizeConfig, parsePersistedConfig } from '~/composables/config/normalization'

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
