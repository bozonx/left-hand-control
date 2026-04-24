export interface UiState {
  selectedLayerId: string
}

export function createDefaultUiState(): UiState {
  return {
    selectedLayerId: '',
  }
}
