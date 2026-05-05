/* eslint-disable vue/one-component-per-file */
import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import ActionPickerModal from '~/components/ActionPickerModal.vue'

const { useMacrosMock } = vi.hoisted(() => ({
  useMacrosMock: vi.fn(),
}))

mockNuxtImport('useMacros', () => useMacrosMock)

const ActionPickerBodyStub = defineComponent({
  props: {
    modelValue: {
      type: String,
      default: '',
    },
    allowMacros: {
      type: Boolean,
      default: true,
    },
  },
  emits: ['update:modelValue', 'pick'],
  template: `
    <div data-testid="picker-body">
      {{ modelValue }}
      <span data-testid="allow-macros">{{ allowMacros }}</span>
      <button data-testid="picker-item" @click="$emit('pick', 'KeyA')">Pick A</button>
    </div>
  `,
})

const UModalStub = defineComponent({
  props: {
    open: {
      type: Boolean,
      default: false,
    },
  },
  template: '<div v-if="open"><slot name="header" /><slot name="body" /></div>',
})

describe('ActionPickerModal', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
    useMacrosMock.mockReset()
    useMacrosMock.mockReturnValue({
      displayAction: (value: string | null | undefined) => value ?? '',
      getActionInfo: (value: string | null | undefined) => ({
        label: value ?? '',
      }),
    })
  })

  it('renders its picker body when externally opened', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        const open = ref(true)
        return { open, value }
      },
      template: '<ActionPickerModal v-model="value" v-model:open="open" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          Teleport: defineComponent({
            props: { to: { type: String, default: 'body' } },
            template: '<div><slot /></div>',
          }),
          UModal: UModalStub,
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    expect(wrapper.find('[data-testid="action-picker-view"]').exists()).toBe(true)
  })

  it('applies and closes immediately when an item is picked from the list', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        const open = ref(true)
        return { open, value }
      },
      template: '<ActionPickerModal v-model="value" v-model:open="open" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          Teleport: defineComponent({
            props: { to: { type: String, default: 'body' } },
            template: '<div><slot /></div>',
          }),
          UModal: UModalStub,
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    const pickerItem = wrapper.find('[data-testid="picker-item"]')
    expect(pickerItem.exists()).toBe(true)
    await pickerItem.trigger('click')
    await flushPromises()

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect((wrapper.vm as any).open).toBe(false)
    expect(wrapper.find('[data-testid="action-picker-view"]').exists()).toBe(false)
  })

  it('forwards allowMacros to the picker body', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        const open = ref(true)
        return { open, value }
      },
      template: '<ActionPickerModal v-model="value" v-model:open="open" :allow-macros="false" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          Teleport: defineComponent({
            props: { to: { type: String, default: 'body' } },
            template: '<div><slot /></div>',
          }),
          UModal: UModalStub,
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    expect(wrapper.find('[data-testid="allow-macros"]').text()).toBe('false')
  })
})
