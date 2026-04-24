import type { KeyLabelMode } from '~/utils/keys'

export interface UiState {
  selectedLayerId: string
  keyLabelMode: KeyLabelMode
}

export function createDefaultUiState(): UiState {
  return {
    selectedLayerId: '',
    keyLabelMode: 'label',
  }
}
