import { describe, expect, it } from 'vitest'

import {
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import { ALL_KEYS, keyLabel, randomId } from '~/utils/keys'

describe('layout id helpers', () => {
  it('creates and parses user layout ids consistently', () => {
    expect(userLayoutId('Nav')).toBe('user:Nav')
    expect(isUserLayoutId('user:Nav')).toBe(true)
    expect(isUserLayoutId('builtin:ivank')).toBe(false)
    expect(isUserLayoutId(undefined)).toBe(false)
    expect(userLayoutNameFromId('user:Nav')).toBe('Nav')
    expect(userLayoutNameFromId('builtin:ivank')).toBe('builtin:ivank')
  })
})

describe('key helpers', () => {
  it('resolves known key labels and falls back to the code', () => {
    expect(keyLabel('CapsLock')).toBe('Caps')
    expect(keyLabel('ControlRight')).toBe('Ctrl')
    expect(keyLabel('ControlRight', 'code')).toBe('ControlRight')
    expect(keyLabel('ControlRight', 'numeric')).toBe('97')
    expect(keyLabel('UnknownKey', 'numeric')).toBe('UnknownKey')
    expect(keyLabel('UnknownKey')).toBe('UnknownKey')
  })

  it('exposes a flat unique key catalog and generates compact ids', () => {
    const uniqueCodes = new Set(ALL_KEYS.map((key) => key.code))

    expect(uniqueCodes.size).toBe(ALL_KEYS.length)

    const id = randomId()
    expect(id).toMatch(/^[a-z0-9]{8}$/)
  })
})
