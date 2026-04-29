// Thin composable around the Rust mapper commands.
// In the plain browser (pnpm dev without Tauri) everything becomes a no-op.

import { layoutSnapshotOf, emptyLayoutPreset, applyPresetToConfig } from '~/utils/layoutPresets'
import type { AppConfig } from '~/types/config'

export interface KeyboardDevice {
  path: string
  name: string
}

export interface MapperStatus {
  running: boolean
  device_path: string | null
  mouse_device_path: string | null
  last_error: string | null
}

export interface MapperState {
  devices: Ref<KeyboardDevice[]>
  mice: Ref<KeyboardDevice[]>
  status: Ref<MapperStatus>
  busy: Ref<boolean>
  error: Ref<string | null>
  refreshDevices: () => Promise<void>
  refreshStatus: () => Promise<void>
  start: (devicePath: string, mouseDevicePath?: string) => Promise<void>
  stop: () => Promise<void>
}

let singleton: MapperState | null = null
let statusPollTimer: ReturnType<typeof setInterval> | null = null
let consumerCount = 0
let reloadTimer: ReturnType<typeof setTimeout> | null = null

export function resetMapperStateForTests() {
  stopStatusPolling()
  if (reloadTimer) {
    clearTimeout(reloadTimer)
    reloadTimer = null
  }
  consumerCount = 0
  singleton = null
}

function startStatusPolling(refreshStatus: () => Promise<void>) {
  if (statusPollTimer) return
  statusPollTimer = setInterval(() => {
    void refreshStatus()
  }, 2000)
}

function stopStatusPolling() {
  if (!statusPollTimer) return
  clearInterval(statusPollTimer)
  statusPollTimer = null
}

function registerConsumer(refreshStatus: () => Promise<void>) {
  startStatusPolling(refreshStatus)
  consumerCount += 1
  onScopeDispose(() => {
    consumerCount = Math.max(0, consumerCount - 1)
    if (consumerCount === 0) stopStatusPolling()
  })
}

export function useMapper(): MapperState {
  if (singleton) {
    registerConsumer(singleton.refreshStatus)
    return singleton
  }

  const { config, flush } = useConfig()
  const { activeAutoLayoutId } = useLayoutSwitcher()
  const library = useLayoutLibrary()
  const devices = ref<KeyboardDevice[]>([])
  const mice = ref<KeyboardDevice[]>([])
  const status = ref<MapperStatus>({ running: false, device_path: null, mouse_device_path: null, last_error: null })
  const busy = ref(false)
  const error = ref<string | null>(null)
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
  // Synchronous baseline using the freshly-loaded config — avoids the
  // race where the first `watch` tick fires before the async
  // initializer has set this, which used to look like a real change
  // and trigger a spurious mapper reload.
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

  async function refreshDevices() {
    error.value = null
    devices.value = []
    mice.value = []

    const tauri = await useTauri()
    const { t } = useI18n()
    if (!tauri) {
      error.value = t('mapper.desktopOnly')
      return
    }

    try {
      devices.value = await tauri.invoke<KeyboardDevice[]>('list_keyboards')
      error.value = null
    } catch (err: any) {
      error.value = t('mapper.listFailed', { err: String(err) })
      console.error('[useMapper] list_keyboards error:', err)
    }

    try {
      mice.value = await tauri.invoke<KeyboardDevice[]>('list_mice')
    } catch (err: any) {
      console.error('[useMapper] list_mice error:', err)
    }
  }

  async function refreshStatus() {
    const tauri = await useTauri()
    if (!tauri) return
    try {
      status.value = await tauri.invoke<MapperStatus>('mapper_status')
      error.value = status.value.last_error
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function invokeStart(devicePath: string, mouseDevicePath?: string) {
    const tauri = await useTauri()
    if (!tauri) {
      const { t } = useI18n()
      error.value = t('mapper.desktopOnly')
      return
    }
    try {
      await flush()
      const activeConfig = await computeActiveConfig()
      await tauri.invoke('start_mapper', {
        devicePath,
        mouseDevicePath: mouseDevicePath || null,
        configJson: JSON.stringify(activeConfig),
      })
      error.value = null
      await refreshStatus()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function start(devicePath: string, mouseDevicePath?: string) {
    busy.value = true
    try {
      await invokeStart(devicePath, mouseDevicePath)
    } finally {
      busy.value = false
    }
  }

  async function invokeStop() {
    const tauri = await useTauri()
    if (!tauri) return
    try {
      await tauri.invoke('stop_mapper')
      await refreshStatus()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function stop() {
    busy.value = true
    try {
      await invokeStop()
    } finally {
      busy.value = false
    }
  }

  function scheduleReload() {
    if (reloadTimer) clearTimeout(reloadTimer)
    reloadTimer = setTimeout(() => {
      reloadTimer = null
      void reloadRunningMapper()
    }, 150)
  }

  async function reloadRunningMapper() {
    const devicePath = status.value.device_path ?? config.value.settings.inputDevicePath ?? ''
    const mouseDevicePath = status.value.mouse_device_path ?? config.value.settings.inputMouseDevicePath ?? ''
    if (!status.value.running || !devicePath) return
    if (busy.value) {
      reloadPending = true
      return
    }

    busy.value = true
    try {
      await invokeStop()
      await invokeStart(devicePath, mouseDevicePath)
    } finally {
      busy.value = false
      if (reloadPending) {
        reloadPending = false
        scheduleReload()
      }
    }
  }

  registerConsumer(refreshStatus)

  watch([
    () => config.value,
    () => config.value.settings.manualActiveLayoutId,
    () => activeAutoLayoutId?.value
  ], async () => {
    const nextRuntimeSnapshot = await runtimeSnapshot()
    if (nextRuntimeSnapshot === lastRuntimeSnapshot) return
    lastRuntimeSnapshot = nextRuntimeSnapshot
    if (!status.value.running) return
    scheduleReload()
  }, { deep: true })

  singleton = {
    devices,
    mice,
    status,
    busy,
    error,
    refreshDevices,
    refreshStatus,
    start,
    stop,
  }
  return singleton
}
