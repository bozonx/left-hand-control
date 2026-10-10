/* eslint-disable vue/one-component-per-file */
import { defineComponent, nextTick, ref } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import ActionPickerValueField from '~/components/features/action-picker/ActionPickerValueField.vue'

function makeHarness() {
  const value = ref('KeyA')
  const selectionVersion = ref(0)
  const component = defineComponent({
    components: { ActionPickerValueField },
    setup() {
      return { selectionVersion, value }
    },
    template: `
      <ActionPickerValueField
        v-model="value"
        active-category="special"
        :filtered-items="[{ label: 'Key B', value: 'KeyB', hint: 'KeyB' }]"
        :selection-version="selectionVersion"
      />
    `,
  })
  return { selectionVersion, value, component }
}

describe('ActionPickerValueField key capture', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
  })

  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('captures a modifier chord from the overlay', async () => {
    const { value, component } = makeHarness()
    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    await wrapper.get('[data-testid="capture-button"]').trigger('click')

    expect(
      document.querySelector('[data-testid="key-capture-overlay"]'),
    ).not.toBeNull()

    document.dispatchEvent(
      new KeyboardEvent('keydown', { code: 'ControlLeft', key: 'Control' }),
    )
    document.dispatchEvent(
      new KeyboardEvent('keydown', { code: 'KeyK', key: 'k' }),
    )
    await wrapper.vm.$nextTick()

    expect(value.value).toBe('KeyA')
    expect(
      document.querySelector('[data-testid="key-capture-overlay"]')
        ?.textContent,
    ).toContain('Ctrl+KeyK')

    document.dispatchEvent(
      new KeyboardEvent('keyup', { code: 'KeyK', key: 'k' }),
    )
    await wrapper.vm.$nextTick()

    expect(value.value).toBe('KeyA')
    expect(
      document.querySelector('[data-testid="key-capture-overlay"]'),
    ).not.toBeNull()

    document.dispatchEvent(
      new KeyboardEvent('keyup', { code: 'ControlLeft', key: 'Control' }),
    )
    await wrapper.vm.$nextTick()

    expect(value.value).toBe('Ctrl+KeyK')
    expect(
      document.querySelector('[data-testid="key-capture-overlay"]'),
    ).toBeNull()
  })

  it('cancels capture without changing the draft', async () => {
    const { value, component } = makeHarness()
    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    await wrapper.get('[data-testid="capture-button"]').trigger('click')
    document.dispatchEvent(
      new KeyboardEvent('keydown', { code: 'ControlLeft', key: 'Control' }),
    )
    await wrapper.vm.$nextTick()

    const cancel = document.querySelector(
      '[data-testid="key-capture-overlay"] button',
    ) as HTMLButtonElement
    expect(cancel).not.toBeNull()
    cancel.click()
    await wrapper.vm.$nextTick()

    expect(value.value).toBe('KeyA')
    expect(
      document.querySelector('[data-testid="key-capture-overlay"]'),
    ).toBeNull()
  })

  it('captures a single modifier by its physical key code', async () => {
    const { value, component } = makeHarness()
    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    await wrapper.get('[data-testid="capture-button"]').trigger('click')
    document.dispatchEvent(
      new KeyboardEvent('keydown', { code: 'ShiftLeft', key: 'Shift' }),
    )
    document.dispatchEvent(
      new KeyboardEvent('keyup', { code: 'ShiftLeft', key: 'Shift' }),
    )
    await wrapper.vm.$nextTick()

    expect(value.value).toBe('ShiftLeft')
    expect(
      document.querySelector('[data-testid="key-capture-overlay"]'),
    ).toBeNull()
  })

  it('hides suggestions after a value is selected outside the input suggestions', async () => {
    const { selectionVersion, value, component } = makeHarness()
    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    const input = wrapper.get('input')
    await input.setValue('Key')
    await wrapper.vm.$nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(true)

    value.value = 'KeyB'
    selectionVersion.value += 1
    await wrapper.vm.$nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(false)
  })

  it('navigates suggestions with keyboard', async () => {
    const value = ref('')
    const component = defineComponent({
      components: { ActionPickerValueField },
      setup() {
        return { value }
      },
      template: `
        <ActionPickerValueField
          v-model="value"
          active-category="special"
          :filtered-items="[
            { label: 'Key A', value: 'KeyA', hint: 'KeyA' },
            { label: 'Key B', value: 'KeyB', hint: 'KeyB' },
            { label: 'Key C', value: 'KeyC', hint: 'KeyC' },
          ]"
        />
      `,
    })

    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    const input = wrapper.get('input')
    await input.setValue('Key')
    await nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(true)
    expect(wrapper.findAll('[role="option"]').length).toBe(3)

    await input.trigger('keydown', { key: 'ArrowDown' })
    await nextTick()

    let option = wrapper.find('[role="option"]')
    expect(option.exists()).toBe(true)
    expect(option.classes().join(' ')).toContain('ring-2')

    await option.trigger('keydown', { key: 'ArrowDown' })
    await nextTick()

    option = wrapper.findAll('[role="option"]')[1]!
    expect(option.classes().join(' ')).toContain('ring-2')

    await option.trigger('keydown', { key: 'ArrowUp' })
    await nextTick()

    option = wrapper.find('[role="option"]')
    expect(option.classes().join(' ')).toContain('ring-2')
  })

  it('selects a suggestion by click', async () => {
    const value = ref('')
    const component = defineComponent({
      components: { ActionPickerValueField },
      setup() {
        return { value }
      },
      template: `
        <ActionPickerValueField
          v-model="value"
          active-category="special"
          :filtered-items="[
            { label: 'Key A', value: 'KeyA', hint: 'KeyA' },
          ]"
        />
      `,
    })

    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    const input = wrapper.get('input')
    await input.setValue('Key')
    await nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(true)

    await wrapper.find('[role="option"]').trigger('click')
    await nextTick()

    expect(value.value).toBe('KeyA')
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false)
  })

  it('closes suggestions with Escape', async () => {
    const value = ref('')
    const component = defineComponent({
      components: { ActionPickerValueField },
      setup() {
        return { value }
      },
      template: `
        <ActionPickerValueField
          v-model="value"
          active-category="special"
          :filtered-items="[{ label: 'Key A', value: 'KeyA', hint: 'KeyA' }]"
        />
      `,
    })

    const wrapper = await mountSuspended(component, {
      attachTo: document.body,
    })

    const input = wrapper.get('input')
    await input.setValue('Key')
    await nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(true)

    await input.trigger('keydown', { key: 'Escape' })
    await nextTick()

    expect(wrapper.find('[role="listbox"]').exists()).toBe(false)
  })
})
