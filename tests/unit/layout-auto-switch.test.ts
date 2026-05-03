import { describe, expect, it } from 'vitest'

import type { AppSettings } from '~/types/config'
import {
  evaluateLayoutGate,
  isLayoutInAuto,
  matchesConditionSet,
  orderLayoutIds,
  pickActiveLayout,
} from '~/utils/layoutAutoSwitch'

const ctxOnUS = { currentSystemLayout: 'us', gameModeActive: false }
const ctxOnRU = { currentSystemLayout: 'ru', gameModeActive: false }
const ctxOnUSGame = { currentSystemLayout: 'us', gameModeActive: true }

function settings(
  partial: Partial<
    Pick<AppSettings, 'layoutOrder' | 'layoutConditions'>
  > = {},
): Pick<AppSettings, 'layoutOrder' | 'layoutConditions'> {
  return {
    layoutOrder: [],
    layoutConditions: {},
    ...partial,
  }
}

describe('matchesConditionSet', () => {
  it('matches when gameMode and layouts pass', () => {
    expect(
      matchesConditionSet({ gameMode: 'on', layouts: ['us'] }, ctxOnUSGame),
    ).toBe(true)
  })

  it('rejects when gameMode mismatches', () => {
    expect(
      matchesConditionSet({ gameMode: 'on', layouts: [] }, ctxOnUS),
    ).toBe(false)
  })

  it('rejects when layout list excludes current', () => {
    expect(
      matchesConditionSet({ layouts: ['ru'] }, ctxOnUS),
    ).toBe(false)
  })

  it('matches when layouts list is empty regardless of current', () => {
    expect(matchesConditionSet({ layouts: [] }, ctxOnUS)).toBe(true)
  })

  it('matches when apps substring is found in window title (case-insensitive)', () => {
    expect(
      matchesConditionSet(
        { layouts: [], apps: ['firefox'] },
        { ...ctxOnUS, activeWindowTitle: 'Mozilla Firefox', activeWindowAppId: 'navigator' },
      ),
    ).toBe(true)
  })

  it('matches when apps substring is found in app id', () => {
    expect(
      matchesConditionSet(
        { layouts: [], apps: ['Steam'] },
        { ...ctxOnUS, activeWindowTitle: 'Library', activeWindowAppId: 'steam' },
      ),
    ).toBe(true)
  })

  it('rejects when apps non-empty and active window unknown', () => {
    expect(
      matchesConditionSet(
        { layouts: [], apps: ['firefox'] },
        { ...ctxOnUS, activeWindowTitle: null, activeWindowAppId: null },
      ),
    ).toBe(false)
  })

  it('rejects when no apps substring matches', () => {
    expect(
      matchesConditionSet(
        { layouts: [], apps: ['firefox', 'chrome'] },
        { ...ctxOnUS, activeWindowTitle: 'KeePassXC', activeWindowAppId: 'keepassxc' },
      ),
    ).toBe(false)
  })
})

describe('evaluateLayoutGate', () => {
  it('returns allow when no rule', () => {
    expect(evaluateLayoutGate(undefined, ctxOnUS)).toBe('allow')
  })

  it('allows when only blacklist defined and it does not match', () => {
    expect(
      evaluateLayoutGate({ blacklist: { gameMode: 'on', layouts: [] } }, ctxOnUS),
    ).toBe('allow')
  })

  it('blocks when only blacklist defined and it matches', () => {
    expect(
      evaluateLayoutGate({ blacklist: { gameMode: 'on', layouts: [] } }, ctxOnUSGame),
    ).toBe('block')
  })

  it('blocks when blacklist matches even if whitelist also matches', () => {
    expect(
      evaluateLayoutGate(
        {
          whitelist: { layouts: ['us'] },
          blacklist: { gameMode: 'on', layouts: [] },
        },
        ctxOnUSGame,
      ),
    ).toBe('block')
  })

  it('blocks when whitelist is set and does not match', () => {
    expect(
      evaluateLayoutGate({ whitelist: { layouts: ['ru'] } }, ctxOnUS),
    ).toBe('block')
  })

  it('allows when whitelist matches', () => {
    expect(
      evaluateLayoutGate({ whitelist: { layouts: ['us'] } }, ctxOnUS),
    ).toBe('allow')
  })
})

describe('isLayoutInAuto', () => {
  it('false when no rule', () => {
    expect(isLayoutInAuto(undefined)).toBe(false)
  })
  it('false when the layout is not explicitly enabled', () => {
    expect(isLayoutInAuto({ whitelist: { layouts: ['us'] } })).toBe(false)
  })
  it('true when explicitly enabled without conditions', () => {
    expect(isLayoutInAuto({ enabledInAuto: true })).toBe(true)
  })
  it('true when explicitly enabled with conditions', () => {
    expect(isLayoutInAuto({ enabledInAuto: true, blacklist: { layouts: ['us'] } })).toBe(true)
  })
})

describe('orderLayoutIds', () => {
  it('uses layoutOrder priority and appends unknowns', () => {
    expect(orderLayoutIds(['a', 'b', 'c'], ['c', 'a'])).toEqual(['c', 'a', 'b'])
  })
  it('drops unknown order ids', () => {
    expect(orderLayoutIds(['a', 'b'], ['x', 'b'])).toEqual(['b', 'a'])
  })
})

describe('pickActiveLayout', () => {
  it('returns null when nothing is included in auto', () => {
    expect(
      pickActiveLayout(['user:a', 'user:b'], settings(), ctxOnUS),
    ).toBeNull()
  })

  it('picks first matching by priority', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        layoutOrder: ['user:b', 'user:a'],
        layoutConditions: {
          'user:a': { enabledInAuto: true, whitelist: { layouts: ['us'] } },
          'user:b': { enabledInAuto: true, whitelist: { layouts: ['us'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBe('user:b')
  })

  it('skips layouts with matching blacklist', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        layoutOrder: ['user:a', 'user:b'],
        layoutConditions: {
          'user:a': {
            enabledInAuto: true,
            whitelist: { layouts: ['us'] },
            blacklist: { gameMode: 'on', layouts: [] },
          },
          'user:b': { enabledInAuto: true, whitelist: { layouts: ['us'] } },
        },
      }),
      ctxOnUSGame,
    )
    expect(result).toBe('user:b')
  })

  it('returns null when no enabled layout matches', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { enabledInAuto: true, whitelist: { layouts: ['ru'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBeNull()
  })

  it('does not pick a layout that is not enabled', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { whitelist: { layouts: ['ru'] } },
        },
      }),
      ctxOnRU,
    )
    expect(result).toBeNull()
  })

  it('picks an enabled layout without conditions as a catch-all', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        layoutConditions: {
          'user:a': { enabledInAuto: true, whitelist: { layouts: ['ru'] } },
          'user:b': { enabledInAuto: true },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBe('user:b')
  })

  it('uses order when a catch-all and a conditional layout both match', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        layoutOrder: ['user:b', 'user:a'],
        layoutConditions: {
          'user:a': { enabledInAuto: true },
          'user:b': { enabledInAuto: true, whitelist: { layouts: ['us'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBe('user:b')
  })

  it('picks a blacklist-only layout when its blacklist does not match', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { enabledInAuto: true, blacklist: { gameMode: 'on', layouts: [] } },
        },
      }),
      ctxOnRU,
    )
    expect(result).toBe('user:a')
  })

  it('does not pick a blacklist-only layout when its blacklist matches', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { enabledInAuto: true, blacklist: { gameMode: 'on', layouts: [] } },
        },
      }),
      ctxOnUSGame,
    )
    expect(result).toBeNull()
  })
})
