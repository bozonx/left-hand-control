import { useI18n } from 'vue-i18n'
import {
  type Macro,
  MACRO_ACTION_PREFIX,
  parseCommandRef,
  parseMacroRef,
  parseSystemRef,
  parseTextAction,
} from '~/types/config'
import { systemActionById } from '~/utils/systemActions'
import { systemMacroById } from '~/utils/systemMacros'

// Helpers around macro references. `macro:<id>` resolves against the user's
// `config.macros` first and falls back to the built-in system macros
// catalog (see `utils/systemMacros.ts`). User macros with the same id as
// a system macro override it.

export function useMacros() {
  const { config } = useConfig()
  const { t } = useI18n()
  const { commandNameById } = useCommands()

  const macros = computed<Macro[]>(() => config.value.macros ?? [])

  const byId = computed<Record<string, Macro>>(() => {
    const m: Record<string, Macro> = {}
    for (const macro of macros.value) m[macro.id] = macro
    return m
  })

  function macroNameById(id: string): string | undefined {
    return byId.value[id]?.name ?? systemMacroById(id)?.name
  }

  function systemActionName(id: string): string | undefined {
    const found = systemActionById(id)
    if (!found) return undefined
    return t(found.nameKey, found.nameParams ?? {})
  }

  function displayAction(action: string | undefined | null): string {
    if (!action) return ''
    const macroRef = parseMacroRef(action)
    if (macroRef) {
      const name = macroNameById(macroRef)
      return name ? `▶ ${name}` : action
    }
    const cmdRef = parseCommandRef(action)
    if (cmdRef) {
      const name = commandNameById(cmdRef)
      return name ? `> ${name}` : action
    }
    const sysRef = parseSystemRef(action)
    if (sysRef) {
      const name = systemActionName(sysRef)
      return name ? `⚙ ${name}` : action
    }
    const textAction = parseTextAction(action)
    if (textAction !== null) {
      return `T "${textAction}"`
    }
    return action
  }

  function getActionInfo(action: string | undefined | null): { label: string, icon?: string } {
    if (!action) return { label: '' }

    const macroRef = parseMacroRef(action)
    if (macroRef) {
      return {
        label: macroNameById(macroRef) ?? action,
        icon: 'i-lucide-zap',
      }
    }

    const cmdRef = parseCommandRef(action)
    if (cmdRef) {
      return {
        label: commandNameById(cmdRef) ?? action,
        icon: 'i-lucide-terminal',
      }
    }

    const sysRef = parseSystemRef(action)
    if (sysRef) {
      return {
        label: systemActionName(sysRef) ?? action,
        icon: 'i-lucide-settings-2',
      }
    }

    const textAction = parseTextAction(action)
    if (textAction !== null) {
      return {
        label: textAction,
        icon: 'i-lucide-text-cursor-input',
      }
    }

    return { label: action }
  }

  return {
    macros,
    byId,
    macroNameById,
    displayAction,
    getActionInfo,
    MACRO_ACTION_PREFIX,
  }
}
