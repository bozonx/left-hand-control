import { parseMacroRef, type Macro, type MacroStep } from '~/types/config'
import { isCanonicalAction } from '~/utils/actionSyntax'
import { randomId } from '~/utils/keys'
import { systemMacroById, type SystemMacro } from '~/utils/systemMacros'
import { useEntityEditor } from '~/composables/useEntityEditor'
import { useActionUsage } from '~/composables/useActionUsage'

const ID_RE = /^[A-Za-z0-9_-]{1,64}$/

export function useMacroEditor() {
  const { config } = useConfig()
  const { t } = useI18n()
  const macrosRef = computed({
    get: () => config.value.macros,
    set: (val) => { config.value.macros = val },
  })
  const { newId, uiKeyOf, moveEntity } = useEntityEditor(macrosRef)
  const { usage } = useActionUsage(parseMacroRef)

  function addMacro() {
    if (!Array.isArray(config.value.macros)) config.value.macros = []
    const macro: Macro = {
      id: newId(),
      name: t('macros.defaultName'),
      steps: [],
      stepPauseMs: undefined,
      modifierDelayMs: undefined,
    }
    config.value.macros.unshift(macro)
    return macro
  }

  function cloneSystemMacro(sys: SystemMacro) {
    if (!Array.isArray(config.value.macros)) config.value.macros = []
    const macro: Macro = {
      id: newId(`${sys.id}Copy`),
      name: `${sys.name} ${t('macros.copySuffix')}`,
      steps: sys.steps.map((step) => ({ id: randomId(), keystroke: step.keystroke })),
      stepPauseMs: undefined,
      modifierDelayMs: undefined,
    }
    config.value.macros.unshift(macro)
    return macro
  }

  function removeMacro(uiKey: string) {
    config.value.macros = config.value.macros.filter(
      (macro) => uiKeyOf(macro) !== uiKey,
    )
  }

  function moveMacro(uiKey: string, delta: number) {
    moveEntity(uiKey, delta)
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

  function stepError(step: MacroStep): string | null {
    const raw = step.keystroke?.trim() ?? ''
    if (!raw) return null
    if (parseMacroRef(raw)) return t('macros.stepErrors.nestedMacro')
    if (!isCanonicalAction(raw)) return t('picker.invalidValue')
    return null
  }

  function stepWarning(step: MacroStep): string | null {
    const raw = step.keystroke?.trim() ?? ''
    if (!raw) return t('macros.stepWarnings.empty')
    return null
  }

  const hasStepErrors = computed(() =>
    config.value.macros.some((macro) =>
      macro.steps.some((step) => stepError(step) !== null),
    ),
  )

  const hasErrors = computed(() => hasIdErrors.value || hasStepErrors.value)

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
    stepError,
    stepWarning,
    hasStepErrors,
    hasErrors,
    usage,
  }
}
