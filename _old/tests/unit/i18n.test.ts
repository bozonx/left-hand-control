import { describe, expect, it, vi } from 'vitest'

import {
  DEFAULT_LOCALE,
  detectSystemLocale,
  localeDisplayName,
  matchSupportedLocale,
} from '~/i18n'

describe('i18n helpers', () => {
  it('matches supported locales by language subtag', () => {
    expect(matchSupportedLocale('ru')).toBe('ru-RU')
    expect(matchSupportedLocale('ru-BY')).toBe('ru-RU')
    expect(matchSupportedLocale('en-GB')).toBe('en-US')
    expect(matchSupportedLocale('de-DE')).toBeNull()
    expect(matchSupportedLocale('')).toBeNull()
  })

  it('detects locale from navigator.languages before navigator.language', () => {
    vi.stubGlobal('navigator', {
      languages: ['ru-RU', 'en-US'],
      language: 'en-US',
    })

    expect(detectSystemLocale()).toBe('ru-RU')
  })

  it('falls back to default locale when navigator is missing or unsupported', () => {
    vi.stubGlobal('navigator', {
      languages: ['de-DE'],
      language: 'fr-FR',
    })
    expect(detectSystemLocale()).toBe(DEFAULT_LOCALE)

    vi.unstubAllGlobals()
    expect(detectSystemLocale()).toBe(DEFAULT_LOCALE)
  })

  it('returns native display names from locale messages', () => {
    expect(localeDisplayName('en-US')).toBe('English')
    expect(localeDisplayName('ru-RU')).toBe('Русский')
  })
})
