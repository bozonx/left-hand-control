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
})

describe('evaluateLayoutGate', () => {
  it('returns allow when no rule', () => {
    expect(evaluateLayoutGate(undefined, ctxOnUS)).toBe('allow')
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
  it('true when explicit flag', () => {
    expect(isLayoutInAuto({ includedInAuto: true })).toBe(true)
  })
  it('true when whitelist defined', () => {
    expect(isLayoutInAuto({ whitelist: { layouts: ['us'] } })).toBe(true)
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

  it('does not pick a layout that is included but has no whitelist match', () => {
    // includedInAuto without conditions should not auto-win — only
    // matching whitelist counts. This protects the priority semantics.
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { includedInAuto: true },
        },
      }),
      ctxOnRU,
    )
    expect(result).toBeNull()
  })

  it('picks a layout that has no whitelist but has a non-blocking blacklist', () => {
    const result = pickActiveLayout(
      ['user:a'],
      settings({
        layoutConditions: {
          'user:a': { blacklist: { gameMode: 'on', layouts: [] } },
        },
      }),
      ctxOnRU,
    )
    expect(result).toBe('user:a')
  })
})
