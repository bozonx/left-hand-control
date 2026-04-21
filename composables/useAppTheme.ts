import type { AppearancePreference } from '~/types/config'

// Bridges the persisted `settings.appearance` with Nuxt UI's colorMode.
//
// Nuxt UI v3 ships `@nuxtjs/color-mode` which drives the `html.dark` class
// and CSS variables. We treat `useConfig().config.settings.appearance` as
// the single source of truth and push it into `useColorMode().preference`
// whenever it changes; `system` defers to `prefers-color-scheme`.

interface AppThemeApi {
  // User preference persisted in config ('system' | 'light' | 'dark').
  preference: Ref<AppearancePreference>
  // Actual resolved mode after honoring 'system' ('light' | 'dark').
  resolved: Ref<'light' | 'dark'>
  // Toggle between light and dark, pinning the user choice (escapes 'system').
  toggle: () => void
}

let singleton: AppThemeApi | null = null

export function resetAppThemeStateForTests() {
  singleton = null
}

export function useAppTheme(): AppThemeApi {
  if (singleton) return singleton

  const { config, loaded } = useConfig()
  const colorMode = useColorMode()

  const preference = computed<AppearancePreference>({
    get: () => (config.value.settings.appearance ?? 'system'),
    set: (v) => {
      config.value.settings.appearance = v
    },
  })

  const resolved = computed<'light' | 'dark'>(() =>
    colorMode.value === 'dark' ? 'dark' : 'light',
  )

  function apply(pref: AppearancePreference) {
    // `preference = 'system'` makes @nuxtjs/color-mode follow the OS.
    colorMode.preference = pref
  }

  // Sync config -> colorMode. Runs as soon as the config is loaded, and
  // re-runs whenever the user changes the preference in Settings.
  watch(
    [loaded, preference],
    ([isLoaded, pref]) => {
      if (!isLoaded) return
      apply(pref)
    },
    { immediate: true },
  )

  function toggle() {
    preference.value = resolved.value === 'dark' ? 'light' : 'dark'
  }

  singleton = { preference, resolved, toggle }
  return singleton
}
