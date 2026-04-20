import { useI18n } from 'vue-i18n'
import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  type SupportedLocale,
  detectSystemLocale,
} from '~/i18n'
import type { LocalePreference } from '~/types/config'

// Bridges the persisted `settings.locale` with the vue-i18n instance.
//
// `config.settings.locale` is the single source of truth:
//   - 'auto'  — resolve from the OS language (navigator.languages) with
//               fallback to English;
//   - 'en-US' / 'ru-RU' — explicit override.
//
// Whenever the preference changes, the resolved locale is written into
// `i18n.global.locale` so every `$t()` / `t()` call reactively updates.

interface AppLocaleApi {
  // Raw user preference (what is stored in config).
  preference: Ref<LocalePreference>
  // Actual locale currently in effect after honoring 'auto'.
  resolved: Ref<SupportedLocale>
  // Locale detected from the OS (navigator.languages). Used to label the
  // "Auto" option in the UI.
  systemLocale: Ref<SupportedLocale>
  // Available locales, in display order (fallback first).
  available: readonly SupportedLocale[]
}

let singleton: AppLocaleApi | null = null

export function useAppLocale(): AppLocaleApi {
  if (singleton) return singleton

  const { config, loaded } = useConfig()
  const i18n = useI18n()

  const systemLocale = ref<SupportedLocale>(detectSystemLocale())

  const preference = computed<LocalePreference>({
    get: () => (config.value.settings.locale ?? 'auto'),
    set: (v) => {
      config.value.settings.locale = v
    },
  })

  const resolved = computed<SupportedLocale>(() => {
    const pref = preference.value
    if (pref === 'auto') return systemLocale.value
    if (SUPPORTED_LOCALES.includes(pref as SupportedLocale)) {
      return pref as SupportedLocale
    }
    return DEFAULT_LOCALE
  })

  watch(
    [loaded, resolved],
    ([isLoaded, loc]) => {
      if (!isLoaded) return
      i18n.locale.value = loc
      if (typeof document !== 'undefined') {
        document.documentElement.lang = loc
      }
    },
    { immediate: true },
  )

  singleton = {
    preference,
    resolved,
    systemLocale,
    available: SUPPORTED_LOCALES,
  }
  return singleton
}
