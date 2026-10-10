import { defineComponent } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'

import LayoutsLibraryCard from '~/components/features/settings/LayoutsLibraryCard.vue'

describe('LayoutsLibraryCard', () => {
  it('emits its public actions from the rendered controls', async () => {
    const entries = [
      { id: 'user:ivan-k', name: "Ivan K's" },
      { id: 'user:nav', name: 'Nav', description: 'Navigation' },
    ]

    const wrapper = await mountSuspended(LayoutsLibraryCard, {
      props: {
        entries,
        currentLayoutId: 'user:nav',
        currentLayoutDescription: 'Changed description',
        isLayoutDirty: true,
        applying: '',
        applyError: null,
        libraryError: null,
        layoutsDir: '/tmp/layouts',
        layoutMode: 'manual',
        autoIncludedIds: new Set<string>(),
      },
      global: {
        stubs: {
          AppTooltip: defineComponent({
            template: '<span><slot /></span>',
          }),
        },
      },
    })

    await wrapper.get('[data-testid="create-empty-layout"]').trigger('click')
    await wrapper.get('[data-testid="create-from-ivank"]').trigger('click')
    await wrapper.get('[data-testid="reset-unsaved"]').trigger('click')
    await wrapper.get('[data-testid="save-current"]').trigger('click')

    // First entry actions
    await wrapper.findAll('[data-testid="layout-rename"]')[0]?.trigger('click')
    await wrapper
      .findAll('[data-testid="layout-description"]')[0]
      ?.trigger('click')

    await wrapper.findAll('[data-testid="layout-open"]')[0]?.trigger('click')
    await wrapper.findAll('[data-testid="layout-delete"]')[0]?.trigger('click')

    expect(wrapper.emitted('createFromEmpty')).toHaveLength(1)
    expect(wrapper.emitted('createFromIvanK')).toHaveLength(1)
    expect(wrapper.emitted('saveCurrent')).toHaveLength(1)
    expect(wrapper.emitted('requestReset')).toHaveLength(1)
    expect(wrapper.emitted('requestApplyEntry')).toEqual([[entries[0]]])
    expect(wrapper.emitted('requestEdit')).toEqual([[entries[0], 'name']])
    expect(wrapper.emitted('requestDelete')).toEqual([[entries[0]]])
  })
})
