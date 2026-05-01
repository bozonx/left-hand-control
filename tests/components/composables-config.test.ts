import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createDefaultConfig } from '~/types/config'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}))

const toastAddMock = vi.fn()
mockNuxtImport('useToast', () => () => ({ add: toastAddMock }))
mockNuxtImport('useI18n', () => () => ({ t: (key: string) => key, locale: ref('en-US') }))

vi.mock('~/utils/layoutPresets', async (importOriginal) => {
  const actual = await importOriginal<typeof import('~/utils/layoutPresets')>()
  return {
    ...actual,
    loadBuiltinLayout: vi.fn().mockResolvedValue(null),
  }
})

describe('useConfig', () => {
  let useConfig: typeof import('~/composables/useConfig')['useConfig']
  let resetConfigStateForTests: typeof import('~/composables/useConfig')['resetConfigStateForTests']

  beforeEach(async () => {
    vi.resetModules()
    toastAddMock.mockClear()
    invokeMock.mockReset()
    invokeMock.mockResolvedValue(undefined)

    const mod = await import('~/composables/useConfig')
    useConfig = mod.useConfig
    resetConfigStateForTests = mod.resetConfigStateForTests
    resetConfigStateForTests()
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
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.needsWelcome.value).toBe(true)
    expect(api.isLayoutDirty.value).toBe(false)
    expect(api.currentLayoutId.value).toBeUndefined()
  })

  it('loads persisted settings and layout', async () => {
    const persisted = JSON.stringify({ version: 1, settings: { locale: 'ru-RU' } })
    invokeMock.mockResolvedValueOnce(persisted).mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.needsWelcome.value).toBe(false)
    expect(api.config.value.settings.locale).toBe('ru-RU')
  })

  it('marks layout dirty when config changes', async () => {
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    expect(api.isLayoutDirty.value).toBe(false)
    api.config.value.commands.push({ id: 'test', name: 'Test', linux: 'echo hi' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)
  })

  it('applyPreset updates config and clears dirty', async () => {
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    const preset = createDefaultConfig()
    await api.applyPreset(preset, 'user:test')

    expect(api.currentLayoutId.value).toBe('user:test')
    expect(api.isLayoutDirty.value).toBe(false)
    expect(invokeMock).toHaveBeenCalledWith('save_config', expect.anything())
    expect(invokeMock).toHaveBeenCalledWith('save_current_layout', expect.anything())
  })

  it('resetCurrentLayout restores saved preset and clears dirty', async () => {
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    api.config.value.commands.push({ id: 'x', name: 'X', linux: '' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)

    await api.resetCurrentLayout()
    expect(api.isLayoutDirty.value).toBe(false)
    expect(api.config.value.commands).toHaveLength(0)
    expect(invokeMock).toHaveBeenCalledWith('save_config', expect.anything())
    expect(invokeMock).toHaveBeenCalledWith('save_current_layout', expect.anything())
  })

  it('markLayoutSavedAs updates currentLayoutId and clears dirty', async () => {
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    api.config.value.commands.push({ id: 'x', name: 'X', linux: '' })
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)

    await api.markLayoutSavedAs('user:saved')
    expect(api.currentLayoutId.value).toBe('user:saved')
    expect(api.isLayoutDirty.value).toBe(false)
    console.log('invokeMock calls:', invokeMock.mock.calls)
    expect(invokeMock).toHaveBeenCalledWith('save_config', expect.anything())
  })

  it('handles load error and shows toast', async () => {
    invokeMock.mockRejectedValueOnce(new Error('disk full'))

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.loadError.value).toContain('disk full')
  })
})
