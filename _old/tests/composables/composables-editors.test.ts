/* eslint-disable vue/one-component-per-file */
import { defineComponent, ref, nextTick } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import { useKeymapEditor } from '~/composables/useKeymapEditor'
import { useLayers } from '~/composables/useLayers'
import { useRulesEditor } from '~/composables/useRulesEditor'

const { useConfigMock } = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)

function makeConfigState() {
  return {
    config: ref(createDefaultConfig()),
  }
}

describe('editor composables', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
  })

  it('useLayers creates, renames and deletes layers while keeping config consistent', async () => {
    const state = makeConfigState()
    state.config.value.layers.push({ id: 'nav', name: 'Navigation' })
    state.config.value.layerKeymaps.nav = { keys: {} }
    state.config.value.rules.push({
      id: 'rule-1',
      key: 'CapsLock',
      layerId: 'nav',
      tapAction: '',
      holdAction: '',
      doubleTapAction: '',
    })
    useConfigMock.mockReturnValue(state)

    const Harness = defineComponent({
      setup() {
        return useLayers()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    expect(vm.ensureLayerKeymap('symbols')).toEqual({ keys: {} })

    const createdId = vm.createLayer({
      name: '  Navigation  ',
      description: '  movement  ',
    })

    expect(createdId).toBeTruthy()
    expect(state.config.value.layers.at(-1)).toMatchObject({
      id: createdId,
      name: 'Navigation',
      description: 'movement',
    })
    expect(state.config.value.layerKeymaps[createdId!]).toEqual({
      keys: {},
    })

    expect(
      vm.renameLayer(createdId!, {
        name: '  Nav  ',
        description: '  arrows  ',
      }),
    ).toBe(true)
    expect(state.config.value.layers.find((layer) => layer.id === createdId)).toMatchObject({
      name: 'Nav',
      description: 'arrows',
    })

    expect(vm.deleteLayer('nav')).toBe(true)
    expect(state.config.value.layers.some((layer) => layer.id === 'nav')).toBe(false)
    expect(state.config.value.layerKeymaps.nav).toBeUndefined()
    expect(state.config.value.rules[0]?.layerId).toBe('')
  })

  it('useRulesEditor manages rules and can create a layer for a pending rule', async () => {
    const state = makeConfigState()
    useConfigMock.mockReturnValue(state)

    const Harness = defineComponent({
      setup() {
        return useRulesEditor()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    expect(vm.layerOptions).toEqual([{ label: '— none —', value: '__none__' }])

    vm.addRule()
    const firstRuleId = state.config.value.rules[0]?.id

    expect(state.config.value.rules[0]).toMatchObject({
      key: '',
      layerId: '',
      tapAction: '',
      holdAction: '',
      doubleTapAction: '',
      holdTimeoutMs: undefined,
      doubleTapTimeoutMs: undefined,
    })
    expect(vm.newestRuleId).toBe(firstRuleId)

    vm.addRule()
    const secondRuleId = state.config.value.rules[0]?.id
    expect(secondRuleId).toBeTruthy()
    expect(secondRuleId).not.toBe(firstRuleId)
    expect(state.config.value.rules[1]?.id).toBe(firstRuleId)

    vm.moveRule(firstRuleId!, 'up')
    expect(state.config.value.rules[0]?.id).toBe(firstRuleId)
    expect(state.config.value.rules[1]?.id).toBe(secondRuleId)

    vm.openNewLayer(firstRuleId!)
    vm.newLayerName = 'Symbols'
    vm.confirmNewLayer()

    const createdLayer = state.config.value.layers.find((layer) => layer.name === 'Symbols')
    expect(createdLayer).toBeTruthy()
    expect(state.config.value.rules[0]?.layerId).toBe(createdLayer?.id)
    expect(vm.newLayerOpen).toBe(false)
    expect(vm.layerOptions).toContainEqual({
      label: 'Symbols',
      value: createdLayer?.id,
    })

    vm.removeRule(firstRuleId!)
    vm.removeRule(secondRuleId!)
    expect(state.config.value.rules).toEqual([])
  })

  it('useKeymapEditor edits keys, extras and selected-layer lifecycle', async () => {
    const state = makeConfigState()
    state.config.value.layers.push({ id: 'nav', name: 'Navigation' })
    state.config.value.layerKeymaps.nav = {
      keys: { KeyH: 'ArrowLeft' },

    }
    useConfigMock.mockReturnValue(state)

    const Harness = defineComponent({
      setup() {
        return useKeymapEditor()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    vm.selectedLayerId = 'nav'
    expect(vm.currentLayer?.name).toBe('Navigation')
    expect(vm.layerItems).toContainEqual({ label: 'Navigation', value: 'nav' })

    vm.openEdit('KeyH', 'H')
    expect(vm.editOpen).toBe(true)
    expect(vm.editAction).toBe('ArrowLeft')

    vm.saveEdit('ArrowUp')
    expect(state.config.value.layerKeymaps.nav?.keys.KeyH).toBe('ArrowUp')

    vm.openEdit('KeyJ', 'J')
    vm.saveEdit('ArrowDown')
    expect(state.config.value.layerKeymaps.nav?.keys.KeyJ).toBe('ArrowDown')
    vm.clearEdit()
    expect(state.config.value.layerKeymaps.nav?.keys.KeyJ).toBeUndefined()

    vm.addExtra('F4')
    expect(state.config.value.layerKeymaps.nav?.keys.F4).toBe('F4')
    expect(vm.extraKeys).toContainEqual({ key: 'F4', action: 'F4' })
    vm.updateExtra('F4', 'action', null)
    expect(state.config.value.layerKeymaps.nav?.keys.F4).toBeNull()
    vm.updateExtra('F4', 'key', 'MouseSide')
    expect(state.config.value.layerKeymaps.nav?.keys.F4).toBeUndefined()
    expect(state.config.value.layerKeymaps.nav?.keys.MouseSide).toBeNull()
    vm.addExtra('MouseSide')
    expect(state.config.value.layerKeymaps.nav?.keys.MouseSide).toBeNull()
    vm.addExtra('KeyQ')
    expect(state.config.value.layerKeymaps.nav?.keys.KeyQ).toBeUndefined()
    vm.addExtra('MouseExtra')
    vm.updateExtra('MouseSide', 'key', 'MouseExtra')
    expect(state.config.value.layerKeymaps.nav?.keys.MouseSide).toBeNull()
    vm.removeExtra('MouseSide')
    expect(state.config.value.layerKeymaps.nav?.keys.MouseSide).toBeUndefined()
    vm.updateExtra('MouseExtra', 'action', '')
    expect(state.config.value.layerKeymaps.nav?.keys.MouseExtra).toBeUndefined()

    vm.addExtra('F4')
    vm.clearSelectedLayer()
    expect(state.config.value.layerKeymaps.nav?.keys).toEqual({ F4: 'F4' })
    vm.openEdit('KeyH', 'H')
    vm.saveEdit('ArrowLeft')
    vm.clearSelectedExtras()
    expect(state.config.value.layerKeymaps.nav?.keys).toEqual({ KeyH: 'ArrowLeft' })

    vm.openRename()
    vm.renameDraftName = 'Nav'
    vm.confirmRename()
    expect(state.config.value.layers.find((layer) => layer.id === 'nav')).toMatchObject({
      name: 'Nav',
    })

    vm.requestDeleteSelectedLayer()
    expect(vm.deleteConfirmOpen).toBe(true)
    vm.deleteSelectedLayer()
    await nextTick()

    expect(vm.selectedLayerId).toBe('')
    expect(state.config.value.layers.some((layer) => layer.id === 'nav')).toBe(false)
    expect(vm.deleteConfirmOpen).toBe(false)

    vm.openNewLayer()
    vm.newLayerName = 'Symbols'
    vm.confirmNewLayer()

    const createdLayer = state.config.value.layers.find((layer) => layer.name === 'Symbols')
    expect(createdLayer).toBeTruthy()
    expect(vm.selectedLayerId).toBe(createdLayer?.id)
    expect(vm.newLayerOpen).toBe(false)
  })
})
