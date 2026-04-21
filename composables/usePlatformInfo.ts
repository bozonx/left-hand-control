import type { PlatformInfo } from '~/types/platform'

export interface PlatformInfoState {
  info: Ref<PlatformInfo | null>
  busy: Ref<boolean>
  error: Ref<string | null>
  refresh: () => Promise<void>
}

let singleton: PlatformInfoState | null = null

export function resetPlatformInfoStateForTests() {
  singleton = null
}

export function usePlatformInfo(): PlatformInfoState {
  if (singleton) return singleton

  const info = ref<PlatformInfo | null>(null)
  const busy = ref(false)
  const error = ref<string | null>(null)

  async function refresh() {
    const tauri = await useTauri()
    if (!tauri) {
      info.value = null
      error.value = null
      return
    }

    busy.value = true
    try {
      info.value = await tauri.invoke<PlatformInfo>('get_platform_info')
      error.value = null
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      busy.value = false
    }
  }

  singleton = {
    info,
    busy,
    error,
    refresh,
  }
  return singleton
}
