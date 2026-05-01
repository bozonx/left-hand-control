import { layoutSnapshotOf, emptyLayoutPreset, applyPresetToConfig } from '~/utils/layoutPresets'
import type { AppConfig } from '~/types/config'
import type { MapperStatus } from '~/composables/useMapper'

export function useMapperRuntime(
  deps: {
    status: Ref<MapperStatus>
    busy: Ref<boolean>
    invokeStart: (devicePath: string, mouseDevicePath?: string) => Promise<void>
    invokeStop: () => Promise<void>
  },
) {
  const { config } = useConfig()
  const { activeAutoLayoutId } = useLayoutSwitcher()
  const library = useLayoutLibrary()

  let reloadTimer: ReturnType<typeof setTimeout> | null = null
  let reloadPending = false

  async function computeActiveConfig(): Promise<AppConfig> {
    const settings = config.value.settings
    const activeId = settings.layoutMode === 'auto'
      ? activeAutoLayoutId?.value
      : settings.manualActiveLayoutId

    if (activeId === settings.currentLayoutId) {
      return config.value
    }

    let preset = emptyLayoutPreset()
    if (activeId) {
      const loaded = await library.loadPreset(activeId)
      if (loaded) preset = loaded
    }
    return applyPresetToConfig(config.value, preset, activeId)
  }

  const runtimeSnapshot = async () => {
    const cfg = await computeActiveConfig()
    return JSON.stringify({
      layout: layoutSnapshotOf(cfg),
      defaultHoldTimeoutMs: cfg.settings.defaultHoldTimeoutMs,
      defaultDoubleTapTimeoutMs: cfg.settings.defaultDoubleTapTimeoutMs,
      defaultMacroStepPauseMs: cfg.settings.defaultMacroStepPauseMs,
      defaultMacroModifierDelayMs: cfg.settings.defaultMacroModifierDelayMs,
    })
  }

  let lastRuntimeSnapshot = JSON.stringify({
    layout: layoutSnapshotOf(config.value),
    defaultHoldTimeoutMs: config.value.settings.defaultHoldTimeoutMs,
    defaultDoubleTapTimeoutMs: config.value.settings.defaultDoubleTapTimeoutMs,
    defaultMacroStepPauseMs: config.value.settings.defaultMacroStepPauseMs,
    defaultMacroModifierDelayMs: config.value.settings.defaultMacroModifierDelayMs,
  })
  void runtimeSnapshot().then((s) => {
    lastRuntimeSnapshot = s
  })

  function initSnapshot() {
    lastRuntimeSnapshot = JSON.stringify({
      layout: layoutSnapshotOf(config.value),
      defaultHoldTimeoutMs: config.value.settings.defaultHoldTimeoutMs,
      defaultDoubleTapTimeoutMs: config.value.settings.defaultDoubleTapTimeoutMs,
      defaultMacroStepPauseMs: config.value.settings.defaultMacroStepPauseMs,
      defaultMacroModifierDelayMs: config.value.settings.defaultMacroModifierDelayMs,
    })
    void runtimeSnapshot().then((s) => {
      lastRuntimeSnapshot = s
    })
  }

  function scheduleReload() {
    if (reloadTimer) clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => {
      reloadTimer = null
      void reloadRunningMapper()
    }, 150)
  }

  async function reloadRunningMapper() {
    const devicePath = deps.status.value.device_path ?? config.value.settings.inputDevicePath ?? ''
    const mouseDevicePath = deps.status.value.mouse_device_path ?? config.value.settings.inputMouseDevicePath ?? ''
    if (!deps.status.value.running || !devicePath) return
    if (deps.busy.value) {
      reloadPending = true
      return
    }

    const nextRuntimeSnapshot = await runtimeSnapshot()
    if (nextRuntimeSnapshot === lastRuntimeSnapshot) return

    deps.busy.value = true
    try {
      await deps.invokeStop()
      await deps.invokeStart(devicePath, mouseDevicePath)
      lastRuntimeSnapshot = nextRuntimeSnapshot
    } finally {
      deps.busy.value = false
      if (reloadPending) {
        reloadPending = false
        scheduleReload()
      }
    }
  }

  function resetState() {
    if (reloadTimer) {
      clearTimeout(reloadTimer)
      reloadTimer = null
    }
    reloadPending = false
  }

  return {
    computeActiveConfig,
    runtimeSnapshot,
    initSnapshot,
    scheduleReload,
    reloadRunningMapper,
    resetState,
  }
}
