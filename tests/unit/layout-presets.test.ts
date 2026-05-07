import { describe, expect, it } from 'vitest'

import { createDefaultConfig, createDefaultEmojiPage } from '~/types/config'
import {
  applyPresetToConfig,
  builtinLayoutName,
  emptyLayoutPreset,
  extractPresetFromConfig,
  layoutSnapshotOf,
  localizeBuiltinLayoutPreset,
  parseLayoutYaml,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'

describe('layout preset helpers', () => {
  it('parses yaml into normalized preset data', () => {
    const preset = parseLayoutYaml(`
name: Navigation
description: "  Primary nav layer  "
layers:
  - id: nav
    name: Nav
    keys:
      KeyH: ArrowLeft
      KeyJ: ~
      KeyL: ArrowRight
    extras:
      - key: MouseSide
        action: BrowserBack
rules:
  - key: CapsLock
    layer: nav
    tap: Escape
    hold: ~
    dtap: Ctrl+Alt+KeyT
    holdMs: 150
    dtapMs: 220
commands:
  - id: terminal
    name: Terminal
    linux: kitty
quickActions:
  - id: play
    name: Play
    action: MediaPlayPause
    icon: i-lucide-play
  - id: draft
    name: Draft action
macros:
  - id: duplicateLine
    steps:
      - Ctrl+KeyC
      - action: Ctrl+KeyV
        id: step-2
    stepPauseMs: 30
`)

    expect(preset).not.toBeNull()
    expect(preset).toMatchObject({
      description: 'Primary nav layer',
      layers: [{ id: 'nav', name: 'Nav' }],
      rules: [
        {
          key: 'CapsLock',
          layerId: 'nav',
          tapAction: 'Escape',
          holdAction: null,
          doubleTapAction: 'Ctrl+Alt+KeyT',
          holdTimeoutMs: 150,
          doubleTapTimeoutMs: 220,
        },
      ],
      layerKeymaps: {
        nav: {
          keys: {
            KeyH: 'ArrowLeft',
            KeyJ: null,
            KeyL: 'ArrowRight',
          },
          extras: [
            {
              key: 'MouseSide',
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
            { action: 'Ctrl+KeyC' },
            { id: 'step-2', action: 'Ctrl+KeyV' },
          ],
        },
      ],
      commands: [
        {
          id: 'terminal',
          name: 'Terminal',
          linux: 'kitty',
        },
      ],
      quickActions: [
        {
          id: 'play',
          name: 'Play',
          action: 'MediaPlayPause',
          icon: 'i-lucide-play',
        },
        {
          id: 'draft',
          name: 'Draft action',
          action: '',
        },
      ],
    })
  })

  it('preserves explicit text actions in layers, rules and macros', () => {
    const preset = parseLayoutYaml(`
layers:
  - id: sym
    keys:
      KeyA: "text:€"
rules:
  - key: AltRight
    tap: "text:."
macros:
  - id: snippet
    steps:
      - "text:TODO: "
`)

    expect(preset).not.toBeNull()
    expect(preset).toMatchObject({
      layerKeymaps: {
        sym: {
          keys: {
            KeyA: 'text:€',
          },
        },
      },
      rules: [
        {
          key: 'AltRight',
          tapAction: 'text:.',
        },
      ],
      macros: [
        {
          id: 'snippet',
          steps: [{ action: 'text:TODO: ' }],
        },
      ],
    })

    const yaml = serializeLayoutYaml(preset!)
    expect(yaml).toContain('KeyA: text:€')
    expect(yaml).toContain('tap: text:.')
    expect(yaml).toContain("- 'text:TODO: '")
  })

  it('serializes and parses a preset without losing semantic values', () => {
    const original = {
      description: 'editing helpers',
      layers: [{ id: 'edit', name: 'Edit', description: 'editing layer' }],
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
          keys: { KeyH: 'ArrowLeft', KeyJ: null },
          extras: [{ id: 'extra-1', key: 'MouseSide', action: 'BrowserBack' }],
        },
      },
      macros: [
        {
          id: 'copyLine',
          name: 'Copy line',
          steps: [
            { id: 's1', action: 'Home' },
            { id: 's2', action: 'Shift+End' },
          ],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
      commands: [
        {
          id: 'terminal',
          name: 'Terminal',
          linux: 'kitty',
        },
      ],
      quickActions: [
        {
          id: 'play',
          name: 'Play',
          action: 'MediaPlayPause',
          icon: 'i-lucide-play',
        },
        {
          id: 'draft',
          name: 'Draft action',
          action: '',
        },
      ],
    }

    const yaml = serializeLayoutYaml(original)
    const reparsed = parseLayoutYaml(yaml)

    expect(reparsed).not.toBeNull()
    expect(reparsed).toMatchObject({
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
          keys: { KeyH: 'ArrowLeft', KeyJ: null },
          extras: [{ key: 'MouseSide', action: 'BrowserBack' }],
        },
      },
      macros: [
        {
          id: 'copyLine',
          name: 'Copy line',
          steps: [{ action: 'Home' }, { action: 'Shift+End' }],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
      commands: [
        {
          id: 'terminal',
          name: 'Terminal',
          linux: 'kitty',
        },
      ],
      quickActions: original.quickActions,
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
      extras: [{ id: 'extra-1', key: 'MouseSide', action: 'BrowserBack' }],
    }
    config.macros.push({
      id: 'duplicateLine',
      name: 'Duplicate line',
      steps: [{ id: 'step-1', action: 'Ctrl+KeyC' }],
    })
    config.commands.push({
      id: 'terminal',
      name: 'Terminal',
      linux: 'kitty',
    })
    config.quickActions.push({
      id: 'play',
      name: 'Play',
      action: 'MediaPlayPause',
      icon: 'i-lucide-play',
    })
    config.settings.appearance = 'dark'

    const preset = extractPresetFromConfig(config)
    const next = applyPresetToConfig(
      createDefaultConfig(),
      preset,
      'user:current',
    )

    expect(next.settings.appearance).toBe('system')
    expect(next.settings.currentLayoutId).toBe('user:current')
    expect(next.layers).toEqual(config.layers)
    expect(next.rules).toEqual(config.rules)
    expect(next.layerKeymaps).toEqual(config.layerKeymaps)
    expect(next.macros).toEqual(config.macros)
    expect(next.commands).toEqual(config.commands)
    expect(next.quickActions).toEqual(config.quickActions)

    preset.layers[0]!.name = 'Changed later'
    preset.layerKeymaps.nav!.keys.KeyH = 'Changed'
    preset.commands[0]!.linux = 'changed'
    preset.quickActions[0]!.name = 'Changed action'
    expect(next.layers[0]!.name).toBe('Navigation')
    expect(next.layerKeymaps.nav!.keys.KeyH).toBe('ArrowLeft')
    expect(next.commands[0]!.linux).toBe('kitty')
    expect(next.quickActions[0]!.name).toBe('Play')
  })

  it('creates stable layout snapshots and empty presets without implicit layers', () => {
    const config = createDefaultConfig()
    const initial = layoutSnapshotOf(config)

    config.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: '',
      tapAction: 'Escape',
      holdAction: '',
      doubleTapAction: '',
    })

    expect(layoutSnapshotOf(config)).not.toBe(initial)
    expect(emptyLayoutPreset()).toEqual({
      layers: [],
      rules: [],
      commands: [],
      macros: [],
      layerKeymaps: {},
      quickActions: [],
      emojiPages: [],
    })
  })

  it('returns null for invalid or non-object yaml', () => {
    expect(parseLayoutYaml('not: [valid')).toBeNull()
    expect(parseLayoutYaml('hello')).toBeNull()
  })

  it('parses emojiPages from yaml', () => {
    const preset = parseLayoutYaml(`
emojiPages:
  - id: page1
    name: Reactions
    cells:
      KeyQ: "😀"
      KeyW: "👍"
      KeyE: "❤️"
`)
    expect(preset).not.toBeNull()
    expect(preset!.emojiPages).toHaveLength(1)
    expect(preset!.emojiPages[0]).toMatchObject({
      id: 'page1',
      name: 'Reactions',
      cells: {
        KeyQ: '😀',
        KeyW: '👍',
        KeyE: '❤️',
      },
    })
  })

  it('serializes emojiPages to yaml and round-trips without loss', () => {
    const original = parseLayoutYaml(`
emojiPages:
  - id: page1
    name: Smileys
    cells:
      KeyQ: "😀"
      KeyA: "🔥"
      KeyZ: "✨"
`)
    expect(original).not.toBeNull()
    const yaml = serializeLayoutYaml(original!)
    const reparsed = parseLayoutYaml(yaml)
    expect(reparsed).not.toBeNull()
    expect(reparsed!.emojiPages[0]).toMatchObject({
      id: 'page1',
      name: 'Smileys',
      cells: { KeyQ: '😀', KeyA: '🔥', KeyZ: '✨' },
    })
  })

  it('ignores legacy single-char emoji hotkeys', () => {
    const preset = parseLayoutYaml(`
emojiPages:
  - id: legacy
    name: Old format
    cells:
      q: "😀"
      w: "👍"
      a: "🔥"
`)
    expect(preset).not.toBeNull()
    const cells = preset!.emojiPages[0]!.cells
    expect(Object.keys(cells)).toHaveLength(0)
  })

  it('filters out invalid hotkey names in emojiPages cells', () => {
    const preset = parseLayoutYaml(`
emojiPages:
  - id: p1
    name: Test
    cells:
      KeyQ: "😀"
      InvalidKey: "💥"
      "": "🤔"
`)
    expect(preset).not.toBeNull()
    const cells = preset!.emojiPages[0]!.cells
    expect(cells['KeyQ' as keyof typeof cells]).toBe('😀')
    expect(Object.keys(cells)).toHaveLength(1)
  })

  it('falls back to a default emoji page when emojiPages is empty or absent', () => {
    const fromEmpty = parseLayoutYaml(`
emojiPages: []
`)
    const fromAbsent = parseLayoutYaml(`
quickActions: []
`)
    const defaultPage = createDefaultEmojiPage()
    expect(fromEmpty!.emojiPages).toHaveLength(1)
    expect(fromAbsent!.emojiPages).toHaveLength(1)
    expect(fromEmpty!.emojiPages[0]!.cells).toMatchObject(defaultPage.cells)
  })

  it('preserves multiple emoji pages on round-trip', () => {
    const preset = parseLayoutYaml(`
emojiPages:
  - id: p1
    name: Page One
    cells:
      KeyQ: "😀"
  - id: p2
    name: Page Two
    cells:
      KeyW: "🎉"
`)
    expect(preset!.emojiPages).toHaveLength(2)
    const yaml = serializeLayoutYaml(preset!)
    const reparsed = parseLayoutYaml(yaml)
    expect(reparsed!.emojiPages).toHaveLength(2)
    expect(reparsed!.emojiPages[0]!.name).toBe('Page One')
    expect(reparsed!.emojiPages[1]!.name).toBe('Page Two')
  })

  it('extracts and applies emojiPages in preset round-trip while cloning data', () => {
    const config = createDefaultConfig()
    config.emojiPages = [
      {
        id: 'p1',
        name: 'My Emojis',
        cells: { KeyQ: '😀', KeyW: '🔥' },
      },
    ]

    const preset = extractPresetFromConfig(config)
    const next = applyPresetToConfig(createDefaultConfig(), preset, undefined)

    expect(next.emojiPages).toEqual(config.emojiPages)

    preset.emojiPages[0]!.name = 'Changed'
    expect(next.emojiPages[0]!.name).toBe('My Emojis')
  })

  it('localizes the built-in preset from i18n without changing the user yaml format', () => {
    const preset = localizeBuiltinLayoutPreset(
      {
        description: 'Fallback description',
        layers: [
          { id: 'nav', name: 'nav' },
          { id: 'sel', name: 'sel', description: 'selection fallback' },
        ],
        rules: [],
        layerKeymaps: {},
        macros: [],
        commands: [],
        quickActions: [],
      },
      (key) =>
        ({
          'builtinLayouts.ivank.name': 'Localized built-in',
          'builtinLayouts.ivank.description': 'Localized description',
          'builtinLayouts.ivank.layers.nav.name': 'Navigation',
          'builtinLayouts.ivank.layers.nav.description': 'Localized nav',
        })[key] ?? key,
    )

    expect(preset).toMatchObject({
      description: 'Localized description',
      layers: [
        { id: 'nav', name: 'Navigation', description: 'Localized nav' },
        { id: 'sel', name: 'sel', description: 'selection fallback' },
      ],
    })
    expect(
      builtinLayoutName((key) =>
        key === 'builtinLayouts.ivank.name' ? 'Localized built-in' : key,
      ),
    ).toBe('Localized built-in')
  })
})
