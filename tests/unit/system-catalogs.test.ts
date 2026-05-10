import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { systemActionById, SYSTEM_ACTIONS } from '~/utils/systemActions'
import { appActionById, APP_ACTIONS } from '~/utils/appActions'
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

describe('app action catalog', () => {
  it('provides unique ids and indexed lookup', () => {
    const ids = APP_ACTIONS.map((action) => action.id)

    expect(new Set(ids).size).toBe(ids.length)
    expect(appActionById('showQuickMenu1')).toMatchObject({
      id: 'showQuickMenu1',
      nameKey: 'appActions.showQuickMenu',
      nameParams: { n: 1 },
    })
    expect(appActionById('showEmojiMenu5')).toMatchObject({
      id: 'showEmojiMenu5',
      nameKey: 'appActions.showEmojiMenu',
      nameParams: { n: 5 },
    })
    expect(appActionById('showQuickMenu')).toBeUndefined()
    expect(appActionById('missing')).toBeUndefined()
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
        { action: 'End' },
        { action: 'Shift+Home' },
        { action: 'Ctrl+KeyC' },
        { action: 'End' },
        { action: 'Enter' },
        { action: 'Ctrl+KeyV' },
      ],
    })
    expect(systemMacroById('missing')).toBeUndefined()
  })

  it('stays in sync with the Rust runtime catalog', () => {
    const rust = readFileSync(
      join(process.cwd(), 'src-tauri/src/mapper/system_macros.rs'),
      'utf8',
    )

    for (const macro of SYSTEM_MACROS) {
      expect(rust).toContain(`id: "${macro.id}"`)
      for (const step of macro.steps) {
        expect(rust).toContain(`"${step.action}"`)
      }
    }

    const rustIds = [...rust.matchAll(/id:\s*"([^"]+)"/g)].map((m) => m[1])
    expect(new Set(rustIds).size).toBe(SYSTEM_MACROS.length)
  })
})
