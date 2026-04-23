import { defineComponent, ref } from 'vue'

import { mockComponent, mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import LayoutsTab from '~/components/LayoutsTab.vue'
import { createDefaultConfig } from '~/types/config'

const {
  useSettingsScreenMock,
  requestApplyEntryMock,
  requestApplyEmptyMock,
  openSaveModalMock,
} = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
  requestApplyEntryMock: vi.fn(),
  requestApplyEmptyMock: vi.fn(),
  openSaveModalMock: vi.fn(),
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockComponent('~/components/features/settings/LayoutsLibraryCard.vue', () =>
  defineComponent({
    emits: ['saveCurrent', 'requestApplyEntry', 'requestApplyEmpty', 'requestDelete'],
    template: `
      <div data-test="library-card">
        <button data-test="save-current" @click="$emit('saveCurrent')">save</button>
        <button data-test="apply-entry" @click="$emit('requestApplyEntry', { id: 'user:test', name: 'Test', builtin: false })">apply</button>
        <button data-test="apply-empty" @click="$emit('requestApplyEmpty')">empty</button>
        <button data-test="request-delete" @click="$emit('requestDelete', { id: 'user:test', name: 'Test', builtin: false })">delete</button>
      </div>
    `,
  }),
)

describe('LayoutsTab', () => {
  beforeEach(() => {
    useSettingsScreenMock.mockReset()
    requestApplyEntryMock.mockReset()
    requestApplyEmptyMock.mockReset()
    openSaveModalMock.mockReset()

    useSettingsScreenMock.mockReturnValue({
      config: ref(createDefaultConfig()),
      currentLayoutId: ref('user:test'),
      isLayoutDirty: ref(true),
      library: {
        entries: ref([{ id: 'user:test', name: 'Test', builtin: false }]),
        error: ref(null),
        layoutsDir: ref('/tmp/layouts'),
      },
      applying: ref(''),
      applyError: ref(null),
      pendingApply: ref(null),
      requestApplyEntry: requestApplyEntryMock,
      requestApplyEmpty: requestApplyEmptyMock,
      cancelApply: vi.fn(),
      confirmApply: vi.fn(),
      saveModalOpen: ref(false),
      saveName: ref(''),
      saveBusy: ref(false),
      saveError: ref(null),
      openSaveModal: openSaveModalMock,
      performSave: vi.fn(),
      closeSaveModal: vi.fn(),
      deletePending: ref(null),
      deleteBusy: ref(false),
      confirmDelete: vi.fn(),
      clearDeletePending: vi.fn(),
    })
  })

  it('wires layout library events into the settings-screen actions', async () => {
    const wrapper = await mountSuspended(LayoutsTab)

    await wrapper.get('[data-test="save-current"]').trigger('click')
    await wrapper.get('[data-test="apply-entry"]').trigger('click')
    await wrapper.get('[data-test="apply-empty"]').trigger('click')
    await wrapper.get('[data-test="request-delete"]').trigger('click')

    expect(openSaveModalMock).toHaveBeenCalledTimes(1)
    expect(requestApplyEntryMock).toHaveBeenCalledWith({
      id: 'user:test',
      name: 'Test',
      builtin: false,
    })
    expect(requestApplyEmptyMock).toHaveBeenCalledTimes(1)

    const state = useSettingsScreenMock.mock.results[0]?.value
    expect(state.deletePending.value).toEqual({
      id: 'user:test',
      name: 'Test',
      builtin: false,
    })
  })
})
