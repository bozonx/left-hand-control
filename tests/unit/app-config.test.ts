import { beforeEach, describe, expect, it, vi } from 'vitest'

describe('app modal config', () => {
  beforeEach(() => {
    vi.resetModules()
    vi.stubGlobal('defineAppConfig', <T>(config: T) => config)
  })

  it('keeps modal content free from scale animation so it can size to content height', async () => {
    const { default: appConfig } = await import('~/app.config')
    const modal = appConfig.ui.modal

    expect(modal.variants.transition.true.content).toBe('')

    const flatten = (v: unknown): string => {
      if (!v) return ''
      if (typeof v === 'string') return v
      if (Array.isArray(v)) return v.map(flatten).join(' ')
      if (typeof v === 'object') return Object.values(v as Record<string, unknown>).map(flatten).join(' ')
      return ''
    }
    const all = flatten(modal)
    expect(all).not.toContain('scale-in')
    expect(all).not.toContain('scale-out')
    expect(all).not.toMatch(/!bottom-0/)
    expect(all).not.toMatch(/!top-0/)
    expect(all).toContain('backface-hidden')
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
