import { describe, expect, it } from 'vitest'

import {
  commandActionRef,
  createDefaultConfig,
  macroActionRef,
  parseCommandRef,
  parseMacroRef,
  parseSystemRef,
  systemActionRef,
} from '~/types/config'
import { normalizeConfig, parsePersistedConfig } from '~/composables/useConfig'

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
    expect(parseMacroRef('Enter')).toBeNull()
    expect(parseCommandRef('Enter')).toBeNull()
    expect(parseSystemRef('')).toBeNull()
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
