import { defineComponent, ref } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it } from 'vitest'

import ActionPickerValueField from '~/components/features/action-picker/ActionPickerValueField.vue'

const Harness = defineComponent({
  components: { ActionPickerValueField },
  setup() {
    const value = ref('KeyA')
    return { value }
  },
  template: `
    <ActionPickerValueField
      v-model="value"
      active-category="special"
      :filtered-items="[]"
    />
  `,
})

describe('ActionPickerValueField key capture', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
  })

  it('captures a modifier chord from the overlay', async () => {
    const wrapper = await mountSuspended(Harness)

    await wrapper.get('button[aria-label="Listen for key press"]').trigger('click')

    expect(document.querySelector('[data-testid="key-capture-overlay"]')).not.toBeNull()

    document.dispatchEvent(new KeyboardEvent('keydown', { code: 'ControlLeft', key: 'Control' }))
    document.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyK', key: 'k' }))
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect(document.querySelector('[data-testid="key-capture-overlay"]')?.textContent).toContain('Ctrl+KeyK')

    document.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyK', key: 'k' }))
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect(document.querySelector('[data-testid="key-capture-overlay"]')).not.toBeNull()

    document.dispatchEvent(new KeyboardEvent('keyup', { code: 'ControlLeft', key: 'Control' }))
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('Ctrl+KeyK')
    expect(document.querySelector('[data-testid="key-capture-overlay"]')).toBeNull()
  })

  it('cancels capture without changing the draft', async () => {
    const wrapper = await mountSuspended(Harness)

    await wrapper.get('button[aria-label="Listen for key press"]').trigger('click')
    document.dispatchEvent(new KeyboardEvent('keydown', { code: 'ControlLeft', key: 'Control' }))
    await wrapper.vm.$nextTick()

    const cancel = Array.from(document.querySelectorAll('button'))
      .find((button) => button.textContent?.includes('Cancel')) as HTMLButtonElement
    cancel.click()
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('KeyA')
    expect(document.querySelector('[data-testid="key-capture-overlay"]')).toBeNull()
  })

  it('captures a single modifier by its physical key code', async () => {
    const wrapper = await mountSuspended(Harness)

    await wrapper.get('button[aria-label="Listen for key press"]').trigger('click')
    document.dispatchEvent(new KeyboardEvent('keydown', { code: 'ShiftLeft', key: 'Shift' }))
    document.dispatchEvent(new KeyboardEvent('keyup', { code: 'ShiftLeft', key: 'Shift' }))
    await wrapper.vm.$nextTick()

    expect((wrapper.vm as any).value).toBe('ShiftLeft')
    expect(document.querySelector('[data-testid="key-capture-overlay"]')).toBeNull()
  })
})
