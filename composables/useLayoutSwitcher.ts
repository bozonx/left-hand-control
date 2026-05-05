// Watches for layout / Game Mode changes and, when running in auto
// mode, switches the active layout according to user-configured
// whitelist / blacklist conditions and the priority order.
//
// In manual mode this composable is a no-op (the user picked the
// active layout explicitly). In auto mode, when no layout matches, the
// active layout becomes empty so the mapper acts as a native passthrough.

import { pickActiveLayout } from '~/utils/layoutAutoSwitch'

const activeAutoLayoutId = ref<string | undefined>(undefined)

let started = false
let scope: ReturnType<typeof effectScope> | null = null

export function useLayoutSwitcher() {
  if (started) return { activeAutoLayoutId }
  started = true
  scope = effectScope(true)

  scope.run(() => {
    const { config } = useConfig()
    const library = useLayoutLibrary()
    const { layout: systemLayout } = useLayout()
    const gameMode = useGameMode()
    const activeWindow = useActiveWindow()

    async function evaluate() {
      const settings = config.value.settings
      if (settings.layoutMode !== 'auto') {
        activeAutoLayoutId.value = undefined
        return
      }

      const availableIds = library.entries.value.map((entry) => entry.id)
      const target = pickActiveLayout(availableIds, settings, {
        currentSystemLayout: systemLayout.value?.short ?? null,
        gameModeActive: !!gameMode.status.value.active,
        gameModeDetectionEnabled: gameMode.status.value.detectionEnabled,
        activeWindowTitle: activeWindow.state.value?.title ?? null,
        activeWindowAppId: activeWindow.state.value?.appId ?? null,
      })

      activeAutoLayoutId.value = target ?? undefined
    }

    // React to mode flip / order / conditions / system layout / game mode / window.
    watch(
      [
        () => config.value.settings.layoutMode,
        () => config.value.settings.layoutOrder,
        () => library.entries.value.map((e) => e.id).join('|'),
        () => systemLayout.value?.short ?? null,
        () => gameMode.status.value.active,
        () => gameMode.status.value.detectionEnabled,
        () => activeWindow.state.value?.title ?? null,
        () => activeWindow.state.value?.appId ?? null,
      ],
      () => {
        void evaluate()
      },
      { immediate: true },
    )

    watch(
      () => config.value.settings.layoutConditions,
      () => {
        void evaluate()
      },
      { immediate: true, deep: true },
    )
  })

  return { activeAutoLayoutId }
}

export function resetLayoutSwitcherStateForTests() {
  scope?.stop()
  scope = null
  started = false
  activeAutoLayoutId.value = undefined
}
