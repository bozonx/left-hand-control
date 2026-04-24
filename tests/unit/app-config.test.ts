import { beforeEach, describe, expect, it, vi } from 'vitest'

describe('app modal config', () => {
  beforeEach(() => {
    vi.resetModules()
    vi.stubGlobal('defineAppConfig', <T>(config: T) => config)
  })

  it('keeps modal content free from transform-based centering and scale animation', async () => {
    const { default: appConfig } = await import('~/app.config')
    const modal = appConfig.ui.modal

    expect(modal.variants.transition.true.content).toBe('')

    expect(modal.compoundVariants).toContainEqual({
      scrollable: false,
      fullscreen: false,
      class: {
        content: expect.stringContaining('!translate-x-0'),
      },
    })

    expect(modal.compoundVariants).toContainEqual({
      scrollable: false,
      fullscreen: false,
      class: {
        content: expect.stringContaining('!translate-y-0'),
      },
    })
  })

  it('keeps overlay popups free from scale-based content animations', async () => {
    const { default: appConfig } = await import('~/app.config')
    const ui = appConfig.ui

    expect(ui.tooltip.slots.content).not.toContain('scale-in')
    expect(ui.tooltip.slots.content).not.toContain('scale-out')

    expect(ui.popover.slots.content).not.toContain('scale-in')
    expect(ui.popover.slots.content).not.toContain('scale-out')

    expect(ui.select.slots.content).not.toContain('scale-in')
    expect(ui.select.slots.content).not.toContain('scale-out')

    expect(ui.selectMenu.slots.content).not.toContain('scale-in')
    expect(ui.selectMenu.slots.content).not.toContain('scale-out')

    expect(ui.inputMenu.slots.content).not.toContain('scale-in')
    expect(ui.inputMenu.slots.content).not.toContain('scale-out')

    expect(ui.dropdownMenu.slots.content).not.toContain('scale-in')
    expect(ui.dropdownMenu.slots.content).not.toContain('scale-out')

    expect(ui.contextMenu.slots.content).not.toContain('scale-in')
    expect(ui.contextMenu.slots.content).not.toContain('scale-out')
  })
})
