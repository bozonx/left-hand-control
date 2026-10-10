import { createI18n } from 'vue-i18n'
import { DEFAULT_LOCALE, messages, SUPPORTED_LOCALES } from '~/i18n'

// Register vue-i18n with the Vue app. The active locale is driven by
// `useAppLocale()`, which reads the user's preference from config and
// writes it back into this instance via `i18n.global.locale.value`.
export default defineNuxtPlugin((nuxtApp) => {
  const i18n = createI18n({
    legacy: false,
    globalInjection: true,
    locale: DEFAULT_LOCALE,
    fallbackLocale: DEFAULT_LOCALE,
    availableLocales: [...SUPPORTED_LOCALES],
    messages,
    missingWarn: false,
    fallbackWarn: false,
  })
  nuxtApp.vueApp.use(i18n)
})
