import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
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
      <button type="button" data-testid="picker-item" @click="$emit('pick', 'KeyA')">pick</button>
    </div>
  `,
})

describe('ActionPickerModal', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
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
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    expect(document.querySelector('[data-testid="action-picker-view"]')).toBeNull()

    await wrapper.get('button[type="button"]').trigger('click')

    expect(document.querySelector('[data-testid="action-picker-view"]')).not.toBeNull()
  })

  it('applies and closes immediately when an item is picked from the list', async () => {
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
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    await wrapper.get('button[type="button"]').trigger('click')
    const pickerItem = document.querySelector('[data-testid="picker-item"]') as HTMLButtonElement
    pickerItem.click()
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect(document.querySelector('[data-testid="action-picker-view"]')).toBeNull()
  })

  it('forwards allowMacros to the picker body', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        return { value }
      },
      template: '<ActionPickerModal v-model="value" :allow-macros="false" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, {
      global: {
        stubs: {
          ActionPickerBody: ActionPickerBodyStub,
          AppTooltip: defineComponent({
            template: '<div><slot /></div>',
          }),
        },
      },
    })

    await wrapper.get('button[type="button"]').trigger('click')

    expect(document.querySelector('[data-testid="allow-macros"]')?.textContent).toBe('false')
  })
})
