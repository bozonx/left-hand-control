/* eslint-disable vue/one-component-per-file */
import { defineComponent, nextTick, ref } from 'vue'

import { mockComponent, mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import SettingsTab from '~/components/SettingsTab.vue'
import { createDefaultConfig } from '~/types/config'

const { useSettingsScreenMock, stateRef } = vi.hoisted(() => ({
  useSettingsScreenMock: vi.fn(),
  stateRef: { value: null as any },
}))

mockNuxtImport('useSettingsScreen', () => useSettingsScreenMock)

mockComponent('~/components/FieldLabel.vue', () =>
  defineComponent({
    props: {
      label: {
        type: String,
        required: true,
      },
    },
    template: '<span>{{ label }}</span>',
  }),
)

mockComponent('~/components/features/settings/MapperCard.vue', () =>
  defineComponent({
    template: '<div data-test="mapper-card" />',
  }),
)

mockComponent('~/components/features/settings/GeneralCard.vue', () =>
  defineComponent({
    template: '<div data-test="general-card" />',
  }),
)

mockComponent('~/components/features/settings/GameModeCard.vue', () =>
  defineComponent({
    template: '<div data-test="game-mode-card" />',
  }),
)

mockComponent('~/components/features/settings/ConfigPathCard.vue', () =>
  defineComponent({
    template: '<div data-test="config-card" />',
  }),
)

mockComponent('~/components/features/settings/SystemCard.vue', () =>
  defineComponent({
    template: '<div data-test="system-card" />',
  }),
)

describe('Settings behavior card numeric fields', () => {
  beforeEach(() => {
    const config = ref(createDefaultConfig())

    stateRef.value = {
      config,
      settingsDir: ref('/tmp/settings'),
      library: {
        layoutsDir: ref('/tmp/layouts'),
      },
      mapper: {
        status: ref({
          running: false,
          device_path: null,
          mouse_device_path: null,
          last_error: null,
        }),
        busy: ref(false),
        error: ref(null),
      },
      mapperIssues: ref([]),
      platform: {
        info: ref(null),
      },
      theme: {
        preference: ref('system'),
        resolved: ref('light'),
      },
      appLocale: {
        preference: ref('auto'),
      },
      appearanceItems: ref([]),
      localeItems: ref([]),
      deviceOptions: ref([]),
      selectedDevice: ref(''),
      mouseOptions: ref([]),
      selectedMouse: ref(''),
      toggleMapper: vi.fn(),
    }

    useSettingsScreenMock.mockReset()
    useSettingsScreenMock.mockReturnValue(stateRef.value)
  })

  it('updates default timeout numeric fields with the mouse wheel on the settings page', async () => {
    const wrapper = await mountSuspended(SettingsTab, {
      attachTo: document.body,
    })

    const input = wrapper.get('input[type="number"]')
    const inputElement = input.element as HTMLInputElement

    await nextTick()
    inputElement.focus()
    await input.trigger('wheel', { deltaY: -100 })

    expect(stateRef.value.config.value.settings.defaultHoldTimeoutMs).toBe(201)
  })
})
