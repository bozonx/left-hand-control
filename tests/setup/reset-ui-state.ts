import { afterEach, vi } from 'vitest'

import { resetAppLocaleStateForTests } from '~/composables/useAppLocale'
import { resetAppThemeStateForTests } from '~/composables/useAppTheme'
import { resetConfigStateForTests } from '~/composables/useConfig'
import { resetLayoutStateForTests } from '~/composables/useLayout'
import { resetLayoutLibraryStateForTests } from '~/composables/useLayoutLibrary'
import { resetLayoutSwitcherStateForTests } from '~/composables/useLayoutSwitcher'
import { resetMapperStateForTests } from '~/composables/useMapper'
import { resetPlatformInfoStateForTests } from '~/composables/usePlatformInfo'
import { resetUiStateForTests } from '~/composables/useUiState'

export async function resetUiSingletonsForTests() {
  resetAppLocaleStateForTests()
  resetAppThemeStateForTests()
  resetConfigStateForTests()
  resetLayoutSwitcherStateForTests()
  await resetLayoutStateForTests()
  resetLayoutLibraryStateForTests()
  resetMapperStateForTests()
  resetPlatformInfoStateForTests()
  resetUiStateForTests()
}

afterEach(async () => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  if (typeof document !== 'undefined') {
    document.documentElement.lang = 'en'
  }
  await resetUiSingletonsForTests()
})
