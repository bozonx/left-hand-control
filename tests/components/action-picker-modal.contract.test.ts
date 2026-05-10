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
    excludedValues: {
      type: Array,
      default: () => [],
    },
    excludedCategoryIds: {
      type: Array,
      default: () => [],
    },
  },
  emits: ['update:modelValue', 'pick'],
  template: `
    <div data-testid="picker-body">
      {{ modelValue }}
      <span data-testid="allow-macros">{{ allowMacros }}</span>
      <span data-testid="excluded-values">{{ excludedValues.join(',') }}</span>
      <span data-testid="excluded-category-ids">{{ excludedCategoryIds.join(',') }}</span>
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

const defaultStubs = {
  Teleport: defineComponent({
    props: { to: { type: String, default: 'body' } },
    template: '<div><slot /></div>',
  }),
  UModal: UModalStub,
  ActionPickerBody: ActionPickerBodyStub,
  AppTooltip: defineComponent({
    template: '<div><slot /></div>',
  }),
}

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

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    expect(wrapper.find('[data-testid="action-picker-view"]').exists()).toBe(true)
  })

  it('applies and closes immediately when an item is picked from the list', async () => {
    const value = ref('')
    const open = ref(true)
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() { return { open, value } },
      template: '<ActionPickerModal v-model="value" v-model:open="open" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    const pickerItem = wrapper.find('[data-testid="picker-item"]')
    expect(pickerItem.exists()).toBe(true)
    await pickerItem.trigger('click')
    await flushPromises()

    expect(value.value).toBe('KeyA')
    expect(open.value).toBe(false)
    expect(wrapper.find('[data-testid="action-picker-view"]').exists()).toBe(false)
  })

  it('emits apply without cancel when an item is picked from the list', async () => {
    const value = ref('')
    const open = ref(true)
    const applied = ref<string[]>([])
    const cancelled = ref(0)
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() { return { applied, cancelled, open, value } },
      template: `
        <ActionPickerModal
          v-model="value"
          v-model:open="open"
          placeholder="pick action"
          @apply="applied.push($event)"
          @cancel="cancelled += 1"
        />
      `,
    })

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    await wrapper.get('[data-testid="picker-item"]').trigger('click')
    await flushPromises()

    expect(value.value).toBe('KeyA')
    expect(applied.value).toEqual(['KeyA'])
    expect(cancelled.value).toBe(0)
  })

  it('opens from its trigger button', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        return { value }
      },
      template: '<ActionPickerModal v-model="value" placeholder="pick action" />',
    })

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    await wrapper.get('[data-testid="action-picker-trigger"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="action-picker-view"]').exists()).toBe(true)
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

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    expect(wrapper.find('[data-testid="allow-macros"]').text()).toBe('false')
  })

  it('does not apply excluded values picked by the body', async () => {
    const value = ref('')
    const open = ref(true)
    const applied = ref<string[]>([])
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() { return { applied, open, value } },
      template: `
        <ActionPickerModal
          v-model="value"
          v-model:open="open"
          :excluded-values="['KeyA']"
          placeholder="pick action"
          @apply="applied.push($event)"
        />
      `,
    })

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    expect(wrapper.find('[data-testid="excluded-values"]').text()).toBe('KeyA')
    await wrapper.get('[data-testid="picker-item"]').trigger('click')
    await flushPromises()

    expect(value.value).toBe('')
    expect(applied.value).toEqual([])
    expect(open.value).toBe(true)
  })

  it('forwards excluded categories to the picker body', async () => {
    const Harness = defineComponent({
      components: { ActionPickerModal },
      setup() {
        const value = ref('')
        const open = ref(true)
        return { open, value }
      },
      template: '<ActionPickerModal v-model="value" v-model:open="open" :excluded-category-ids="[\'lettersSymbols\']" />',
    })

    const wrapper = await mountSuspended(Harness, { global: { stubs: defaultStubs } })

    expect(wrapper.find('[data-testid="excluded-category-ids"]').text()).toBe('lettersSymbols')
  })
})
