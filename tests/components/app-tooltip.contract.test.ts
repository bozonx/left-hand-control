import { defineComponent, nextTick } from 'vue'

import { mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import AppTooltip from '~/components/shared/AppTooltip.vue'

const UTooltipStub = defineComponent({
  props: {
    open: { type: Boolean, default: false },
    reference: { type: Object, default: null },
    disabled: { type: Boolean, default: false },
    delayDuration: { type: Number, default: 0 },
    ui: { type: Object, default: () => ({}) },
  },
  emits: ['update:open'],
  template: `
    <div data-testid="tooltip-root">
      <slot />
      <div v-if="open" data-testid="tooltip-content">
        <slot name="content" />
      </div>
    </div>
  `,
})

describe('AppTooltip', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  async function openTooltip(wrapper: Awaited<ReturnType<typeof mountSuspended>>) {
    await wrapper.find('span').trigger('mouseenter')
    vi.advanceTimersByTime(1)
    await nextTick()
  }

  it('shows tooltip on hover after hoverDelay', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { text: 'Hello', hoverDelay: 100 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: { default: '<button>btn</button>' },
    })

    await wrapper.find('span').trigger('mouseenter')
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)

    vi.advanceTimersByTime(99)
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)

    vi.advanceTimersByTime(1)
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="tooltip-content"]').text()).toBe('Hello')
  })

  it('hides tooltip on mouseleave', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { text: 'Hello', hoverDelay: 1 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: { default: '<button>btn</button>' },
    })

    await openTooltip(wrapper)
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(true)

    await wrapper.find('span').trigger('mouseleave')
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)
  })

  it('does not open when disabled', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { text: 'Hello', disabled: true, hoverDelay: 1 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: { default: '<button>btn</button>' },
    })

    await wrapper.find('span').trigger('mouseenter')
    vi.advanceTimersByTime(10)
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)
  })

  it('toggles on click when toggleOnClick is true', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { text: 'Toggle', toggleOnClick: true, hoverDelay: 1 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: { default: '<button>btn</button>' },
    })

    const span = wrapper.find('span')
    await span.trigger('click')
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(true)

    await span.trigger('click')
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)
  })

  it('uses slot content instead of text when provided', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { hoverDelay: 1 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: {
        default: '<button>btn</button>',
        content: '<div data-testid="custom-content">Custom</div>',
      },
    })

    await openTooltip(wrapper)
    expect(wrapper.find('[data-testid="custom-content"]').exists()).toBe(true)
  })

  it('closes tooltip when disabled prop changes to true', async () => {
    const wrapper = await mountSuspended(AppTooltip, {
      props: { text: 'Hello', hoverDelay: 1 },
      global: { stubs: { UTooltip: UTooltipStub } },
      slots: { default: '<button>btn</button>' },
    })

    await openTooltip(wrapper)
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(true)

    await wrapper.setProps({ disabled: true })
    await nextTick()
    expect(wrapper.find('[data-testid="tooltip-content"]').exists()).toBe(false)
  })
})
