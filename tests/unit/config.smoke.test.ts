import { describe, expect, it } from 'vitest'

import {
  commandActionRef,
  createDefaultConfig,
  parseTextAction,
  macroActionRef,
  parseCommandRef,
  parseMacroRef,
  parseSystemRef,
  systemActionRef,
  textActionRef,
} from '~/types/config'
import { normalizeConfig, parsePersistedConfig } from '~/composables/useConfig'
import { isCanonicalAction } from '~/utils/actionSyntax'

describe('config helpers', () => {
  it('creates the expected default config shape', () => {
    const config = createDefaultConfig()

    expect(config.version).toBe(1)
    expect(config.layers).toEqual([])
    expect(config.layerKeymaps).toEqual({})
    expect(config.settings.appearance).toBe('system')
  })

  it('round-trips action references', () => {
    expect(parseMacroRef(macroActionRef('duplicateLine'))).toBe('duplicateLine')
    expect(parseCommandRef(commandActionRef('toggleMusic'))).toBe('toggleMusic')
    expect(parseSystemRef(systemActionRef('switchDesktop1'))).toBe(
      'switchDesktop1',
    )
    expect(parseTextAction(textActionRef('TODO: '))).toBe('TODO: ')
    expect(parseMacroRef('Enter')).toBeNull()
    expect(parseCommandRef('Enter')).toBeNull()
    expect(parseSystemRef('')).toBeNull()
  })

  it('accepts canonical actions and rejects legacy forms', () => {
    expect(isCanonicalAction('KeyA')).toBe(true)
    expect(isCanonicalAction('Digit1')).toBe(true)
    expect(isCanonicalAction('ArrowLeft')).toBe(true)
    expect(isCanonicalAction('Ctrl+KeyC')).toBe(true)
    expect(isCanonicalAction('Ctrl+Shift+ArrowLeft')).toBe(true)
    expect(isCanonicalAction('text:TODO: ')).toBe(true)
    expect(isCanonicalAction('Calculator')).toBe(true)
    expect(isCanonicalAction('MediaStop')).toBe(true)
    expect(isCanonicalAction('Numpad5')).toBe(true)
    expect(isCanonicalAction('MouseLeft')).toBe(true)
    expect(isCanonicalAction('LaunchMail')).toBe(true)

    expect(isCanonicalAction('Esc')).toBe(false)
    expect(isCanonicalAction('Left')).toBe(false)
    expect(isCanonicalAction('Ctrl+C')).toBe(false)
    expect(isCanonicalAction('1')).toBe(false)
    expect(isCanonicalAction('€')).toBe(false)
  })

  it('normalizes partial persisted config into a complete shape', () => {
    const config = normalizeConfig({
      layers: [{ id: 'nav', name: 'Navigation' }],
      rules: [
        {
          id: 'rule-1',
          key: 'Space',
          layerId: 'nav',
          tapAction: undefined,
          holdAction: undefined,
        },
      ],
      layerKeymaps: {
        nav: {
          keys: null,
        },
      },
      settings: {
        appearance: 'dark',
      },
    })

    expect(config.layers.map((layer) => layer.id)).toEqual(['nav'])
    expect(config.layerKeymaps.nav).toEqual({ keys: {}, extras: [] })
    expect(config.rules[0]).toMatchObject({
      tapAction: '',
      holdAction: '',
      doubleTapAction: '',
    })
    expect(config.settings.appearance).toBe('dark')
    expect(config.settings.locale).toBe('auto')
  })

  it('preserves manual selection and full auto-condition state', () => {
    const config = normalizeConfig({
      settings: {
        currentLayoutId: 'user:nav',
        layoutMode: 'manual',
        layoutConditions: {
          'user:nav': {
            whitelist: {
              gameMode: 'off',
              layouts: ['us'],
              apps: ['code', 'kitty'],
            },
            disabledInAuto: true,
          },
        },
      },
    })

    expect(config.settings.manualActiveLayoutId).toBe('user:nav')
    expect(config.settings.layoutConditions['user:nav']).toEqual({
      whitelist: {
        gameMode: 'off',
        layouts: ['us'],
        apps: ['code', 'kitty'],
      },
      disabledInAuto: true,
    })
  })

  it('preserves explicit null layer mappings as swallow', () => {
    const config = normalizeConfig({
      layers: [{ id: 'nav', name: 'Navigation' }],
      layerKeymaps: {
        nav: {
          keys: {
            KeyH: null,
            KeyJ: 'ArrowDown',
            KeyK: '',
          },
          extras: [],
        },
      },
    })

    expect(config.layerKeymaps.nav?.keys).toEqual({
      KeyH: null,
      KeyJ: 'ArrowDown',
      KeyK: null,
    })
  })

  it('throws a readable error for invalid persisted json', () => {
    expect(() => parsePersistedConfig('{')).toThrow(/config\.json is invalid/i)
  })
})
