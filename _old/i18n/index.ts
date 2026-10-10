import enUS from './locales/en-US'
import ruRU from './locales/ru-RU'

// Supported UI locales. The first entry is the fallback.
export const SUPPORTED_LOCALES = ['en-US', 'ru-RU'] as const
export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number]

export const DEFAULT_LOCALE: SupportedLocale = 'en-US'

export const messages: Record<SupportedLocale, typeof enUS> = {
  'en-US': enUS,
  'ru-RU': ruRU,
}

// Human-readable locale names, used in the language selector. Sourced from
// each locale's own `language.name` so the list is always shown in the
// native form of that language.
export function localeDisplayName(locale: SupportedLocale): string {
  return messages[locale].language.name
}

// Pick the best-matching supported locale for a raw BCP-47 tag like "ru",
// "ru-RU", "en-GB". Matching is done on the left-hand language subtag:
// if any supported locale has the same language code, the first one wins.
// Returns `null` if no match is found.
export function matchSupportedLocale(tag: string): SupportedLocale | null {
  if (!tag) return null
  const base = tag.toLowerCase().split(/[-_]/)[0]
  if (!base) return null
  for (const loc of SUPPORTED_LOCALES) {
    if (loc.toLowerCase().split('-')[0] === base) return loc
  }
  return null
}

// Detect a supported locale from the browser / OS. Falls back to
// DEFAULT_LOCALE if nothing matches.
export function detectSystemLocale(): SupportedLocale {
  if (typeof navigator === 'undefined') return DEFAULT_LOCALE
  const candidates: string[] = []
  if (Array.isArray(navigator.languages)) candidates.push(...navigator.languages)
  if (navigator.language) candidates.push(navigator.language)
  for (const tag of candidates) {
    const match = matchSupportedLocale(tag)
    if (match) return match
  }
  return DEFAULT_LOCALE
}
