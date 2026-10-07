import { defineComponent, nextTick, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import { commandFingerprint, commandsTrusted } from '~/utils/commandTrust'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))

vi.mock('~/composables/useTauri', () => ({
  useTauri: vi.fn().mockResolvedValue({ invoke: invokeMock }),
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

  it('retains permission for local edits but blocks replacement and revoked commands', async () => {
    invokeMock.mockResolvedValueOnce('').mockResolvedValueOnce('')
    const api = await getApi()
    await api.load()
    api.config.value.settings.currentLayoutId = 'user:test'
    api.config.value.settings.commandTrust['user:test'] = { fingerprint: commandFingerprint([]), trustedAt: '' }
    api.config.value.commands.push({ id: 'hello', name: 'Hello', linux: 'printf hello' })
    expect(commandsTrusted(api.config.value)).toBe(true)
    api.config.value.commands[0]!.workingDirectory = '~/Documents'
    expect(commandsTrusted(api.config.value)).toBe(true)
    const replacement = createDefaultConfig()
    replacement.commands.push({ id: 'hello', name: 'Hello', linux: 'printf replaced' })
    await api.replaceCurrentLayoutSnapshot(replacement, 'user:test')
    expect(commandsTrusted(api.config.value)).toBe(false)
    api.config.value.commands[0]!.linux = 'printf blocked'
    expect(commandsTrusted(api.config.value)).toBe(false)
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

    api.config.value.commands = [...api.config.value.commands, { id: 'x', name: 'X', linux: '' }]
    await nextTick()
    expect(api.isLayoutDirty.value).toBe(true)

    await api.markLayoutSavedAs('user:saved')
    expect(api.currentLayoutId.value).toBe('user:saved')
    expect(api.isLayoutDirty.value).toBe(false)
  })

  it('handles load error and shows toast', async () => {
    invokeMock.mockRejectedValueOnce(new Error('disk full'))

    const api = await getApi()
    await api.load()

    expect(api.loaded.value).toBe(true)
    expect(api.loadError.value).toContain('disk full')
  })

  it('reloads instead of overwriting when a save hits an external change', async () => {
    const persisted = JSON.stringify({ version: 1, settings: {} })
    const changed = JSON.stringify({ version: 1, settings: { locale: 'ru-RU' } })
    invokeMock.mockResolvedValueOnce(persisted).mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'save_config') {
        throw new Error('EXTERNAL_CHANGE: file was changed by another process')
      }
      if (command === 'load_config') return changed
      return ''
    })
    api.config.value.settings.locale = 'en-US'
    await nextTick()
    await expect(api.flush()).rejects.toThrow('EXTERNAL_CHANGE')
    await vi.waitFor(() => expect(api.config.value.settings.locale).toBe('ru-RU'))
    expect(toastAddMock).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'app.externalChangeTitle' }),
    )
    expect(toastAddMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ title: 'app.saveFailedTitle' }),
    )
  })

  it('reloads when files changed on disk and no save is pending', async () => {
    const persisted = JSON.stringify({ version: 1, settings: {} })
    invokeMock.mockResolvedValueOnce(persisted).mockResolvedValueOnce('')

    const api = await getApi()
    await api.load()

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'config_changed_on_disk') return false
      return ''
    })
    expect(await api.reloadIfChangedOnDisk()).toBe(false)

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'config_changed_on_disk') return true
      if (command === 'load_config') {
        return JSON.stringify({ version: 1, settings: { locale: 'ru-RU' } })
      }
      return ''
    })
    expect(await api.reloadIfChangedOnDisk()).toBe(true)
    expect(api.config.value.settings.locale).toBe('ru-RU')
  })
})
