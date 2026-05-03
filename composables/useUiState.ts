import { createDefaultUiState, type UiState } from '~/types/uiState'
import type { KeyLabelMode } from '~/utils/keys'

function normalizeUiState(raw: unknown): UiState {
  const base = createDefaultUiState()
  if (!raw || typeof raw !== 'object') return base
  const value = raw as Partial<UiState>
  const selectedLayerId =
    typeof value.selectedLayerId === 'string' && value.selectedLayerId.trim()
      ? value.selectedLayerId
      : base.selectedLayerId
  const keyLabelMode =
    value.keyLabelMode === 'label'
    || value.keyLabelMode === 'code'
    || value.keyLabelMode === 'numeric'
      ? value.keyLabelMode
      : base.keyLabelMode
  return {
    selectedLayerId,
    keyLabelMode,
    homeHelpOpen: typeof value.homeHelpOpen === 'boolean' ? value.homeHelpOpen : base.homeHelpOpen,
    homePlatformOpen: typeof value.homePlatformOpen === 'boolean' ? value.homePlatformOpen : base.homePlatformOpen,
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
  setSelectedLayerId: (value: string) => void
  setKeyLabelMode: (value: KeyLabelMode) => void
  setHomeHelpOpen: (value: boolean) => void
  setHomePlatformOpen: (value: boolean) => void
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

  const persistence = usePersistedState({
    delayMs: 150,
    async onSave() {
      const tauri = await useTauri()
      if (!tauri) return
      await tauri.invoke('save_ui_state', {
        contents: JSON.stringify(state.value, null, 2),
      })
    },
    canSave() {
      return loaded.value
    },
  })

  const { scheduleSave, flush } = persistence

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

  function setSelectedLayerId(value: string) {
    const next = value || ''
    if (state.value.selectedLayerId === next) return
    state.value = {
      ...state.value,
      selectedLayerId: next,
    }
    scheduleSave()
  }

  function setKeyLabelMode(value: KeyLabelMode) {
    if (state.value.keyLabelMode === value) return
    state.value = {
      ...state.value,
      keyLabelMode: value,
    }
    scheduleSave()
  }

  function setHomeHelpOpen(value: boolean) {
    if (state.value.homeHelpOpen === value) return
    state.value = {
      ...state.value,
      homeHelpOpen: value,
    }
    scheduleSave()
  }

  function setHomePlatformOpen(value: boolean) {
    if (state.value.homePlatformOpen === value) return
    state.value = {
      ...state.value,
      homePlatformOpen: value,
    }
    scheduleSave()
  }

  singleton = {
    state,
    loaded,
    loadError,
    load,
    flush,
    setSelectedLayerId,
    setKeyLabelMode,
    setHomeHelpOpen,
    setHomePlatformOpen,
  }
  return singleton
}
