export const APP_TABS = ['layouts', 'rules', 'keymap', 'macros', 'settings'] as const

export type AppTab = (typeof APP_TABS)[number]

export interface UiState {
  activeTab: AppTab
  selectedLayerId: string
}

export function createDefaultUiState(): UiState {
  return {
    activeTab: 'layouts',
    selectedLayerId: 'base',
  }
}
