import { defineComponent } from 'vue'
import { mockComponent, mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'

import IndexPage from '~/pages/index.vue'

mockComponent('~/components/shared/AppTooltip.vue', () =>
  defineComponent({
    template: '<span data-test="app-tooltip"><slot /></span>',
  }),
)

describe('IndexPage', () => {
  it('renders the layouts home content', async () => {
    const wrapper = await mountSuspended(IndexPage)

    expect(wrapper.text()).toContain('Quick start')
  })
})
