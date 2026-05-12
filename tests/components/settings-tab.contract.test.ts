/* eslint-disable vue/one-component-per-file */
import { defineComponent } from 'vue'

import {
  mockComponent,
  mockNuxtImport,
  mountSuspended,
} from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import SettingsTab from '~/components/SettingsTab.vue'
import { makeSettingsScreenMock } from '~/tests/factories/settings-screen'

const { useSettingsScreenMock, toggleMapperMock } = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
  toggleMapperMock: vi.fn(),
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockComponent('~/components/features/settings/MapperCard.vue', () =>
  defineComponent({
    emits: ['toggle'],
    template:
      '<button data-test="mapper-card" @click="$emit(\'toggle\')">mapper</button>',
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

mockComponent('~/components/features/settings/SystemCard.vue', () =>
  defineComponent({
    template: '<div data-test="system-card">system</div>',
  }),
)

describe('SettingsTab', () => {
  beforeEach(() => {
    useSettingsScreenMock.mockReset()
    toggleMapperMock.mockReset()
    useSettingsScreenMock.mockReturnValue(
      makeSettingsScreenMock({ toggleMapper: toggleMapperMock }),
    )
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
