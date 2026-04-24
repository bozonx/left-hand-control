import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'

import IndexPage from '~/pages/index.vue'

describe('IndexPage', () => {
  it('renders the layouts home content', async () => {
    const wrapper = await mountSuspended(IndexPage)

    expect(wrapper.text()).toContain('Using the library')
  })
})
