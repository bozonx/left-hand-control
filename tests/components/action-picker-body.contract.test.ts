/* eslint-disable vue/one-component-per-file */
import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it, vi } from 'vitest'

import ActionPickerBody from '~/components/ActionPickerBody.vue'

const { useCommandsMock, useMacrosMock } = vi.hoisted(() => ({
  useCommandsMock: vi.fn(),
  useMacrosMock: vi.fn(),
}))

mockNuxtImport('useCommands', () => useCommandsMock)
mockNuxtImport('useMacros', () => useMacrosMock)

function setupMocks() {
  useCommandsMock.mockReturnValue({ commands: ref([]) })
  useMacrosMock.mockReturnValue({ macros: ref([]) })
}

describe('ActionPickerBody text category', () => {
  it('keeps the empty text tab selected and stores typed text as a text action', async () => {
    setupMocks()
    const value = ref('')
    const Harness = defineComponent({
      components: { ActionPickerBody },
      setup() {
        return { value }
      },
      template: '<ActionPickerBody v-model="value" />',
    })

    const wrapper = await mountSuspended(Harness)

    await wrapper.get('[data-category-id="text"]').trigger('click')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('textarea').exists()).toBe(true)

    await wrapper.get('textarea').setValue('TODO: ')

    expect(value.value).toBe('text:TODO: ')
  })

  it('hides the text category when it is excluded', async () => {
    setupMocks()
    const value = ref('')
    const Harness = defineComponent({
      components: { ActionPickerBody },
      setup() {
        return { value }
      },
      template: '<ActionPickerBody v-model="value" :excluded-category-ids="[\'text\']" />',
    })

    const wrapper = await mountSuspended(Harness)

    expect(wrapper.find('[data-category-id="text"]').exists()).toBe(false)
  })
})
