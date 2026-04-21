import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { createDefaultConfig } from '~/types/config'
import { useSettingsScreen } from '~/composables/useSettingsScreen'

const {
  useConfigMock,
  useLayoutLibraryMock,
  useMapperMock,
  usePlatformInfoMock,
  useAppThemeMock,
  useAppLocaleMock,
  useI18nMock,
} = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
  useLayoutLibraryMock: vi.fn(),
  useMapperMock: vi.fn(),
  usePlatformInfoMock: vi.fn(),
  useAppThemeMock: vi.fn(),
  useAppLocaleMock: vi.fn(),
  useI18nMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)
mockNuxtImport('useLayoutLibrary', () => useLayoutLibraryMock)
mockNuxtImport('useMapper', () => useMapperMock)
mockNuxtImport('usePlatformInfo', () => usePlatformInfoMock)
mockNuxtImport('useAppTheme', () => useAppThemeMock)
mockNuxtImport('useAppLocale', () => useAppLocaleMock)
mockNuxtImport('useI18n', () => useI18nMock)

vi.mock('~/utils/layoutPresets', async () => {
  const actual = await vi.importActual<typeof import('~/utils/layoutPresets')>('~/utils/layoutPresets')
  return {
    ...actual,
    loadBuiltinLayout: vi.fn(),
    extractPresetFromConfig: vi.fn(),
    emptyLayoutPreset: vi.fn(),
  }
})

function t(key: string, params?: Record<string, unknown>) {
  if (key === 'settings.languageAutoResolved') {
    return `Auto (system -> ${params?.resolved})`
  }
  if (key === 'settings.emptyLayoutName') return 'Empty layout'
  if (key === 'settings.loadFailed') return `Failed to load ${params?.name}`
  if (key === 'settings.saveErrorEmpty') return 'Please enter a name.'
  if (key === 'settings.appearanceItems.system') return 'Use system'
  if (key === 'settings.appearanceItems.light') return 'Light'
  if (key === 'settings.appearanceItems.dark') return 'Dark'
  return key
}

