import type { AppearancePreference } from '~/types/config'

interface AppThemeApi {
  preference: Ref<AppearancePreference>
  resolved: Ref<'light' | 'dark'>
  toggle: () => void
}

let singleton: AppThemeApi | null = null

export function resetAppThemeStateForTests() {
  singleton = null
}

export function useAppTheme(): AppThemeApi {
  if (singleton) return singleton

  const { config, loaded } = useConfig()
  const media = import.meta.client ? window.matchMedia('(prefers-color-scheme: dark)') : null
  const systemDark = ref(media?.matches ?? false)
  const preference = computed<AppearancePreference>({
    get: () => config.value.settings.appearance ?? 'system',
    set: (value) => { config.value.settings.appearance = value },
  })
  const resolved = computed<'light' | 'dark'>(() =>
    preference.value === 'dark' || (preference.value === 'system' && systemDark.value)
      ? 'dark' : 'light',
  )
  const onChange = (event: MediaQueryListEvent) => { systemDark.value = event.matches }
  media?.addEventListener('change', onChange)
  onScopeDispose(() => media?.removeEventListener('change', onChange))
  watch([loaded, resolved], ([isLoaded, mode]) => {
    if (!isLoaded || !import.meta.client) return
    document.documentElement.classList.toggle('dark', mode === 'dark')
    document.documentElement.classList.toggle('light', mode === 'light')
  }, { immediate: true })

  function toggle() {
    preference.value = resolved.value === 'dark' ? 'light' : 'dark'
  }

  singleton = { preference, resolved, toggle }
  return singleton
}
