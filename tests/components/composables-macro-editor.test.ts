import { defineComponent, nextTick, ref } from 'vue'

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
    state.config.value.layers.push({ id: 'nav', name: 'Navigation' })
    state.config.value.layerKeymaps.nav = { keys: {}, extras: [] }
    state.config.value.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: '',
      tapAction: macroActionRef('dup'),
      holdAction: '',
      doubleTapAction: '',
    })
    state.config.value.layerKeymaps.nav!.keys.KeyH = macroActionRef('dup')
    state.config.value.layerKeymaps.nav!.extras.push({
      id: 'extra-1',
      key: 'MouseSide',
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
    expect(firstMacro.name).toBeTruthy()
    expect(firstMacro.steps).toEqual([])

    firstMacro.id = 'dup'
    expect(vm.idError(firstMacro)).toBeNull()
    expect(vm.hasIdErrors).toBe(false)
    expect(vm.hasStepErrors).toBe(false)
    expect(vm.hasErrors).toBe(false)

    vm.addStep(firstMacro)
    const stepId = firstMacro.steps[0]!.id
    expect(vm.stepWarning(firstMacro.steps[0])).toBeTruthy()
    firstMacro.steps[0]!.action = 'Ctrl+KeyC'
    expect(vm.stepError(firstMacro.steps[0])).toBeNull()
    expect(vm.stepWarning(firstMacro.steps[0])).toBeNull()
    vm.addStep(firstMacro)
    firstMacro.steps[1]!.action = 'macro:Ctrl+KeyV'
    vm.moveStep(firstMacro, 1, -1)
    expect(firstMacro.steps.map((step) => step.action)).toEqual(['macro:Ctrl+KeyV', 'Ctrl+KeyC'])
    vm.removeStep(firstMacro, stepId)
    expect(firstMacro.steps).toHaveLength(1)

    const sameUiKey = vm.uiKeyOf(firstMacro)
    expect(vm.uiKeyOf(firstMacro)).toBe(sameUiKey)

    const userDuplicate = {
      id: 'dup',
      name: 'Duplicate user macro',
      steps: [],
    }
    state.config.value.macros.push(userDuplicate)
    expect(vm.idError(firstMacro)).toBe('This ID is already used by another user macro.')
    expect(vm.idError(userDuplicate)).toBe('This ID is already used by another user macro.')
    expect(vm.uiKeyOf(userDuplicate)).not.toBe(vm.uiKeyOf(firstMacro))
    state.config.value.macros[state.config.value.macros.length - 1]!.id = 'dup2'
    await nextTick()
    expect(vm.idError(firstMacro)).toBeNull()
    expect(vm.idError(userDuplicate)).toBeNull()

    const sysMacro = systemMacroById('duplicateLine')!
    vm.cloneSystemMacro(sysMacro)
    const cloned = state.config.value.macros[0]!
    expect(cloned.id).toBe('duplicateLineCopy')
    expect(cloned.name).toBe('Duplicate line (copy)')
    expect(cloned.steps.map((step) => step.action)).toEqual(
      sysMacro.steps.map((step) => step.action),
    )

    const duplicate = {
      id: 'duplicateLine',
      name: 'Duplicate line',
      steps: [{ id: 'dup1', action: 'macro:Shift+Ctrl+KeyD' }],
    }
    state.config.value.macros.push(duplicate)
    expect(vm.idError(duplicate)).toContain('already taken')
    expect(vm.hasIdErrors).toBe(true)

    expect(vm.idError(duplicate)).toContain('already taken')

    firstMacro.steps[0]!.action = macroActionRef('dup2')
    expect(vm.stepError(firstMacro.steps[0], firstMacro.id)).toBeNull()

    state.config.value.macros.push({
      id: 'other',
      name: 'Other macro',
      steps: [{ id: 'other-step', action: macroActionRef('dup') }],
    })
    firstMacro.steps[0]!.action = macroActionRef('other')
    expect(vm.stepError(firstMacro.steps[0], firstMacro.id)).toContain('cycle')
    expect(vm.hasStepErrors).toBe(true)
    expect(vm.hasErrors).toBe(true)

    const usage = vm.usage
    expect(usage.dup).toEqual([
      'rule CapsLock (tap)',
      'nav.KeyH',
      'nav.MouseSide',
      'macro other (#1)',
    ])

    vm.removeMacro(vm.uiKeyOf(firstMacro))
    expect(state.config.value.macros.some((macro) => macro.id === 'dup')).toBe(false)
  })
})
