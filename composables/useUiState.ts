import { BASE_LAYER_ID } from '~/types/config'
import { APP_TABS, type AppTab, createDefaultUiState, type UiState } from '~/types/uiState'

function normalizeUiState(raw: unknown): UiState {
  const base = createDefaultUiState()
  if (!raw || typeof raw !== 'object') return base
  const value = raw as Partial<UiState>
  const activeTab = APP_TABS.includes(value.activeTab as AppTab)
    ? (value.activeTab as AppTab)
    : base.activeTab
  const selectedLayerId =
    typeof value.selectedLayerId === 'string' && value.selectedLayerId.trim()
      ? value.selectedLayerId
      : base.selectedLayerId
  return {
    activeTab,
    selectedLayerId,
  }
}

function parsePersistedUiState(raw: string): UiState {
  try {
    return normalizeUiState(JSON.parse(raw))
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new Error(`ui-state.json is invalid: ${message}`)
  }
}

interface UiStateStore {
  state: Ref<UiState>
  loaded: Ref<boolean>
  loadError: Ref<string | null>
  load: () => Promise<void>
  flush: () => Promise<void>
  setActiveTab: (value: AppTab) => void
  setSelectedLayerId: (value: string) => void
}

let singleton: UiStateStore | null = null

export function resetUiStateForTests() {
  singleton = null
}

export function useUiState(): UiStateStore {
  if (singleton) return singleton

  const state = ref<UiState>(createDefaultUiState())
  const loaded = ref(false)
  const loadError = ref<string | null>(null)

  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let saving = false
  let pendingFlushWaiters: Array<{
    resolve: () => void
    reject: (error: unknown) => void
  }> = []

  async function persistNow() {
    const tauri = await useTauri()
    if (!tauri) return
    saving = true
    try {
      await tauri.invoke('save_ui_state', {
        contents: JSON.stringify(state.value, null, 2),
      })
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.resolve()
    } catch (error) {
      const waiters = pendingFlushWaiters
      pendingFlushWaiters = []
      for (const waiter of waiters) waiter.reject(error)
      throw error
    } finally {
      saving = false
    }
  }

  function scheduleSave() {
    if (!loaded.value) return
    if (saveTimer) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      void persistNow().catch(() => {})
    }, 150)
  }

  async function load() {
    loaded.value = false
    loadError.value = null

    const tauri = await useTauri()
    if (!tauri) {
      state.value = createDefaultUiState()
      loaded.value = true
      return
    }

    try {
      const raw = await tauri.invoke<string>('load_ui_state')
      state.value = raw ? parsePersistedUiState(raw) : createDefaultUiState()
    } catch (error) {
      state.value = createDefaultUiState()
      loadError.value = error instanceof Error ? error.message : String(error)
    } finally {
      loaded.value = true
    }
  }

  async function flush() {
    if (saveTimer) {
      clearTimeout(saveTimer)
      saveTimer = null
      await persistNow()
      return
    }
    if (saving) {
      await new Promise<void>((resolve, reject) => {
        pendingFlushWaiters.push({ resolve, reject })
      })
    }
  }

  function setActiveTab(value: AppTab) {
    if (state.value.activeTab === value) return
    state.value = {
      ...state.value,
      activeTab: value,
    }
    scheduleSave()
  }

  function setSelectedLayerId(value: string) {
    const next = value || BASE_LAYER_ID
    if (state.value.selectedLayerId === next) return
    state.value = {
      ...state.value,
      selectedLayerId: next,
    }
    scheduleSave()
  }

  singleton = {
    state,
    loaded,
    loadError,
    load,
    flush,
    setActiveTab,
    setSelectedLayerId,
  }
  return singleton
}
