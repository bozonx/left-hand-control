import { createDefaultUiState, type UiState } from '~/types/uiState'
import type { KeyLabelMode } from '~/utils/keys'

interface UiStateStore {
  state: Ref<UiState>
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

  function setSelectedLayerId(value: string) {
    const next = value || ''
    if (state.value.selectedLayerId === next) return
    state.value = {
      ...state.value,
      selectedLayerId: next,
    }
  }

  function setKeyLabelMode(value: KeyLabelMode) {
    if (state.value.keyLabelMode === value) return
    state.value = {
      ...state.value,
      keyLabelMode: value,
    }
  }

  function setHomeHelpOpen(value: boolean) {
    if (state.value.homeHelpOpen === value) return
    state.value = {
      ...state.value,
      homeHelpOpen: value,
    }
  }

  function setHomePlatformOpen(value: boolean) {
    if (state.value.homePlatformOpen === value) return
    state.value = {
      ...state.value,
      homePlatformOpen: value,
    }
  }

  singleton = {
    state,
    setSelectedLayerId,
    setKeyLabelMode,
    setHomeHelpOpen,
    setHomePlatformOpen,
  }
  return singleton
}
