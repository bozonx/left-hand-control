import { defineComponent, nextTick, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { useConfig, resetConfigStateForTests } from '~/composables/useConfig'
import { createDefaultConfig } from '~/types/config'

const toastAddMock = vi.fn()
mockNuxtImport('useToast', () => () => ({ add: toastAddMock }))

mockNuxtImport('useI18n', () => () => ({
  t: (key: string) => key,
  locale: ref('en-US'),
}))

const persistNowMock = vi.fn().mockResolvedValue(undefined)
const flushMock = vi.fn().mockResolvedValue(undefined)
const scheduleSaveMock = vi.fn()

mockNuxtImport('usePersistedState', () => () => ({
  saving: ref(false),
  scheduleSave: scheduleSaveMock,
  flush: flushMock,
  persistNow: persistNowMock,
}))

vi.mock('~/composables/config/storage', () => ({
  readConfigRaw: vi.fn(),
  readCurrentLayoutRaw: vi.fn(),
  writeConfigRaw: vi.fn().mockResolvedValue(undefined),
  writeCurrentLayoutRaw: vi.fn().mockResolvedValue(undefined),
  writeUserLayoutRaw: vi.fn().mockResolvedValue('ivan-k.yaml'),
  getSettingsDir: vi.fn().mockResolvedValue('/tmp/settings'),
}))

vi.mock('~/utils/layoutPresets', async (importOriginal) => {
  const actual = await importOriginal<typeof import('~/utils/layoutPresets')>()
  return {
    ...actual,
    loadBuiltinLayout: vi.fn().mockResolvedValue(null),
  }
})

describe('useConfig', () => {
  beforeEach(() => {
    resetConfigStateForTests()
    toastAddMock.mockClear()
    persistNowMock.mockClear()
    flushMock.mockClear()
    scheduleSaveMock.mockClear()
  })

  async function getApi() {
    let api: ReturnType<typeof useConfig>
    const Harness = defineComponent({
      setup() {
        api = useConfig()
        return {}
      },
      template: '<div />',
    })
    await mountSuspended(Harness)
    return api!
  }

  it('starts with default config and is not dirty', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockResolvedValue('')
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.needsWelcome.value).toBe(true)
    expect(api.isLayoutDirty.value).toBe(false)
    expect(api.currentLayoutId.value).toBeUndefined()
  })

  it('loads persisted settings and layout', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    const persisted = JSON.stringify({ version: 1, settings: { locale: 'ru-RU' } })
    ;(readConfigRaw as any).mockResolvedValue(persisted)
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.needsWelcome.value).toBe(false)
    expect(api.config.value.settings.locale).toBe('ru-RU')
  })

  it('marks layout dirty when config changes', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockResolvedValue('')
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    expect(api.isLayoutDirty.value).toBe(false)
    api.config.value.commands.push({ id: 'test', name: 'Test', linux: 'echo hi' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)
  })

  it('applyPreset updates config and clears dirty', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockResolvedValue('')
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    const preset = createDefaultConfig()
    await api.applyPreset(preset, 'user:test')

    expect(api.currentLayoutId.value).toBe('user:test')
    expect(api.isLayoutDirty.value).toBe(false)
    expect(flushMock).toHaveBeenCalled()
    expect(persistNowMock).toHaveBeenCalled()
  })

  it('resetCurrentLayout restores saved preset and clears dirty', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockResolvedValue('')
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    api.config.value.commands.push({ id: 'x', name: 'X', linux: '' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)

    await api.resetCurrentLayout()
    expect(api.isLayoutDirty.value).toBe(false)
    expect(api.config.value.commands).toHaveLength(0)
    expect(flushMock).toHaveBeenCalled()
    expect(persistNowMock).toHaveBeenCalled()
  })

  it('markLayoutSavedAs updates currentLayoutId and clears dirty', async () => {
    const { readConfigRaw, readCurrentLayoutRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockResolvedValue('')
    ;(readCurrentLayoutRaw as any).mockResolvedValue('')

    const api = await getApi()
    await api.load()

    api.config.value.commands.push({ id: 'x', name: 'X', linux: '' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)

    await api.markLayoutSavedAs('user:saved')
    expect(api.currentLayoutId.value).toBe('user:saved')
    expect(api.isLayoutDirty.value).toBe(false)
    expect(flushMock).toHaveBeenCalled()
  })

  it('handles load error and shows toast', async () => {
    const { readConfigRaw } = await import('~/composables/config/storage')
    ;(readConfigRaw as any).mockRejectedValue(new Error('disk full'))

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.loadError.value).toContain('disk full')
  })
})
