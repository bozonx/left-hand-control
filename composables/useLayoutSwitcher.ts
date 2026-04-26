// Watches for layout / Game Mode changes and, when running in auto
// mode, switches the active layout according to user-configured
// whitelist / blacklist conditions and the priority order.
//
// In manual mode this composable is a no-op (the user picked the
// active layout explicitly). In auto mode, when no layout matches and
// no default is set, the active layout becomes empty (`undefined` id +
// empty preset) so the mapper acts as a native passthrough.

import { emptyLayoutPreset } from '~/utils/layoutPresets'
import { pickActiveLayout } from '~/utils/layoutAutoSwitch'

let started = false

export function useLayoutSwitcher() {
  if (started) return
  started = true

  const { config, applyPreset, currentLayoutId } = useConfig()
  const library = useLayoutLibrary()
  const { layout: systemLayout } = useLayout()
  const gameMode = useGameMode()

  let inFlight: Promise<void> | null = null
  let pendingTargetId: string | null | undefined = undefined
  let lastAppliedTarget: string | null | undefined = undefined

  async function switchTo(targetId: string | null) {
    if (targetId === currentLayoutId.value) {
      lastAppliedTarget = targetId
      return
    }
    if (targetId) {
      const preset = await library.loadPreset(targetId)
      if (!preset) return
      await applyPreset(preset, targetId)
    } else {
      // No matching layout — drop to native passthrough.
      await applyPreset(emptyLayoutPreset(), undefined)
    }
    lastAppliedTarget = targetId
  }

  async function evaluate() {
    const settings = config.value.settings
    if (settings.layoutMode !== 'auto') return

    const availableIds = library.entries.value.map((entry) => entry.id)
    const target = pickActiveLayout(availableIds, settings, {
      currentSystemLayout: systemLayout.value?.short ?? null,
      gameModeActive: !!gameMode.status.value.active,
    })

    if (target === lastAppliedTarget && target === currentLayoutId.value) return

    pendingTargetId = target
    if (inFlight) return
    inFlight = (async () => {
      try {
        // Drain any further updates that happened while we were busy.
        // eslint-disable-next-line no-constant-condition
        while (true) {
          const next = pendingTargetId
          pendingTargetId = undefined
          await switchTo(next ?? null)
          if (pendingTargetId === undefined) break
        }
      } finally {
        inFlight = null
      }
    })()
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
}
