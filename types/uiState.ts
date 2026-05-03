import type { KeyLabelMode } from '~/utils/keys'

export interface UiState {
  selectedLayerId: string
  keyLabelMode: KeyLabelMode
  homeHelpOpen: boolean
  homePlatformOpen: boolean
}

export function createDefaultUiState(): UiState {
  return {
    selectedLayerId: '',
    keyLabelMode: 'label',
    homeHelpOpen: true,
    homePlatformOpen: true,
  }
}
