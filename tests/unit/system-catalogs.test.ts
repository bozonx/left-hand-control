import { describe, expect, it } from 'vitest'

import { systemActionById, SYSTEM_ACTIONS } from '~/utils/systemActions'
import { systemMacroById, SYSTEM_MACROS } from '~/utils/systemMacros'

describe('system action catalog', () => {
  it('provides unique ids and indexed lookup', () => {
    const ids = SYSTEM_ACTIONS.map((action) => action.id)

    expect(new Set(ids).size).toBe(ids.length)
    expect(systemActionById('switchDesktop1')).toMatchObject({
      id: 'switchDesktop1',
      nameKey: 'systemActions.switchDesktop',
      nameParams: { n: 1 },
      platforms: ['linux-kde'],
    })
    expect(systemActionById('taskEntry10')).toMatchObject({
      id: 'taskEntry10',
      nameParams: { n: 10 },
    })
    expect(systemActionById('missing')).toBeUndefined()
  })
})

describe('system macro catalog', () => {
  it('provides unique ids and macro lookup with ordered steps', () => {
    const ids = SYSTEM_MACROS.map((macro) => macro.id)

    expect(new Set(ids).size).toBe(ids.length)
    expect(systemMacroById('duplicateLine')).toMatchObject({
      id: 'duplicateLine',
      name: 'Duplicate line',
      steps: [
        { keystroke: 'End' },
        { keystroke: 'Shift+Home' },
        { keystroke: 'Ctrl+C' },
        { keystroke: 'End' },
        { keystroke: 'Enter' },
        { keystroke: 'Ctrl+V' },
      ],
    })
    expect(systemMacroById('missing')).toBeUndefined()
  })
})
