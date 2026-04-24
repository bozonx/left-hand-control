import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { commandActionRef, createDefaultConfig } from '~/types/config'
import { useCommandEditor } from '~/composables/useCommandEditor'

const { useConfigMock } = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)

function makeConfigState() {
  return {
    config: ref(createDefaultConfig()),
  }
}

describe('useCommandEditor', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
  })

  it('adds commands, validates them and tracks usage across rules, layers and macros', async () => {
    const state = makeConfigState()
    state.config.value.layers.push({ id: 'nav', name: 'Navigation' })
    state.config.value.layerKeymaps.nav = { keys: {}, extras: [] }
    state.config.value.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: '',
      tapAction: commandActionRef('terminal'),
      holdAction: '',
      doubleTapAction: '',
    })
    state.config.value.layerKeymaps.nav.keys.KeyH = commandActionRef('terminal')
    state.config.value.layerKeymaps.nav.extras.push({
      id: 'extra-1',
      name: 'Mouse4',
      action: commandActionRef('terminal'),
    })
    state.config.value.macros.push({
      id: 'macro-1',
      name: 'Macro',
      steps: [{ id: 'step-1', keystroke: commandActionRef('terminal') }],
    })
    useConfigMock.mockReturnValue(state)

    const Harness = defineComponent({
      setup() {
        return useCommandEditor()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    vm.addCommand()
    const firstCommand = state.config.value.commands[0]!
    expect(firstCommand.name).toBe('New command')
    expect(firstCommand.linux).toBe('')
    expect(vm.linuxError(firstCommand)).toBe('Linux command cannot be empty.')

    firstCommand.id = 'terminal'
    firstCommand.linux = 'kitty'
    expect(vm.idError(firstCommand)).toBeNull()
    expect(vm.linuxError(firstCommand)).toBeNull()
    expect(vm.hasErrors).toBe(false)

    state.config.value.commands.push({
      id: 'terminal',
      name: 'Terminal 2',
      linux: 'wezterm',
    })
    const duplicate = state.config.value.commands[1]!
    expect(vm.idError(firstCommand)).toBe('This ID is already used by another command.')
    expect(vm.hasErrors).toBe(true)

    duplicate.id = 'wezterm'
    expect(vm.idError(duplicate)).toBeNull()

    const usage = vm.usage
    expect(usage.terminal).toEqual([
      'rule CapsLock (tap)',
      'nav.KeyH',
      'nav.Mouse4',
      'macro macro-1 (#1)',
    ])

    vm.moveCommand(vm.uiKeyOf(duplicate), -1)
    expect(state.config.value.commands[0]?.id).toBe('wezterm')

    vm.removeCommand(vm.uiKeyOf(firstCommand))
    expect(state.config.value.commands.some((command: any) => command.id === 'terminal')).toBe(false)
  })
})
