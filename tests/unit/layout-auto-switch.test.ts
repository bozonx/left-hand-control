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
    Pick<AppSettings, 'layoutOrder' | 'layoutConditions' | 'autoDefaultLayoutId'>
  > = {},
): Pick<AppSettings, 'layoutOrder' | 'layoutConditions' | 'autoDefaultLayoutId'> {
  return {
    layoutOrder: [],
    layoutConditions: {},
    autoDefaultLayoutId: undefined,
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

  it('blocks when rule has no whitelist', () => {
    expect(
      evaluateLayoutGate({ blacklist: { gameMode: 'on', layouts: [] } }, ctxOnUS),
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
  it('false when disabled flag set', () => {
    expect(isLayoutInAuto({ whitelist: { layouts: ['us'] }, disabledInAuto: true })).toBe(false)
  })
  it('true when whitelist defined', () => {
    expect(isLayoutInAuto({ whitelist: { layouts: ['us'] } })).toBe(true)
  })
  it('false when only blacklist defined (whitelist required)', () => {
    expect(isLayoutInAuto({ blacklist: { layouts: ['us'] } })).toBe(false)
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
          'user:a': { whitelist: { layouts: ['us'] } },
          'user:b': { whitelist: { layouts: ['us'] } },
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
            whitelist: { layouts: ['us'] },
            blacklist: { gameMode: 'on', layouts: [] },
          },
          'user:b': { whitelist: { layouts: ['us'] } },
        },
      }),
      ctxOnUSGame,
    )
    expect(result).toBe('user:b')
  })

  it('falls back to autoDefaultLayoutId when no match', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:default'],
      settings({
        autoDefaultLayoutId: 'user:default',
        layoutConditions: {
          'user:a': { whitelist: { layouts: ['ru'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBe('user:default')
  })

  it('returns null when default id is missing from available', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        autoDefaultLayoutId: 'user:gone',
        layoutConditions: {
          'user:a': { whitelist: { layouts: ['ru'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBeNull()
  })

  it('does not pick a layout that has no conditions', () => {
    // A layout without whitelist/blacklist is not a candidate.
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': {},
        },
      }),
      ctxOnRU,
    )
    expect(result).toBeNull()
  })

  it('falls back to default layout', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        autoDefaultLayoutId: 'user:b',
        layoutConditions: {
          'user:a': { whitelist: { layouts: ['ru'] } },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBe('user:b')
  })

  it('does not fall back to default when it is disabled', () => {
    const result = pickActiveLayout(
      ['user:a', 'user:b'],
      settings({
        autoDefaultLayoutId: 'user:b',
        layoutConditions: {
          'user:a': { whitelist: { layouts: ['ru'] } },
          'user:b': { disabledInAuto: true },
        },
      }),
      ctxOnUS,
    )
    expect(result).toBeNull()
  })

  it('does not pick a layout that has only a blacklist (whitelist required)', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { blacklist: { gameMode: 'on', layouts: [] } },
        },
      }),
      ctxOnRU,
    )
    expect(result).toBeNull()
  })
})
