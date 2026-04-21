import { afterEach, vi } from 'vitest'

import { resetAppLocaleStateForTests } from '~/composables/useAppLocale'
import { resetAppThemeStateForTests } from '~/composables/useAppTheme'
import { resetConfigStateForTests } from '~/composables/useConfig'
import { resetLayoutStateForTests } from '~/composables/useLayout'
import { resetLayoutLibraryStateForTests } from '~/composables/useLayoutLibrary'
import { resetMapperStateForTests } from '~/composables/useMapper'
import { resetPlatformInfoStateForTests } from '~/composables/usePlatformInfo'

export async function resetUiSingletonsForTests() {
  resetAppLocaleStateForTests()
  resetAppThemeStateForTests()
  resetConfigStateForTests()
  await resetLayoutStateForTests()
  resetLayoutLibraryStateForTests()
  resetMapperStateForTests()
  resetPlatformInfoStateForTests()
}

afterEach(async () => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  if (typeof localStorage !== 'undefined') {
    localStorage.clear()
  }
  if (typeof document !== 'undefined') {
    document.documentElement.lang = 'en'
  }
  await resetUiSingletonsForTests()
})
