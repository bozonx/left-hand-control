import { BUILTIN_LAYOUT_ID } from '~/types/config'
import {
  type LayoutLibraryEntry,
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import {
  emptyLayoutPreset,
  extractPresetFromConfig,
  loadBuiltinLayout,
} from '~/utils/layoutPresets'
import { localeDisplayName } from '~/i18n'
import { usePlatformInfo } from '~/composables/usePlatformInfo'

interface PendingApply {
  kind: 'entry' | 'empty'
  entry?: LayoutLibraryEntry
  label: string
}

export function useSettingsScreen() {
  const {
    config,
    configPath,
    flush,
    applyPreset,
    markLayoutSavedAs,
    currentLayoutId,
    isLayoutDirty,
  } = useConfig()
  const library = useLayoutLibrary()
  const mapper = useMapper()
  const platform = usePlatformInfo()
  const theme = useAppTheme()
  const appLocale = useAppLocale()
  const { t } = useI18n()

  const appearanceItems = computed(() => [
    { label: t('settings.appearanceItems.system'), value: 'system' as const },
    { label: t('settings.appearanceItems.light'), value: 'light' as const },
    { label: t('settings.appearanceItems.dark'), value: 'dark' as const },
  ])

  const localeItems = computed(() => {
    const resolvedName = localeDisplayName(appLocale.systemLocale.value)
    return [
      {
        label: t('settings.languageAutoResolved', { resolved: resolvedName }),
        value: 'auto' as const,
      },
      ...appLocale.available.map((loc) => ({
        label: localeDisplayName(loc),
        value: loc,
      })),
    ]
  })

  const applying = ref('')
  const applyError = ref<string | null>(null)
  const pendingApply = ref<PendingApply | null>(null)

  const saveModalOpen = ref(false)
  const saveName = ref('')
  const saveBusy = ref(false)
  const saveError = ref<string | null>(null)

  const deletePending = ref<LayoutLibraryEntry | null>(null)
  const deleteBusy = ref(false)

  const deviceOptions = computed(() =>
    mapper.devices.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  )

  const selectedDevice = computed<string>({
    get: () => config.value.settings.inputDevicePath ?? '',
    set: (value: string) => {
      config.value.settings.inputDevicePath = value
    },
  })

  function requestApply(target: PendingApply) {
    applyError.value = null
    pendingApply.value = target
  }

  function requestApplyEntry(entry: LayoutLibraryEntry) {
    requestApply({
      kind: 'entry',
      entry,
      label: entry.name,
    })
  }

  function requestApplyEmpty() {
    requestApply({
      kind: 'empty',
      label: t('settings.emptyLayoutName'),
    })
  }

  function cancelApply() {
    pendingApply.value = null
  }

  async function confirmApply() {
    const target = pendingApply.value
    if (!target) return
    const id = target.kind === 'empty' ? 'empty' : target.entry!.id
    applying.value = id
    try {
      if (target.kind === 'empty') {
        await applyPreset(emptyLayoutPreset(t('settings.emptyLayoutName')), undefined)
      } else {
        const entry = target.entry!
        const preset =
          entry.id === BUILTIN_LAYOUT_ID
            ? await loadBuiltinLayout()
            : await library.loadPreset(entry.id)
        if (!preset) {
          applyError.value = t('settings.loadFailed', { name: entry.name })
          return
        }
        await applyPreset(preset, entry.id)
      }
      pendingApply.value = null
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error)
    } finally {
      applying.value = ''
    }
  }

  function openSaveModal() {
    saveError.value = null
    saveName.value = isUserLayoutId(currentLayoutId.value)
      ? userLayoutNameFromId(currentLayoutId.value!)
      : ''
    saveModalOpen.value = true
  }

  async function performSave() {
    const name = saveName.value.trim()
    if (!name) {
      saveError.value = t('settings.saveErrorEmpty')
      return
    }
    saveBusy.value = true
    saveError.value = null
    try {
      const preset = extractPresetFromConfig(config.value, name)
      const savedName = await library.saveUserPreset(name, preset)
      await markLayoutSavedAs(userLayoutId(savedName))
      saveModalOpen.value = false
    } catch (error) {
      saveError.value = error instanceof Error ? error.message : String(error)
    } finally {
      saveBusy.value = false
    }
  }

  function closeSaveModal() {
    saveModalOpen.value = false
  }

  async function confirmDelete() {
    const entry = deletePending.value
    if (!entry || entry.builtin) return
    deleteBusy.value = true
    try {
      await library.deleteUserPreset(userLayoutNameFromId(entry.id))
      if (currentLayoutId.value === entry.id) {
        config.value.settings.currentLayoutId = undefined
        await flush()
      }
      deletePending.value = null
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error)
    } finally {
      deleteBusy.value = false
    }
  }

  function clearDeletePending() {
    deletePending.value = null
  }

  async function toggleMapper() {
    try {
      await flush()
      if (mapper.status.value.running) {
        await mapper.stop()
        return
      }
      if (!selectedDevice.value) return
      await mapper.start(selectedDevice.value)
    } catch (error) {
      mapper.error.value = error instanceof Error ? error.message : String(error)
    }
  }

  onMounted(async () => {
    await Promise.all([
      mapper.refreshDevices(),
      mapper.refreshStatus(),
      platform.refresh(),
      library.refresh(),
    ])
  })

  return {
    config,
    configPath,
    currentLayoutId,
    isLayoutDirty,
    library,
    mapper,
    platform,
    theme,
    appLocale,
    appearanceItems,
    localeItems,
    applying,
    applyError,
    pendingApply,
    requestApply,
    requestApplyEntry,
    requestApplyEmpty,
    cancelApply,
    confirmApply,
    saveModalOpen,
    saveName,
    saveBusy,
    saveError,
    openSaveModal,
    performSave,
    closeSaveModal,
    deletePending,
    deleteBusy,
    confirmDelete,
    clearDeletePending,
    deviceOptions,
    selectedDevice,
    toggleMapper,
  }
}
