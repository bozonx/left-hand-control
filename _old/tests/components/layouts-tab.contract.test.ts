/* eslint-disable vue/one-component-per-file */
import { defineComponent } from 'vue'

import {
  mockComponent,
  mockNuxtImport,
  mountSuspended,
} from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import LayoutsTab from '~/components/LayoutsTab.vue'
import { makeSettingsScreenMock } from '~/tests/factories/settings-screen'

const {
  useSettingsScreenMock,
  requestApplyEntryMock,
  createFromEmptyMock,
  saveCurrentLayoutMock,
  updateDescriptionMock,
} = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
  requestApplyEntryMock: vi.fn(),
  createFromEmptyMock: vi.fn(),
  saveCurrentLayoutMock: vi.fn(),
  updateDescriptionMock: vi.fn(),
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockComponent('~/components/shared/AppTooltip.vue', () =>
  defineComponent({
    template: '<span data-test="app-tooltip"><slot /></span>',
  }),
)

mockComponent('~/components/features/settings/LayoutsLibraryCard.vue', () =>
  defineComponent({
    emits: [
      'saveCurrent',
      'requestApplyEntry',
      'createFromEmpty',
      'requestDelete',
      'updateDescription',
    ],
    template: `
      <div data-test="library-card">
        <button data-test="save-current" @click="$emit('saveCurrent')">save</button>
        <button data-test="apply-entry" @click="$emit('requestApplyEntry', { id: 'user:test', name: 'Test' })">apply</button>
        <button data-test="apply-empty" @click="$emit('createFromEmpty')">empty</button>
        <button data-test="request-delete" @click="$emit('requestDelete', { id: 'user:test', name: 'Test' })">delete</button>
        <button data-test="update-description" @click="$emit('updateDescription', { id: 'user:test', name: 'Test' }, 'new desc')">update</button>
      </div>
    `,
  }),
)

describe('LayoutsTab', () => {
  beforeEach(() => {
    useSettingsScreenMock.mockReset()
    requestApplyEntryMock.mockReset()
    createFromEmptyMock.mockReset()
    saveCurrentLayoutMock.mockReset()
    updateDescriptionMock.mockReset()

    useSettingsScreenMock.mockReturnValue(
      makeSettingsScreenMock({
        requestApplyEntry: requestApplyEntryMock,
        createFromEmpty: createFromEmptyMock,
        saveCurrentLayout: saveCurrentLayoutMock,
        updateDescription: updateDescriptionMock,
      }),
    )
  })

  it('wires layout library events into the settings-screen actions', async () => {
    const wrapper = await mountSuspended(LayoutsTab)

    await wrapper.get('[data-test="save-current"]').trigger('click')
    await wrapper.get('[data-test="apply-entry"]').trigger('click')
    await wrapper.get('[data-test="apply-empty"]').trigger('click')
    await wrapper.get('[data-test="request-delete"]').trigger('click')
    await wrapper.get('[data-test="update-description"]').trigger('click')

    expect(saveCurrentLayoutMock).toHaveBeenCalledTimes(1)
    expect(requestApplyEntryMock).toHaveBeenCalledWith({
      id: 'user:test',
      name: 'Test',
    })
    expect(createFromEmptyMock).toHaveBeenCalledTimes(1)
    expect(updateDescriptionMock).toHaveBeenCalledWith(
      {
        id: 'user:test',
        name: 'Test',
      },
      'new desc',
    )

    const state = useSettingsScreenMock.mock.results[0]?.value
    expect(state.deletePending.value).toEqual({
      id: 'user:test',
      name: 'Test',
    })
  })
})
