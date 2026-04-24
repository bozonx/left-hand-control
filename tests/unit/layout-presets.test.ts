import { describe, expect, it } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import {
  applyPresetToConfig,
  emptyLayoutPreset,
  extractPresetFromConfig,
  layoutSnapshotOf,
  parseLayoutYaml,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'

describe('layout preset helpers', () => {
  it('parses yaml into normalized preset data', () => {
    const preset = parseLayoutYaml(
      `
name: Navigation
description: "  Primary nav layer  "
layers:
  - id: nav
    name: Nav
rules:
  - key: CapsLock
    layer: nav
    tap: Esc
    hold: ~
    dtap: Ctrl+Alt+T
    holdMs: 150
    dtapMs: 220
keymaps:
  nav:
    keys:
      KeyH: ArrowLeft
      KeyL: ArrowRight
      KeyX: null
    extras:
      - name: Mouse4
        action: BrowserBack
macros:
  - id: duplicateLine
    steps:
      - Ctrl+C
      - keystroke: Ctrl+V
        id: step-2
    stepPauseMs: 30
`,
      'Fallback',
    )

    expect(preset).not.toBeNull()
    expect(preset).toMatchObject({
      name: 'Navigation',
      description: 'Primary nav layer',
      layers: [{ id: 'nav', name: 'Nav' }],
      rules: [
        {
          key: 'CapsLock',
          layerId: 'nav',
          tapAction: 'Esc',
          holdAction: null,
          doubleTapAction: 'Ctrl+Alt+T',
          holdTimeoutMs: 150,
          doubleTapTimeoutMs: 220,
        },
      ],
      layerKeymaps: {
        nav: {
          keys: {
            KeyH: 'ArrowLeft',
            KeyL: 'ArrowRight',
          },
          extras: [
            {
              name: 'Mouse4',
              action: 'BrowserBack',
            },
          ],
        },
      },
      macros: [
        {
          id: 'duplicateLine',
          name: 'duplicateLine',
          stepPauseMs: 30,
          steps: [
            { keystroke: 'Ctrl+C' },
            { id: 'step-2', keystroke: 'Ctrl+V' },
          ],
        },
      ],
    })
  })

  it('serializes and parses a preset without losing semantic values', () => {
    const original = {
      name: 'Editing',
      description: 'editing helpers',
      layers: [
        { id: 'edit', name: 'Edit', description: 'editing layer' },
      ],
      rules: [
        {
          id: 'rule-1',
          key: 'CapsLock',
          layerId: 'edit',
          tapAction: '',
          holdAction: null,
          doubleTapAction: 'Enter',
          holdTimeoutMs: 210,
          doubleTapTimeoutMs: undefined,
        },
      ],
      layerKeymaps: {
        edit: {
          keys: { KeyH: 'ArrowLeft' },
          extras: [{ id: 'extra-1', name: 'Mouse4', action: 'BrowserBack' }],
        },
      },
      macros: [
        {
          id: 'copyLine',
          name: 'Copy line',
          steps: [{ id: 's1', keystroke: 'Home' }, { id: 's2', keystroke: 'Shift+End' }],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
    }

    const yaml = serializeLayoutYaml(original)
    const reparsed = parseLayoutYaml(yaml, 'Fallback')

    expect(reparsed).not.toBeNull()
    expect(reparsed).toMatchObject({
      name: 'Editing',
      description: 'editing helpers',
      layers: original.layers,
      rules: [
        {
          key: 'CapsLock',
          layerId: 'edit',
          tapAction: '',
          holdAction: null,
          doubleTapAction: 'Enter',
          holdTimeoutMs: 210,
          doubleTapTimeoutMs: undefined,
        },
      ],
      layerKeymaps: {
        edit: {
          keys: { KeyH: 'ArrowLeft' },
          extras: [{ name: 'Mouse4', action: 'BrowserBack' }],
        },
      },
      macros: [
        {
          id: 'copyLine',
          name: 'Copy line',
          steps: [{ keystroke: 'Home' }, { keystroke: 'Shift+End' }],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
    })
    expect(reparsed?.rules[0]?.id).toMatch(/^r_[a-z0-9]{8}$/)
  })

  it('extracts and applies presets while preserving settings and cloning layout data', () => {
    const config = createDefaultConfig()
    config.layers.push({ id: 'nav', name: 'Navigation' })
    config.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: 'nav',
      tapAction: '',
      holdAction: '',
      doubleTapAction: '',
    })
    config.layerKeymaps.nav = {
      keys: { KeyH: 'ArrowLeft' },
      extras: [{ id: 'extra-1', name: 'Mouse4', action: 'BrowserBack' }],
    }
    config.macros.push({
      id: 'duplicateLine',
      name: 'Duplicate line',
      steps: [{ id: 'step-1', keystroke: 'Ctrl+C' }],
    })
    config.settings.appearance = 'dark'

    const preset = extractPresetFromConfig(config, 'Current')
    const next = applyPresetToConfig(createDefaultConfig(), preset, 'user:current')

    expect(next.settings.appearance).toBe('system')
    expect(next.settings.currentLayoutId).toBe('user:current')
    expect(next.layers).toEqual(config.layers)
    expect(next.rules).toEqual(config.rules)
    expect(next.layerKeymaps).toEqual(config.layerKeymaps)
    expect(next.macros).toEqual(config.macros)

    preset.layers[0]!.name = 'Changed later'
    preset.layerKeymaps.nav!.keys.KeyH = 'Changed'
    expect(next.layers[0]!.name).toBe('Navigation')
    expect(next.layerKeymaps.nav!.keys.KeyH).toBe('ArrowLeft')
  })

  it('creates stable layout snapshots and empty presets without implicit layers', () => {
    const config = createDefaultConfig()
    const initial = layoutSnapshotOf(config)

    config.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: '',
      tapAction: 'Esc',
      holdAction: '',
      doubleTapAction: '',
    })

    expect(layoutSnapshotOf(config)).not.toBe(initial)
    expect(emptyLayoutPreset()).toEqual({
      name: 'Empty layout',
      layers: [],
      rules: [],
      layerKeymaps: {},
      macros: [],
    })
  })

  it('returns null for invalid or non-object yaml', () => {
    expect(parseLayoutYaml('not: [valid', 'Fallback')).toBeNull()
    expect(parseLayoutYaml('hello', 'Fallback')).toBeNull()
  })
})