describe('useSettingsScreen', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
    useLayoutLibraryMock.mockReset()
    useMapperMock.mockReset()
    usePlatformInfoMock.mockReset()
    useAppThemeMock.mockReset()
    useAppLocaleMock.mockReset()
    useI18nMock.mockReset()
  })

  it('refreshes dependencies on mount and exposes derived device/locale options', async () => {
    const config = ref(createDefaultConfig())
    config.value.settings.inputDevicePath = '/dev/input/event1'

    const flush = vi.fn()
    const library = {
      entries: ref([]),
      error: ref(null),
      layoutsDir: ref('/tmp/layouts'),
      refresh: vi.fn(),
      loadPreset: vi.fn(),
      saveUserPreset: vi.fn(),
      deleteUserPreset: vi.fn(),
    }
    const mapper = {
      devices: ref([{ name: 'Keyboard', path: '/dev/input/event1' }]),
      status: ref({ running: false, device_path: null, last_error: null }),
      busy: ref(false),
      error: ref<string | null>(null),
      refreshDevices: vi.fn(),
      refreshStatus: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
    }
    const platform = {
      info: ref(null),
      busy: ref(false),
      error: ref<string | null>(null),
      refresh: vi.fn(),
    }

    useConfigMock.mockReturnValue({
      config,
      configPath: ref('/tmp/config.json'),
      flush,
      applyPreset: vi.fn(),
      markLayoutSavedAs: vi.fn(),
      currentLayoutId: ref(undefined),
      isLayoutDirty: ref(false),
    })
    useLayoutLibraryMock.mockReturnValue(library)
    useMapperMock.mockReturnValue(mapper)
    usePlatformInfoMock.mockReturnValue(platform)
    useAppThemeMock.mockReturnValue({
      preference: ref('system'),
      resolved: ref('light'),
    })
    useAppLocaleMock.mockReturnValue({
      preference: ref('auto'),
      systemLocale: ref('ru-RU'),
      available: ['en-US', 'ru-RU'],
    })
    useI18nMock.mockReturnValue({ t })

    const Harness = defineComponent({
      setup() {
        return useSettingsScreen()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    expect(mapper.refreshDevices).toHaveBeenCalledTimes(1)
    expect(mapper.refreshStatus).toHaveBeenCalledTimes(1)
    expect(platform.refresh).toHaveBeenCalledTimes(1)
    expect(library.refresh).toHaveBeenCalledTimes(1)
    expect(vm.deviceOptions).toEqual([
      { label: 'Keyboard  —  /dev/input/event1', value: '/dev/input/event1' },
    ])
    expect(vm.selectedDevice).toBe('/dev/input/event1')
    vm.selectedDevice = '/dev/input/event2'
    expect(config.value.settings.inputDevicePath).toBe('/dev/input/event2')
    expect(vm.appearanceItems).toEqual([
      { label: 'Use system', value: 'system' },
      { label: 'Light', value: 'light' },
      { label: 'Dark', value: 'dark' },
    ])
    expect(vm.localeItems).toEqual([
      { label: 'Auto (system -> Русский)', value: 'auto' },
      { label: 'English', value: 'en-US' },
      { label: 'Русский', value: 'ru-RU' },
    ])
  })

  it('applies layouts, saves current layout, deletes active layout and toggles mapper', async () => {
    const { emptyLayoutPreset, extractPresetFromConfig, loadBuiltinLayout } =
      await import('~/utils/layoutPresets')

    const config = ref(createDefaultConfig())
    config.value.settings.currentLayoutId = 'user:old'
    config.value.settings.inputDevicePath = '/dev/input/event1'

    const applyPreset = vi.fn()
    const markLayoutSavedAs = vi.fn()
    const flush = vi.fn()
    const library = {
      entries: ref([]),
      error: ref(null),
      layoutsDir: ref('/tmp/layouts'),
      refresh: vi.fn(),
      loadPreset: vi.fn().mockResolvedValue({ name: 'Nav' }),
      saveUserPreset: vi.fn().mockResolvedValue('renamed'),
      deleteUserPreset: vi.fn(),
    }
    const mapper = {
      devices: ref([{ name: 'Keyboard', path: '/dev/input/event1' }]),
      status: ref({ running: false, device_path: null, last_error: null }),
      busy: ref(false),
      error: ref<string | null>(null),
      refreshDevices: vi.fn(),
      refreshStatus: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
    }

    useConfigMock.mockReturnValue({
      config,
      configPath: ref('/tmp/config.json'),
      flush,
      applyPreset,
      markLayoutSavedAs,
      currentLayoutId: ref('user:old'),
      isLayoutDirty: ref(true),
    })
    useLayoutLibraryMock.mockReturnValue(library)
    useMapperMock.mockReturnValue(mapper)
    usePlatformInfoMock.mockReturnValue({
      info: ref(null),
      busy: ref(false),
      error: ref(null),
      refresh: vi.fn(),
    })
    useAppThemeMock.mockReturnValue({
      preference: ref('system'),
      resolved: ref('light'),
    })
    useAppLocaleMock.mockReturnValue({
      preference: ref('auto'),
      systemLocale: ref('en-US'),
      available: ['en-US', 'ru-RU'],
    })
    useI18nMock.mockReturnValue({ t })

    vi.mocked(emptyLayoutPreset).mockReturnValue({ name: 'Empty layout' } as any)
    vi.mocked(extractPresetFromConfig).mockReturnValue({ name: 'Current' } as any)
    vi.mocked(loadBuiltinLayout).mockResolvedValue({ name: 'Built-in' } as any)

    const Harness = defineComponent({
      setup() {
        return useSettingsScreen()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    vm.requestApplyEntry({ id: 'builtin:ivank', name: 'Built-in', builtin: true })
    await vm.confirmApply()
    expect(loadBuiltinLayout).toHaveBeenCalledTimes(1)
    expect(applyPreset).toHaveBeenCalledWith({ name: 'Built-in' }, 'builtin:ivank')

    vm.requestApplyEntry({ id: 'user:nav', name: 'Nav', builtin: false })
    await vm.confirmApply()
    expect(library.loadPreset).toHaveBeenCalledWith('user:nav')
    expect(applyPreset).toHaveBeenCalledWith({ name: 'Nav' }, 'user:nav')

    vm.requestApplyEmpty()
    await vm.confirmApply()
    expect(emptyLayoutPreset).toHaveBeenCalledWith('Empty layout')
    expect(applyPreset).toHaveBeenCalledWith({ name: 'Empty layout' }, undefined)

    vm.openSaveModal()
    expect(vm.saveName).toBe('old')
    vm.saveName = '  My Layout  '
    await vm.performSave()
    expect(extractPresetFromConfig).toHaveBeenCalledWith(config.value, 'My Layout')
    expect(library.saveUserPreset).toHaveBeenCalledWith('My Layout', { name: 'Current' })
    expect(markLayoutSavedAs).toHaveBeenCalledWith('user:renamed')
    expect(vm.saveModalOpen).toBe(false)

    vm.deletePending = { id: 'user:old', name: 'Old', builtin: false }
    await vm.confirmDelete()
    expect(library.deleteUserPreset).toHaveBeenCalledWith('old')
    expect(config.value.settings.currentLayoutId).toBeUndefined()
    expect(flush).toHaveBeenCalled()

    await vm.toggleMapper()
    expect(flush).toHaveBeenCalled()
    expect(mapper.start).toHaveBeenCalledWith('/dev/input/event1')

    mapper.status.value.running = true
    await vm.toggleMapper()
    expect(mapper.stop).toHaveBeenCalledTimes(1)
  })

  it('surfaces apply/save errors and respects guard clauses', async () => {
    const { loadBuiltinLayout } = await import('~/utils/layoutPresets')

    const config = ref(createDefaultConfig())
    const flush = vi.fn()
    const library = {
      entries: ref([]),
      error: ref(null),
      layoutsDir: ref('/tmp/layouts'),
      refresh: vi.fn(),
      loadPreset: vi.fn().mockResolvedValue(null),
      saveUserPreset: vi.fn(),
      deleteUserPreset: vi.fn(),
    }
    const mapper = {
      devices: ref([]),
      status: ref({ running: false, device_path: null, last_error: null }),
      busy: ref(false),
      error: ref<string | null>(null),
      refreshDevices: vi.fn(),
      refreshStatus: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
    }

    useConfigMock.mockReturnValue({
      config,
      configPath: ref('/tmp/config.json'),
      flush,
      applyPreset: vi.fn(),
      markLayoutSavedAs: vi.fn(),
      currentLayoutId: ref(undefined),
      isLayoutDirty: ref(false),
    })
    useLayoutLibraryMock.mockReturnValue(library)
    useMapperMock.mockReturnValue(mapper)
    usePlatformInfoMock.mockReturnValue({
      info: ref(null),
      busy: ref(false),
      error: ref(null),
      refresh: vi.fn(),
    })
    useAppThemeMock.mockReturnValue({
      preference: ref('system'),
      resolved: ref('light'),
    })
    useAppLocaleMock.mockReturnValue({
      preference: ref('auto'),
      systemLocale: ref('en-US'),
      available: ['en-US', 'ru-RU'],
    })
    useI18nMock.mockReturnValue({ t })

    vi.mocked(loadBuiltinLayout).mockResolvedValue(null)

    const Harness = defineComponent({
      setup() {
        return useSettingsScreen()
      },
      template: '<div />',
    })

    const wrapper = await mountSuspended(Harness)
    const vm = wrapper.vm as any

    vm.requestApplyEntry({ id: 'user:missing', name: 'Missing', builtin: false })
    await vm.confirmApply()
    expect(vm.applyError).toBe('Failed to load Missing')

    vm.saveName = '   '
    await vm.performSave()
    expect(vm.saveError).toBe('Please enter a name.')

    vm.deletePending = { id: 'builtin:ivank', name: 'Built-in', builtin: true }
    await vm.confirmDelete()
    expect(library.deleteUserPreset).not.toHaveBeenCalled()

    await vm.toggleMapper()
    expect(mapper.start).not.toHaveBeenCalled()
  })
})
