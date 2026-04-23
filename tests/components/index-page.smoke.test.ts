import { defineComponent, ref } from 'vue'

import { mockComponent, mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import IndexPage from '~/pages/index.vue'

const { loadMock, useConfigMock } = vi.hoisted(() => ({
  loadMock: vi.fn<() => Promise<void>>(),
  useConfigMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)

mockComponent('~/components/LoadingScreen.vue', () =>
  defineComponent({
    props: {
      error: {
        type: String,
        default: null,
      },
    },
    template: '<div data-test="loading-screen">Loading screen</div>',
  }),
)

mockComponent('~/components/app/AppShell.vue', () =>
  defineComponent({
    template: '<div data-test="app-shell">App shell</div>',
  }),
)

describe('IndexPage', () => {
  beforeEach(() => {
    loadMock.mockReset()
    useConfigMock.mockReset()
  })

  it('shows the loading screen and loads config on mount', async () => {
    useConfigMock.mockReturnValue({
      loaded: ref(false),
      loadError: ref(null),
      needsWelcome: ref(false),
      load: loadMock,
    })

    const wrapper = await mountSuspended(IndexPage)

    expect(loadMock).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="loading-screen"]').exists()).toBe(true)
  })

  it('shows the app shell when config requires onboarding', async () => {
    useConfigMock.mockReturnValue({
      loaded: ref(true),
      loadError: ref(null),
      needsWelcome: ref(true),
      load: loadMock,
    })

    const wrapper = await mountSuspended(IndexPage)

    expect(wrapper.find('[data-test="app-shell"]').exists()).toBe(true)
  })
})
