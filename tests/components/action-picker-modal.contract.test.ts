import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import ActionPickerModal from '~/components/ActionPickerModal.vue'

const { useMacrosMock } = vi.hoisted(() => ({
  useMacrosMock: vi.fn(),
}))

mockNuxtImport('useMacros', () => useMacrosMock)

const UModalStub = defineComponent({
  props: {
    open: {
      type: Boolean,
      default: false,
    },
    title: {
      type: String,
      default: '',
    },
  },
  emits: ['update:open'],
  template: `
    <div data-testid="modal-root" :data-open="open ? 'yes' : 'no'">
      <slot name="body" />
      <slot name="footer" />
    </div>
  `,
})

const ActionPickerBodyStub = defineComponent({
  props: {
    modelValue: {
      type: String,
      default: '',
    },
  },
  emits: ['update:modelValue'],
  template: '<div data-testid="picker-body">{{ modelValue }}</div>',
})

describe('ActionPickerModal', () => {
  beforeEach(() => {
    useMacrosMock.mockReset()
    useMacrosMock.mockReturnValue({
      displayAction: (value: string | null | undefined) => value ?? '',
    })
  })

  it('opens itself when used without external v-model:open control', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        return { value }
      },
      template: '<ActionPickerModal v-model="value" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          UModal: UModalStub,
          ActionPickerBody: ActionPickerBodyStub,
        },
      },
    })

    expect(wrapper.get('[data-testid="modal-root"]').attributes('data-open')).toBe('no')

    await wrapper.get('button[type="button"]').trigger('click')

    expect(wrapper.get('[data-testid="modal-root"]').attributes('data-open')).toBe('yes')
  })
})
