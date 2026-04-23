import { type Macro, type MacroStep } from '~/types/config'
import { randomId } from '~/utils/keys'
import { systemMacroById, type SystemMacro } from '~/utils/systemMacros'

const ID_RE = /^[A-Za-z0-9_-]{1,64}$/

export function useMacroEditor() {
  const { config } = useConfig()
  const { t } = useI18n()
  const macroUiKeys = new WeakMap<Macro, string>()

  function newMacroId(base?: string): string {
    if (base && !config.value.macros.some((macro) => macro.id === base)) return base
    if (base) {
      for (let i = 2; i < 1000; i++) {
        const candidate = `${base}${i}`
        if (!config.value.macros.some((macro) => macro.id === candidate)) return candidate
      }
    }
    let id: string
    do {
      id = randomId()
    } while (config.value.macros.some((macro) => macro.id === id))
    return id
  }

  function addMacro() {
    if (!Array.isArray(config.value.macros)) config.value.macros = []
    config.value.macros.push({
      id: newMacroId(),
      name: t('macros.defaultName'),
      steps: [],
      stepPauseMs: undefined,
      modifierDelayMs: undefined,
    })
  }

  function cloneSystemMacro(sys: SystemMacro) {
    if (!Array.isArray(config.value.macros)) config.value.macros = []
    config.value.macros.push({
      id: newMacroId(`${sys.id}Copy`),
      name: `${sys.name} ${t('macros.copySuffix')}`,
      steps: sys.steps.map((step) => ({ id: randomId(), keystroke: step.keystroke })),
      stepPauseMs: undefined,
      modifierDelayMs: undefined,
    })
  }

  function removeMacro(uiKey: string) {
    config.value.macros = config.value.macros.filter(
      (macro) => uiKeyOf(macro) !== uiKey,
    )
  }

  function moveMacro(uiKey: string, delta: number) {
    const index = config.value.macros.findIndex((macro) => uiKeyOf(macro) === uiKey)
    const next = index + delta
    if (index < 0 || next < 0 || next >= config.value.macros.length) return
    const macros = config.value.macros.slice()
    const [item] = macros.splice(index, 1) as [Macro]
    macros.splice(next, 0, item)
    config.value.macros = macros
  }

  function addStep(macro: Macro) {
    macro.steps.push({ id: randomId(), keystroke: '' })
  }

  function removeStep(macro: Macro, stepId: string) {
    macro.steps = macro.steps.filter((step) => step.id !== stepId)
  }

  function moveStep(macro: Macro, index: number, delta: number) {
    const next = index + delta
    if (next < 0 || next >= macro.steps.length) return
    const steps = macro.steps.slice()
    const [item] = steps.splice(index, 1) as [MacroStep]
    steps.splice(next, 0, item)
    macro.steps = steps
  }

  function uiKeyOf(macro: Macro) {
    let key = macroUiKeys.get(macro)
    if (!key) {
      key = randomId()
      macroUiKeys.set(macro, key)
    }
    return key
  }

  const idCounts = computed<Record<string, number>>(() => {
    const counts: Record<string, number> = {}
    for (const macro of config.value.macros) {
      counts[macro.id] = (counts[macro.id] ?? 0) + 1
    }
    return counts
  })

  function idError(macro: Macro): string | null {
    const raw = macro.id ?? ''
    if (raw.trim() === '') return t('macros.idErrors.empty')
    if (!ID_RE.test(raw)) return t('macros.idErrors.format')
    if ((idCounts.value[raw] ?? 0) > 1) return t('macros.idErrors.dupUser')
    const systemMacro = systemMacroById(raw)
    if (systemMacro) {
      return t('macros.idErrors.dupSystem', { name: systemMacro.name })
    }
    return null
  }

  const hasIdErrors = computed(() =>
    config.value.macros.some((macro) => idError(macro) !== null),
  )

  const usage = computed(() => {
    const byMacro: Record<string, string[]> = {}
    const note = (id: string, where: string) => {
      if (!byMacro[id]) byMacro[id] = []
      byMacro[id].push(where)
    }
    const noteIfMacro = (action: string | null | undefined, where: string) => {
      if (action?.startsWith(prefix)) {
        note(action.slice(prefix.length), where)
      }
    }
    const prefix = 'macro:'
    for (const rule of config.value.rules) {
      const ruleLabel = rule.key || '?'
      noteIfMacro(rule.tapAction, `rule ${ruleLabel} (tap)`)
      noteIfMacro(rule.holdAction, `rule ${ruleLabel} (hold)`)
      noteIfMacro(rule.doubleTapAction, `rule ${ruleLabel} (double-tap)`)
    }
    for (const [layerId, keymap] of Object.entries(config.value.layerKeymaps)) {
      for (const [code, action] of Object.entries(keymap.keys ?? {})) {
        noteIfMacro(action, `${layerId}.${code}`)
      }
      for (const extra of keymap.extras ?? []) {
        noteIfMacro(extra.action, `${layerId}.${extra.name || 'extra'}`)
      }
    }
    return byMacro
  })

  return {
    addMacro,
    cloneSystemMacro,
    removeMacro,
    moveMacro,
    addStep,
    removeStep,
    moveStep,
    uiKeyOf,
    idError,
    hasIdErrors,
    usage,
  }
}
