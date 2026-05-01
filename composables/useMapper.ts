// Thin composable around the Rust mapper commands.
// In the plain browser (pnpm dev without Tauri) everything becomes a no-op.

import {
  useMapperDevices,
  registerConsumer,
  resetMapperDevicesState,
} from '~/composables/useMapperDevices'
import { useMapperRuntime } from '~/composables/useMapperRuntime'

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

export function resetMapperStateForTests() {
  resetMapperDevicesState()
  singleton = null
}

export function useMapper(): MapperState {
  if (singleton) {
    registerConsumer(singleton.refreshStatus)
    return singleton
  }

  const { config, flush } = useConfig()
  const { t } = useI18n()
  const { activeAutoLayoutId } = useLayoutSwitcher()
  const {
    devices,
    mice,
    status,
    error,
    refreshDevices,
    refreshStatus,
  } = useMapperDevices()

  const busy = ref(false)

  async function invokeStart(devicePath: string, mouseDevicePath?: string) {
    const tauri = await useTauri()
    if (!tauri) {
      error.value = t('mapper.desktopOnly')
      return
    }
    try {
      await flush()
      const activeConfig = await runtime.computeActiveConfig()
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

  const runtime = useMapperRuntime({
    status,
    busy,
    invokeStart,
    invokeStop,
  })
  runtime.initSnapshot()

  registerConsumer(refreshStatus)

  watch([
    () => config.value,
    () => config.value.settings.manualActiveLayoutId,
    () => activeAutoLayoutId?.value,
  ], () => {
    if (!status.value.running) return
    runtime.scheduleReload()
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
