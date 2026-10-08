/* eslint-disable vue/one-component-per-file */
import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it, vi } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import ActionPickerBody from '~/components/ActionPickerBody.vue'

const { useCommandsMock, useMacrosMock, useConfigMock } = vi.hoisted(() => ({
  useCommandsMock: vi.fn(),
  useMacrosMock: vi.fn(),
  useConfigMock: vi.fn(),
}))

mockNuxtImport('useCommands', () => useCommandsMock)
mockNuxtImport('useMacros', () => useMacrosMock)
mockNuxtImport('useConfig', () => useConfigMock)

function setupMocks() {
  useConfigMock.mockReturnValue({ config: ref(createDefaultConfig()) })
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


describe('ActionPickerBody commands', () => {
  it('hides disabled commands and creates a local draft when enabled', async () => {
    setupMocks()
    const config = useConfigMock().config
    const wrapper = await mountSuspended(ActionPickerBody)
    expect(wrapper.find('[data-category-id="commands"]').exists()).toBe(false)
    config.value.settings.commandsEnabled = true
    await wrapper.vm.$nextTick()
    await wrapper.get('[data-category-id="commands"]').trigger('click')
    await wrapper.get('textarea').setValue('printf hello')
    expect(wrapper.emitted('update:command')?.at(-1)?.[0]).toMatchObject({ linux: 'printf hello' })
    expect(config.value.commands).toEqual([])
  })
})
