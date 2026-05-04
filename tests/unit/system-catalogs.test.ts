import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

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
        { keystroke: 'Ctrl+KeyC' },
        { keystroke: 'End' },
        { keystroke: 'Enter' },
        { keystroke: 'Ctrl+KeyV' },
      ],
    })
    expect(systemMacroById('missing')).toBeUndefined()
  })

  it('stays in sync with the Rust runtime catalog', () => {
    const rust = readFileSync(
      join(process.cwd(), 'src-tauri/src/mapper/system_macros.rs'),
      'utf8',
    )
    const rustMacros = [...rust.matchAll(/id:\s*"([^"]+)",\s*steps:\s*&\[(.*?)\]/gs)]
      .map((match) => ({
        id: match[1],
        steps: [...match[2]!.matchAll(/"([^"]+)"/g)].map((step) => ({ keystroke: step[1] })),
      }))

    expect(rustMacros).toHaveLength(SYSTEM_MACROS.length)
    expect(SYSTEM_MACROS.map((macro) => ({
      id: macro.id,
      steps: macro.steps,
    }))).toEqual(rustMacros)
  })
})
