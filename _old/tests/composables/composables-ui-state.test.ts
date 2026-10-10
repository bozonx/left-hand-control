import { describe, expect, it } from 'vitest'
import { resetUiStateForTests, useUiState } from '~/composables/useUiState'
import { createDefaultUiState } from '~/types/uiState'

describe('UI session state', () => {
  it('shares preferences within a session and resets them in the next session', () => {
    const state = useUiState()
    state.setSelectedLayerId('nav')
    state.setKeyLabelMode('numeric')
    state.setHomeHelpOpen(false)
    state.setHomePlatformOpen(false)
    expect(useUiState().state.value).toEqual({
      selectedLayerId: 'nav',
      keyLabelMode: 'numeric',
      homeHelpOpen: false,
      homePlatformOpen: false,
    })
    resetUiStateForTests()
    expect(useUiState().state.value).toEqual(createDefaultUiState())
  })
})
