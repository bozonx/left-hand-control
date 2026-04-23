import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createDefaultConfig, macroActionRef } from '~/types/config'
import { useMacroEditor } from '~/composables/useMacroEditor'
import { systemMacroById } from '~/utils/systemMacros'

const { useConfigMock } = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)

function makeConfigState() {
  return {
    config: ref(createDefaultConfig()),
  }
}

describe('useMacroEditor', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
  })

  it('adds and clones macros, validates ids, tracks usage and edits steps', async () => {
    const state = makeConfigState()
    state.config.value.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: '',
      tapAction: macroActionRef('dup'),
      holdAction: '',
      doubleTapAction: '',
    })
    state.config.value.layerKeymaps.base!.keys.KeyH = macroActionRef('dup')
    state.config.value.layerKeymaps.base!.extras.push({
      id: 'extra-1',
      name: 'Mouse4',
      action: macroActionRef('dup'),
    })
    useConfigMock.mockReturnValue(state)

    const Harness = defineComponent({
      setup() {
        return useMacroEditor()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    vm.addMacro()
    const firstMacro = state.config.value.macros[0]!
    expect(firstMacro.name).toBe('New macro')
    expect(firstMacro.steps).toEqual([])

    firstMacro.id = 'dup'
    expect(vm.idError(firstMacro)).toBeNull()
    expect(vm.hasIdErrors).toBe(false)

    vm.addStep(firstMacro)
    const stepId = firstMacro.steps[0]!.id
    firstMacro.steps[0]!.keystroke = 'Ctrl+C'
    vm.addStep(firstMacro)
    firstMacro.steps[1]!.keystroke = 'Ctrl+V'
    vm.moveStep(firstMacro, 1, -1)
    expect(firstMacro.steps.map((step) => step.keystroke)).toEqual(['Ctrl+V', 'Ctrl+C'])
    vm.removeStep(firstMacro, stepId)
    expect(firstMacro.steps).toHaveLength(1)

    const sameUiKey = vm.uiKeyOf(firstMacro)
    expect(vm.uiKeyOf(firstMacro)).toBe(sameUiKey)

    const sysMacro = systemMacroById('duplicateLine')!
    vm.cloneSystemMacro(sysMacro)
    const cloned = state.config.value.macros[1]!
    expect(cloned.id).toBe('duplicateLineCopy')
    expect(cloned.name).toBe('Duplicate line (copy)')
    expect(cloned.steps.map((step) => step.keystroke)).toEqual(
      sysMacro.steps.map((step) => step.keystroke),
    )

    const duplicate = {
      id: 'dup',
      name: 'Duplicate id',
      steps: [],
    }
    state.config.value.macros.push(duplicate)
    expect(vm.idError(firstMacro)).toBe('This ID is already used by another user macro.')
    expect(vm.hasIdErrors).toBe(true)

    duplicate.id = 'duplicateLine'
    expect(vm.idError(duplicate)).toBe(
      'This ID is already taken by system macro "Duplicate line".',
    )

    const usage = vm.usage
    expect(usage.dup).toEqual([
      'rule CapsLock (tap)',
      'base.KeyH',
      'base.Mouse4',
    ])

    vm.removeMacro(vm.uiKeyOf(firstMacro))
    expect(state.config.value.macros.some((macro) => macro.id === 'dup')).toBe(false)
  })
})
