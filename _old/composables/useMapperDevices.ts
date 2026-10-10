import type { InputDevice, MapperStatus } from '~/composables/useMapper'

let statusPollTimer: ReturnType<typeof setInterval> | null = null
let consumerCount = 0

export function startStatusPolling(refreshStatus: () => Promise<void>) {
  if (statusPollTimer) return
  statusPollTimer = setInterval(() => {
    void refreshStatus()
  }, 2000)
}

export function stopStatusPolling() {
  if (!statusPollTimer) return
  clearInterval(statusPollTimer)
  statusPollTimer = null
}

export function registerConsumer(refreshStatus: () => Promise<void>) {
  startStatusPolling(refreshStatus)
  consumerCount += 1
  onScopeDispose(() => {
    consumerCount = Math.max(0, consumerCount - 1)
    if (consumerCount === 0) stopStatusPolling()
  })
}

export function resetMapperDevicesState() {
  stopStatusPolling()
  consumerCount = 0
}

export function useMapperDevices() {
  const { t } = useI18n()
  const inputDevices = ref<InputDevice[]>([])
  const devices = computed(() =>
    inputDevices.value.filter((device) => device.is_keyboard),
  )
  const mice = computed(() =>
    inputDevices.value.filter((device) => device.is_mouse),
  )
  const status = ref<MapperStatus>({
    running: false,
    device_path: null,
    mouse_device_path: null,
    last_error: null,
  })
  const error = ref<string | null>(null)

  async function refreshDevices() {
    error.value = null
    inputDevices.value = []

    const tauri = await useTauri()
    if (!tauri) {
      error.value = t('mapper.desktopOnly')
      return
    }

    try {
      inputDevices.value = await tauri.invoke<InputDevice[]>('list_input_devices')
      error.value = null
    } catch (err: unknown) {
      error.value = t('mapper.listFailed', { err: String(err) })
      logger.error('[mapper] list_input_devices failed', err)
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

  return {
    inputDevices,
    devices,
    mice,
    status,
    error,
    refreshDevices,
    refreshStatus,
  }
}
