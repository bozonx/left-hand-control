import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import RuleActionField from '~/components/features/rules/RuleActionField.vue'

const { useMacrosMock } = vi.hoisted(() => ({
  useMacrosMock: vi.fn(),
}))

mockNuxtImport('useMacros', () => useMacrosMock)

const ResettableSelectMenuStub = defineComponent({
  props: {
    modelValue: {
      type: String,
      default: '',
    },
    items: {
      type: Array,
      default: () => [],
    },
  },
  emits: ['update:modelValue'],
  template: `
    <div data-testid="mode-select">
      <div data-testid="mode-value">{{ modelValue }}</div>
      <button type="button" data-testid="pick-native" @click="$emit('update:modelValue', 'native')">native</button>
      <button type="button" data-testid="pick-none" @click="$emit('update:modelValue', 'none')">none</button>
      <button type="button" data-testid="pick-action" @click="$emit('update:modelValue', 'action')">action</button>
    </div>
  `,
})

const ActionPickerModalStub = defineComponent({
  props: {
    modelValue: {
      type: String,
      default: '',
    },
    open: {
      type: Boolean,
      default: false,
    },
  },
  emits: ['update:modelValue', 'update:open', 'apply', 'cancel'],
  template: `
    <div data-testid="picker-modal" :data-open="open ? 'yes' : 'no'" :data-value="modelValue">
      <button type="button" data-testid="modal-apply" @click="$emit('apply', 'KeyA')">apply</button>
      <button type="button" data-testid="modal-cancel" @click="$emit('cancel')">cancel</button>
    </div>
  `,
})

const FieldResetButtonStub = defineComponent({
  props: {
    label: {
      type: String,
      default: '',
    },
  },
  emits: ['click'],
  template: '<button type="button" data-testid="field-reset" :aria-label="label" @click="$emit(\'click\')">reset</button>',
})

describe('RuleActionField', () => {
  beforeEach(() => {
    useMacrosMock.mockReset()
    useMacrosMock.mockReturnValue({
      displayAction: (value: string | null | undefined) => value ?? '',
      getActionInfo: (value: string | null | undefined) => ({
        label: value ?? '',
        icon: '',
      }),
    })
  })

  it('restores the previous value when action picking is cancelled', async () => {
    const Harness = defineComponent({
      components: { RuleActionField },
      setup() {
        const value = ref<string | null>('')
        return { value }
      },
      template: '<RuleActionField v-model="value" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          FieldResetButton: FieldResetButtonStub,
          ResettableSelectMenu: ResettableSelectMenuStub,
          ActionPickerModal: ActionPickerModalStub,
        },
      },
    })

    await wrapper.get('[data-testid="pick-action"]').trigger('click')
    expect(wrapper.get('[data-testid="picker-modal"]').attributes('data-open')).toBe('yes')

    await wrapper.get('[data-testid="modal-cancel"]').trigger('click')

    expect((wrapper.vm as any).value).toBe('')
    expect(wrapper.find('[data-testid="mode-select"]').exists()).toBe(true)
  })

  it('stores the chosen action and can reset back to native', async () => {
    const Harness = defineComponent({
      components: { RuleActionField },
      setup() {
        const value = ref<string | null>('')
        return { value }
      },
      template: '<RuleActionField v-model="value" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          FieldResetButton: FieldResetButtonStub,
          ResettableSelectMenu: ResettableSelectMenuStub,
          ActionPickerModal: ActionPickerModalStub,
        },
      },
    })

    await wrapper.get('[data-testid="pick-action"]').trigger('click')
    await wrapper.get('[data-testid="modal-apply"]').trigger('click')

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect(wrapper.find('[data-testid="mode-select"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('KeyA')

    const resetButton = wrapper.findAll('button').find((button) =>
      button.attributes('aria-label')?.includes('Reset'),
    )
    expect(resetButton).toBeTruthy()

    await resetButton!.trigger('click')

    expect((wrapper.vm as any).value).toBe('')
    expect(wrapper.find('[data-testid="mode-select"]').exists()).toBe(true)
  })
})
