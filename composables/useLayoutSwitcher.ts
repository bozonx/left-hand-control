// Watches for layout / Game Mode changes and, when running in auto
// mode, switches the active layout according to user-configured
// whitelist / blacklist conditions and the priority order.
//
// In manual mode this composable is a no-op (the user picked the
// active layout explicitly). In auto mode, when no layout matches and
// no default is set, the active layout becomes empty (`undefined` id +
// empty preset) so the mapper acts as a native passthrough.

import { pickActiveLayout } from '~/utils/layoutAutoSwitch'

const activeAutoLayoutId = ref<string | undefined>(undefined)

let started = false

export function useLayoutSwitcher() {
  if (started) return { activeAutoLayoutId }
  started = true

  const { config } = useConfig()
  const library = useLayoutLibrary()
  const { layout: systemLayout } = useLayout()
  const gameMode = useGameMode()

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
    })

    activeAutoLayoutId.value = target ?? undefined
  }

  // React to mode flip / order / conditions / system layout / game mode.
  watch(
    [
      () => config.value.settings.layoutMode,
      () => config.value.settings.layoutOrder,
      () => config.value.settings.layoutConditions,
      () => config.value.settings.autoDefaultLayoutId,
      () => library.entries.value.map((e) => e.id).join('|'),
      () => systemLayout.value?.short ?? null,
      () => gameMode.status.value.active,
    ],
    () => {
      void evaluate()
    },
    { deep: true, immediate: true },
  )

  return { activeAutoLayoutId }
}

