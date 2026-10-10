import {
  type AppConfig,
  type LayoutPreset,
  createDefaultConfig,
} from '~/types/config'
import {
  applyPresetToConfig,
  extractPresetFromConfig,
  layoutSnapshotOf,
} from '~/utils/layoutPresets'
import {
  clonePreset,
} from '~/composables/config/layoutSerialization'
import {
  parsePersistedSettings,
  serializePersistedSettings,
} from '~/composables/config/normalization'
import {
  configChangedOnDisk,
  getSettingsDir,
  isExternalChangeError,
  readConfigRaw,
  writeConfigRaw,
} from '~/composables/config/storage'

export { getSettingsDir } from '~/composables/config/storage'
export {
  normalizeConfig,
  parsePersistedConfig,
} from '~/composables/config/normalization'

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
  applyPreset: (
    preset: LayoutPreset,
    layoutId: string | undefined,
    activate?: boolean,
  ) => Promise<void>
  markLayoutSavedAs: (layoutId: string) => Promise<void>
  replaceCurrentLayoutSnapshot: (
    preset: LayoutPreset,
    layoutId: string,
  ) => Promise<void>
  resetCurrentLayout: () => Promise<void>
  reloadIfChangedOnDisk: () => Promise<boolean>
}

let singleton: ConfigState | null = null
let loadGeneration = 0
let focusListenerAttached = false

export function resetConfigStateForTests() {
  singleton = null
}

export function useConfig(): ConfigState {
  if (singleton) return singleton

  const toast = useToast()
  const { t } = useI18n()
  const route = useRoute()
  const config = ref<AppConfig>(createDefaultConfig())
  const loaded = ref(false)
  const lastError = ref<string | null>(null)
  const loadError = ref<string | null>(null)
  const settingsDir = ref('')
  const needsWelcome = ref(false)
  const layoutSnapshot = ref<string>(layoutSnapshotOf(config.value))
  const savedLayoutPreset = ref<LayoutPreset>(
    extractPresetFromConfig(config.value),
  )

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
      if (route.path === '/quick-menu' || route.path === '/emoji-menu') return
      await writeConfigRaw(serializePersistedSettings(config.value))
      lastError.value = null
      lastNotifiedSaveError = null
    },
    onError(e) {
      if (isExternalChangeError(e)) {
        toast.add({
          title: t('app.externalChangeTitle'),
          description: t('app.externalChangeDescription'),
          color: 'warning',
          icon: 'i-lucide-refresh-cw',
        })
        void load(true)
        return
      }
      const message = saveErrorMessage(e)
      lastError.value = message
      notifySaveError(message)
    },
    canSave() {
      return loaded.value && !needsWelcome.value
        && route.path !== '/quick-menu' && route.path !== '/emoji-menu'
    },
  })

  const { saving, scheduleSave, flush, persistNow, hasPendingSave } =
    persistence

  // Pick up edits made outside this window (the Slint shell edits the same
  // files). Skipped while a save is pending so local edits are not lost.
  async function reloadIfChangedOnDisk(): Promise<boolean> {
    if (!loaded.value || hasPendingSave()) return false
    try {
      if (!(await configChangedOnDisk())) return false
    } catch {
      return false
    }
    await useLayoutLibrary().refresh()
    await load(true)
    return true
  }

  function replacePreset(preset: LayoutPreset, layoutId: string | undefined) {
    config.value = applyPresetToConfig(config.value, preset, layoutId)
  }

  async function applyPreset(
    preset: LayoutPreset,
    layoutId: string | undefined,
    activate = true,
  ) {
    replacePreset(preset, layoutId)
    if (activate && config.value.settings.layoutMode === 'manual') {
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
    replacePreset(preset, layoutId)
    config.value.settings.layoutMode = 'manual'
    config.value.settings.manualActiveLayoutId = layoutId
    savedLayoutPreset.value = clonePreset(preset)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    needsWelcome.value = false
    await flush()
    await persistNow()
  }

  async function resetCurrentLayout() {
    replacePreset(clonePreset(savedLayoutPreset.value), currentLayoutId.value)
    layoutSnapshot.value = layoutSnapshotOf(config.value)
    await flush()
    await persistNow()
  }

  async function load(preserveLayout = false) {
    const previous = preserveLayout ? extractPresetFromConfig(config.value) : null
    const previousId = currentLayoutId.value
    const wasDirty = isLayoutDirty.value
    const gen = ++loadGeneration

    loaded.value = false
    loadError.value = null
    needsWelcome.value = false

    try {
      config.value = createDefaultConfig()

      const rawConfig = await readConfigRaw()

      if (rawConfig) {
        config.value.settings = parsePersistedSettings(rawConfig).settings
        needsWelcome.value = false
        const layoutId = config.value.settings.currentLayoutId
        const persistedLayout = layoutId
          ? await useLayoutLibrary().loadPreset(layoutId)
          : null

        if (persistedLayout) {
          config.value = applyPresetToConfig(
            config.value,
            persistedLayout,
            config.value.settings.currentLayoutId,
          )
        }
        if (!Object.hasOwn(JSON.parse(rawConfig).settings ?? {}, 'commandsEnabled')) {
          config.value.settings.commandsEnabled = config.value.commands.length > 0
        }
        savedLayoutPreset.value = extractPresetFromConfig(config.value)
        layoutSnapshot.value = layoutSnapshotOf(config.value)
        if (previous && wasDirty && previousId === currentLayoutId.value) {
          config.value = applyPresetToConfig(config.value, previous, previousId)
        }
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
    () => config.value.settings,
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
    reloadIfChangedOnDisk,
  }
  if (typeof window !== 'undefined' && !focusListenerAttached) {
    focusListenerAttached = true
    window.addEventListener('focus', () => {
      void singleton?.reloadIfChangedOnDisk()
    })
  }
  return singleton
}
