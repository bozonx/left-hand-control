/* eslint-disable vue/one-component-per-file */
import { defineComponent, nextTick, ref } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'

import SettingTimeoutField from '~/components/SettingTimeoutField.vue'
import NumericInput from '~/components/shared/NumericInput.vue'

describe('numeric timeout fields', () => {
  it('opens the editor when the timeout row is clicked', async () => {
    const Harness = defineComponent({
      components: { SettingTimeoutField },
      setup() {
        const value = ref<number | undefined>(undefined)
        return { value }
      },
      template: `
        <SettingTimeoutField
          v-model="value"
          label="Hold ms"
          :default-value="200"
          suffix="ms"
        />
      `,
    })

    const wrapper = await mountSuspended(Harness, {
      attachTo: document.body,
      global: {
        components: {
          NumericInput,
        },
      },
    })

    expect(wrapper.find('input[type="number"]').exists()).toBe(false)

    await wrapper.get('.group').trigger('click')

    const input = wrapper.get('input[type="number"]').element as HTMLInputElement
    expect(input.value).toBe('200')
    expect(document.activeElement).toBe(input)
  })

  it('updates a focused numeric input with the mouse wheel', async () => {
    const Harness = defineComponent({
      components: { NumericInput },
      setup() {
        const value = ref<string | number>(10)
        return { value }
      },
      template: '<NumericInput v-model="value" :min="0" :max="20" />',
    })

    const wrapper = await mountSuspended(Harness, {
      attachTo: document.body,
    })
    const input = wrapper.get('input[type="number"]')
    const inputElement = input.element as HTMLInputElement

    await nextTick()
    inputElement.focus()
    await input.trigger('wheel', { deltaY: -100 })
    expect((wrapper.vm as { value: string | number }).value).toBe(11)

    await input.trigger('wheel', { deltaY: 100, shiftKey: true })
    expect((wrapper.vm as { value: string | number }).value).toBe(1)
  })
})
