/* eslint-disable vue/one-component-per-file */
import { ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import LayoutModals from '~/components/features/settings/LayoutModals.vue'
import { makeSettingsScreenMock } from '~/tests/factories/settings-screen'

const { useSettingsScreenMock } = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockNuxtImport('useI18n', () => () => ({
  t: (key: string) => key,
}))

describe('LayoutModals', () => {
  beforeEach(() => {
    useSettingsScreenMock.mockReset()
    document.body.innerHTML = ''
  })

  it('renders the delete confirmation modal when deletePending is set', async () => {
    const confirmDeleteMock = vi.fn()
    const clearDeletePendingMock = vi.fn()

    useSettingsScreenMock.mockReturnValue({
      ...makeSettingsScreenMock(),
      editMode: ref('name'),
      editDescription: ref(''),
      resetError: ref(null),
      deletePending: ref({ id: 'user:test', name: 'Test Layout' }),
      deleteBusy: ref(false),
      deleteError: ref(null),
      confirmDelete: confirmDeleteMock,
      clearDeletePending: clearDeletePendingMock,
    })

    await mountSuspended(LayoutModals, {
      attachTo: document.body,
    })

    expect(document.body.textContent).toContain('Test Layout')
    const deleteBtn = Array.from(document.body.querySelectorAll('button')).find(
      (b) => b.textContent?.includes('Delete'),
    )
    expect(deleteBtn).toBeTruthy()
  })

  it('renders the save modal when saveModalOpen is true', async () => {
    const performSaveMock = vi.fn()
    const closeSaveModalMock = vi.fn()

    useSettingsScreenMock.mockReturnValue({
      ...makeSettingsScreenMock(),
      editMode: ref('name'),
      editDescription: ref(''),
      resetError: ref(null),
      saveModalOpen: ref(true),
      saveName: ref('My Layout'),
      saveBusy: ref(false),
      saveError: ref(null),
      performSave: performSaveMock,
      closeSaveModal: closeSaveModalMock,
    })

    await mountSuspended(LayoutModals, {
      attachTo: document.body,
    })

    const input = document.body.querySelector('input')
    expect(input).not.toBeNull()
    expect(input!.value).toBe('My Layout')
  })
})
