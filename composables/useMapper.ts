// Thin composable around the Rust mapper commands.
// In the plain browser (pnpm dev without Tauri) everything becomes a no-op.

type TauriCore = typeof import('@tauri-apps/api/core')

let tauriCache: TauriCore | null | undefined
async function getTauri(): Promise<TauriCore | null> {
  if (tauriCache !== undefined) return tauriCache
  try {
    tauriCache = await import('@tauri-apps/api/core')
  } catch {
    tauriCache = null
  }
  return tauriCache
}

export interface KeyboardDevice {
  path: string
  name: string
}

export interface MapperStatus {
  running: boolean
  device_path: string | null
  last_error: string | null
}

interface MapperState {
  devices: Ref<KeyboardDevice[]>
  status: Ref<MapperStatus>
  busy: Ref<boolean>
  error: Ref<string | null>
  refreshDevices: () => Promise<void>
  refreshStatus: () => Promise<void>
  start: (devicePath: string) => Promise<void>
  stop: () => Promise<void>
}

let singleton: MapperState | null = null

export function useMapper(): MapperState {
  if (singleton) return singleton

  const { config, flush } = useConfig()
  const devices = ref<KeyboardDevice[]>([])
  const status = ref<MapperStatus>({ running: false, device_path: null, last_error: null })
  const busy = ref(false)
  const error = ref<string | null>(null)

  async function refreshDevices() {
    const tauri = await getTauri()
    if (!tauri) return
    try {
      devices.value = await tauri.invoke<KeyboardDevice[]>('list_keyboards')
      error.value = null
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function refreshStatus() {
    const tauri = await getTauri()
    if (!tauri) return
    try {
      status.value = await tauri.invoke<MapperStatus>('mapper_status')
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function start(devicePath: string) {
    const tauri = await getTauri()
    if (!tauri) {
      const { t } = useI18n()
      error.value = t('mapper.desktopOnly')
      return
    }
    busy.value = true
    try {
      await flush()
      await tauri.invoke('start_mapper', {
        devicePath,
        configJson: JSON.stringify(config.value),
      })
      error.value = null
      await refreshStatus()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      busy.value = false
    }
  }

  async function stop() {
    const tauri = await getTauri()
    if (!tauri) return
    busy.value = true
    try {
      await tauri.invoke('stop_mapper')
      error.value = null
      await refreshStatus()
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      busy.value = false
    }
  }

  singleton = {
    devices,
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
