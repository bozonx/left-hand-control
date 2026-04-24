import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import IndexPage from '~/pages/index.vue'

const { navigateToMock } = vi.hoisted(() => ({
  navigateToMock: vi.fn(),
}))

mockNuxtImport('navigateTo', () => navigateToMock)

describe('IndexPage', () => {
  beforeEach(() => {
    navigateToMock.mockReset()
  })

  it('redirects to the layouts route', async () => {
    await mountSuspended(IndexPage)

    expect(navigateToMock).toHaveBeenCalledTimes(1)
    expect(navigateToMock).toHaveBeenCalledWith('/layouts', { replace: true })
  })
})
