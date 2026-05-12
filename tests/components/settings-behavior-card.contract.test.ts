/* eslint-disable vue/one-component-per-file */
import { defineComponent, nextTick, ref } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'

import BehaviorCard from '~/components/features/settings/BehaviorCard.vue'
import { createDefaultConfig } from '~/types/config'

const AppTooltipStub = defineComponent({
  name: 'AppTooltip',
  props: ['text', 'disabled'],
  setup(_props, { slots }) {
    return () => slots.default?.()
  },
})

describe('BehaviorCard', () => {
  it('updates default timeout numeric fields with the mouse wheel', async () => {
    const config = ref(createDefaultConfig())

    const Harness = defineComponent({
      components: { BehaviorCard },
      setup() {
        return { config }
      },
      template: '<BehaviorCard v-model:config="config" />',
    })

    const wrapper = await mountSuspended(Harness, {
      attachTo: document.body,
      global: {
        stubs: {
          AppTooltip: AppTooltipStub,
        },
      },
    })

    const inputs = wrapper.findAll('input[type="number"]')
    expect(inputs).toHaveLength(4)

    const holdInput = inputs[0]!
    const holdElement = holdInput.element as HTMLInputElement

    await nextTick()
    holdElement.focus()
    await holdInput.trigger('wheel', { deltaY: -100 })

    expect(config.value.settings.defaultHoldTimeoutMs).toBe(201)
  })
})
