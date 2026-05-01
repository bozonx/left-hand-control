import { defineComponent, ref } from 'vue'

import { mockComponent, mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import SettingsTab from '~/components/SettingsTab.vue'
import { createDefaultConfig } from '~/types/config'

const {
  useSettingsScreenMock,
  requestApplyEntryMock,
  requestApplyEmptyMock,
  openSaveModalMock,
  confirmApplyMock,
  cancelApplyMock,
  performSaveMock,
  closeSaveModalMock,
  confirmDeleteMock,
  clearDeletePendingMock,
  toggleMapperMock,
} = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
  requestApplyEntryMock: vi.fn(),
  requestApplyEmptyMock: vi.fn(),
  openSaveModalMock: vi.fn(),
  confirmApplyMock: vi.fn(),
  cancelApplyMock: vi.fn(),
  performSaveMock: vi.fn(),
  closeSaveModalMock: vi.fn(),
  confirmDeleteMock: vi.fn(),
  clearDeletePendingMock: vi.fn(),
  toggleMapperMock: vi.fn(),
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockComponent('~/components/features/settings/MapperCard.vue', () =>
  defineComponent({
    emits: ['toggle'],
    template: '<button data-test="mapper-card" @click="$emit(\'toggle\')">mapper</button>',
  }),
)

mockComponent('~/components/features/settings/GeneralCard.vue', () =>
  defineComponent({
    template: '<div data-test="general-card">general</div>',
  }),
)

mockComponent('~/components/features/settings/BehaviorCard.vue', () =>
  defineComponent({
    template: '<div data-test="behavior-card">behavior</div>',
  }),
)

mockComponent('~/components/features/settings/GameModeCard.vue', () =>
  defineComponent({
    template: '<div data-test="game-mode-card">game mode</div>',
  }),
)

mockComponent('~/components/features/settings/ConfigPathCard.vue', () =>
  defineComponent({
    template: '<div data-test="config-card">config</div>',
  }),
)

describe('SettingsTab', () => {
  beforeEach(() => {
    useSettingsScreenMock.mockReset()
    requestApplyEntryMock.mockReset()
    requestApplyEmptyMock.mockReset()
    openSaveModalMock.mockReset()
    confirmApplyMock.mockReset()
    cancelApplyMock.mockReset()
    performSaveMock.mockReset()
    closeSaveModalMock.mockReset()
    confirmDeleteMock.mockReset()
    clearDeletePendingMock.mockReset()
    toggleMapperMock.mockReset()

    useSettingsScreenMock.mockReturnValue({
      config: ref(createDefaultConfig()),
      settingsDir: ref('/tmp/settings'),
      currentLayoutId: ref('user:test'),
      isLayoutDirty: ref(true),
      library: {
        entries: ref([{ id: 'user:test', name: 'Test', builtin: false }]),
        error: ref(null),
        layoutsDir: ref('/tmp/layouts'),
      },
      mapper: {
        status: ref({ running: false, device_path: null, last_error: null }),
        devices: ref([]),
        busy: ref(false),
        error: ref(null),
      },
      globalBanner: ref(null),
      globalIssues: ref([]),
      mapperIssues: ref([]),
      theme: {
        preference: ref('system'),
        resolved: ref('light'),
      },
      appLocale: {
        preference: ref('auto'),
        available: ['en-US', 'ru-RU'],
      },
      appearanceItems: ref([]),
      localeItems: ref([]),
      applying: ref(''),
      applyError: ref(null),
      pendingApply: ref(null),
      requestApplyEntry: requestApplyEntryMock,
      requestApplyEmpty: requestApplyEmptyMock,
      cancelApply: cancelApplyMock,
      confirmApply: confirmApplyMock,
      saveModalOpen: ref(false),
      saveName: ref(''),
      saveBusy: ref(false),
      saveError: ref(null),
      openSaveModal: openSaveModalMock,
      performSave: performSaveMock,
      closeSaveModal: closeSaveModalMock,
      deletePending: ref(null),
      deleteBusy: ref(false),
      confirmDelete: confirmDeleteMock,
      clearDeletePending: clearDeletePendingMock,
      deviceOptions: ref([]),
      selectedDevice: ref(''),
      toggleMapper: toggleMapperMock,
    })
  })

  it('wires settings cards into the settings-screen actions', async () => {
    const wrapper = await mountSuspended(SettingsTab)

    await wrapper.get('[data-test="mapper-card"]').trigger('click')

    expect(toggleMapperMock).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="mapper-card"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="general-card"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="behavior-card"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="game-mode-card"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="config-card"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="library-card"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="global-banner"]').exists()).toBe(false)
  })
})
