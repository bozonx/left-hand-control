import { describe, expect, it } from 'vitest'

import {
  BASE_LAYER_ID,
  createDefaultConfig,
  macroActionRef,
  parseMacroRef,
  parseSystemRef,
  systemActionRef,
} from '~/types/config'

describe('config helpers', () => {
  it('creates the expected default config shape', () => {
    const config = createDefaultConfig()

    expect(config.version).toBe(1)
    expect(config.layers).toEqual([{ id: BASE_LAYER_ID, name: 'Base' }])
    expect(config.layerKeymaps[BASE_LAYER_ID]).toEqual({
      keys: {},
      extras: [],
    })
    expect(config.settings.appearance).toBe('system')
  })

  it('round-trips action references', () => {
    expect(parseMacroRef(macroActionRef('duplicateLine'))).toBe('duplicateLine')
    expect(parseSystemRef(systemActionRef('switchDesktop1'))).toBe(
      'switchDesktop1',
    )
    expect(parseMacroRef('Enter')).toBeNull()
    expect(parseSystemRef('')).toBeNull()
  })
})
