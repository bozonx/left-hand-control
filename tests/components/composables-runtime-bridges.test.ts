import { defineComponent, ref, nextTick, type Ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { commandActionRef, createDefaultConfig, macroActionRef, systemActionRef, textActionRef } from '~/types/config'
import { useAppLocale } from '~/composables/useAppLocale'
import { useAppTheme } from '~/composables/useAppTheme'
import { useMacros } from '~/composables/useMacros'

const {
  useConfigMock,
  useColorModeMock,
} = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
  useColorModeMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)
mockNuxtImport('useColorMode', () => useColorModeMock)

vi.mock('~/composables/useConfig', () => ({
  useConfig: () => useConfigMock(),
  resetConfigStateForTests: () => {},
}))

vi.mock('vue-i18n', async () => {
  const actual = await vi.importActual<typeof import('vue-i18n')>('vue-i18n')
  return {
    ...actual,
    useI18n: vi.fn(() => ({
      locale: ref('en-US'),
      t: (key: string, params?: Record<string, unknown>) => {
        if (key === 'systemActions.switchDesktop') return `Switch desktop ${params?.n}`
        return key
      },
    })),
  }
})

describe('runtime bridge composables', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
    useColorModeMock.mockReset()
  })

  it('useMacros resolves names and display labels for macros, commands and system actions', async () => {
    const config = ref(createDefaultConfig())
    config.value.macros = [{
      id: 'duplicateLine',
      name: 'User duplicate',
      steps: [],
    }]
    config.value.commands = [{
      id: 'toggleMusic',
      name: 'Toggle music',
      linux: 'playerctl play-pause',
    }]

    useConfigMock.mockReturnValue({
      config,
      loaded: ref(true),
    })

    let api: ReturnType<typeof useMacros>
    const Harness = defineComponent({
      setup() {
        api = useMacros()
        return {}
      },
      template: '<div />',
    })

    await mountSuspended(Harness)

    expect(api!.macroNameById('duplicateLine')).toBe('User duplicate')
    expect(api!.macroNameById('moveLineDown')).toBe('Move line down')
    expect(api!.displayAction(macroActionRef('duplicateLine'))).toBe('▶ User duplicate')
    expect(api!.displayAction(macroActionRef('moveLineDown'))).toBe('▶ Move line down')
    expect(api!.displayAction(commandActionRef('toggleMusic'))).toBe('> Toggle music')
    expect(api!.displayAction(systemActionRef('switchDesktop2'))).toBe('⚙ Switch desktop 2')
    expect(api!.displayAction(textActionRef('TODO: '))).toBe('T "TODO: "')
    expect(api!.displayAction('Enter')).toBe('Enter')
    expect(api!.displayAction('')).toBe('')
    expect(api!.MACRO_ACTION_PREFIX).toBe('macro:')
  })

  it('useAppLocale exposes resolved system-driven locale metadata', async () => {
    const config = ref(createDefaultConfig())
    const loaded = ref(true)
    config.value.settings.locale = 'auto'

    useConfigMock.mockReturnValue({
      config,
      loaded,
    })

    vi.stubGlobal('navigator', {
      languages: ['ru-RU'],
      language: 'ru-RU',
    })

    let api: ReturnType<typeof useAppLocale>
    const Harness = defineComponent({
      setup() {
        api = useAppLocale()
        return {}
      },
      template: '<div />',
    })

    await mountSuspended(Harness)

    await nextTick()
    expect(api!.systemLocale.value).toBe('ru-RU')
    expect(api!.resolved.value).toBe('ru-RU')
    expect(api!.available).toEqual(['en-US', 'ru-RU'])
  })

  it('useAppTheme derives resolved mode and toggles persisted preference', async () => {
    const config = ref(createDefaultConfig())
    const loaded = ref(true)
    config.value.settings.appearance = 'system'

    const colorMode = ref<'light' | 'dark'>('light') as Ref<'light' | 'dark'>
    ;(colorMode as any).preference = 'system'

    useConfigMock.mockReturnValue({
      config,
      loaded,
    })
    useColorModeMock.mockReturnValue(colorMode)

    let api: ReturnType<typeof useAppTheme>
    const Harness = defineComponent({
      setup() {
        api = useAppTheme()
        return {}
      },
      template: '<div />',
    })

    await mountSuspended(Harness)

    await nextTick()
    expect(api!.resolved.value).toBe('light')

    api!.toggle()
    await nextTick()
    expect(api!.preference.value).toBe('dark')
  })
})
